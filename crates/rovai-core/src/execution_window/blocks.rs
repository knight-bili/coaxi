//! Read-time blocks over the existing operation index. A folded group transports at most
//! its latest and current operation; its children use an independent, bounded cursor.
use std::collections::{BTreeMap, HashMap};

use super::*;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCounts {
    pub completed: usize,
    pub failed: usize,
    pub stopped: usize,
    pub recorded: usize,
    pub running: usize,
    pub waiting: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionBlock {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projection_version: Option<i64>,
    pub key: String,
    pub kind: &'static str,
    pub sequence: i64,
    pub last_sequence: i64,
    pub change_sequence: i64,
    pub tool_count: usize,
    pub counts: ToolCounts,
    pub evidence: Vec<AgentRunExecutionEvidenceView>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub supporting_evidence: Vec<AgentRunExecutionEvidenceView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionBlockPage {
    pub schema_version: i64,
    pub delegation_blocks_version: i64,
    pub thread_id: String,
    pub agent_run_id: String,
    pub requested_before_sequence: Option<i64>,
    pub requested_after_sequence: Option<i64>,
    pub next_before_sequence: Option<i64>,
    pub next_after_sequence: Option<i64>,
    pub through_sequence: i64,
    pub through_change_sequence: i64,
    pub has_more: bool,
    pub blocks: Vec<ExecutionBlock>,
    pub active_blocks: Vec<ExecutionBlock>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionBlockChanges {
    pub schema_version: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_after_entry_sequence: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_after_entry_sequence: Option<i64>,
    pub thread_id: String,
    pub agent_run_id: String,
    pub requested_after_change_sequence: i64,
    pub next_after_change_sequence: i64,
    pub through_sequence: i64,
    pub through_change_sequence: i64,
    pub has_more: bool,
    pub blocks: Vec<ExecutionBlock>,
    pub refreshed_blocks: Vec<ExecutionBlock>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionGroupPage {
    pub schema_version: i64,
    pub thread_id: String,
    pub agent_run_id: String,
    pub group_sequence: i64,
    pub requested_before_sequence: Option<i64>,
    pub requested_after_sequence: Option<i64>,
    pub next_before_sequence: Option<i64>,
    pub next_after_sequence: Option<i64>,
    pub through_change_sequence: i64,
    pub has_more: bool,
    pub evidence: Vec<AgentRunExecutionEvidenceView>,
}

#[derive(Clone)]
struct Item {
    sequence: i64,
    ids: String,
    change: i64,
    metadata: AgentRunExecutionEvidenceView,
    visible: bool,
    carrier_for: Option<String>,
    support_only: bool,
}
struct Block {
    items: Vec<Item>,
    tool: bool,
    supporting: Vec<Item>,
    projection_version: Option<i64>,
}
impl Block {
    fn sequence(&self) -> i64 {
        self.items[0].sequence
    }
    fn last_sequence(&self) -> i64 {
        self.items.last().unwrap().sequence
    }
    fn change(&self) -> i64 {
        self.items
            .iter()
            .chain(&self.supporting)
            .map(|item| item.change)
            .max()
            .unwrap_or(0)
    }
    fn visible(&self) -> impl Iterator<Item = &Item> {
        self.items.iter().filter(|item| item.visible)
    }
    fn active(&self) -> bool {
        self.visible().any(|item| {
            item.metadata
                .canonical
                .as_ref()
                .is_some_and(|canonical| canonical.phase != "terminal")
        })
    }
}

fn status(item: &AgentRunExecutionEvidenceView) -> &'static str {
    if let Some(canonical) = &item.canonical {
        return match canonical.outcome.as_str() {
            "succeeded" => "completed",
            "failed" => "failed",
            "cancelled" => "stopped",
            "unknown" if canonical.phase == "started" || canonical.phase == "progress" => "running",
            "unknown"
                if canonical.phase != "terminal"
                    && item.payload["status"]
                        .as_str()
                        .is_some_and(|status| status.contains("wait")) =>
            {
                "waiting"
            }
            _ => "recorded",
        };
    }
    match item.phase.as_str() {
        "completed" => "completed",
        "failed" => "failed",
        _ => "running",
    }
}

// This index carries classification inputs, never result text, diffs or full narration.
// All logical selection and hydration happen in the same SQLite read transaction.
fn index(connection: &Connection, run: &str, delegations: bool) -> Result<Vec<Block>> {
    let logical = select_items(connection, run, None, None, None, None, i64::MAX)?;
    let mut statement = connection.prepare(r#"
        SELECT id, agent_run_id, execution_epoch, sequence, event_type, kind, phase,
          json_object(
            'item', json_object('type', json_extract(payload_preview_json, '$.item.type'),
              'command', json_extract(payload_preview_json, '$.item.command'),
              'tool', json_extract(payload_preview_json, '$.item.tool'),
              'title', json_extract(payload_preview_json, '$.item.title')),
            'command', json_extract(payload_preview_json, '$.command'),
            'canonicalTool', json_extract(payload_preview_json, '$.canonicalTool'),
            'sourceAuthority', json_extract(payload_preview_json, '$.sourceAuthority'),
            'resultDigest', json_extract(payload_preview_json, '$.resultDigest'),
            'agentOutputDigest', json_extract(payload_preview_json, '$.agentOutputDigest'),
            'coreEnvelope', json_object('ok', json_extract(payload_preview_json, '$.coreEnvelope.ok'),
              'operation', json_extract(payload_preview_json, '$.coreEnvelope.operation')),
            'input', CASE WHEN json_type(payload_preview_json, '$.input') = 'text'
              THEN json_extract(payload_preview_json, '$.input')
              ELSE json_object('command', COALESCE(json_extract(payload_preview_json, '$.input.command'),
                json_extract(payload_preview_json, '$.input.commandLine'),
                json_extract(payload_preview_json, '$.input.CommandLine'), json_extract(payload_preview_json, '$.input.cmd'))) END,
            'title', json_extract(payload_preview_json, '$.title'),
            'status', COALESCE(json_extract(payload_preview_json, '$.status'), json_extract(payload_preview_json, '$.item.status')),
            'tool', json_extract(payload_preview_json, '$.tool'),
            'hasContent', CASE WHEN event_type IN ('agent.text.block', 'agent.text.delta')
              THEN json_extract(payload_preview_json, '$.status') = 'streaming'
                OR length(COALESCE(json_extract(payload_preview_json, '$.text'), json_extract(payload_preview_json, '$.delta'), '')) > 0
              WHEN event_type IN ('runtime.plan', 'runtime.plan.delta')
              THEN length(COALESCE(json_extract(payload_preview_json, '$.explanation'), json_extract(payload_preview_json, '$.delta'), '')) > 0
                OR COALESCE(json_array_length(payload_preview_json, '$.plan'), 0) > 0
              ELSE 1 END),
          NULL, 0, 1, occurred_at, operation_id, revision, change_sequence, output_truncated
        FROM agent_run_execution_evidence WHERE agent_run_id = ?1 AND kind <> 'reasoning_summary'
          AND event_type IN ('agent.text.block', 'agent.text.delta', 'runtime.plan', 'runtime.plan.delta',
            'runtime.diagnostic', 'runtime.compaction.display', 'activity.started', 'activity.completed', 'runtime.action')
        ORDER BY sequence
    "#)?;
    let metadata = statement
        .query_map([run], execution_evidence_row)?
        .map(|row| execution_evidence_view(row?))
        .collect::<Result<Vec<_>>>()?;
    let by_id: HashMap<_, _> = metadata
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect();
    let mut indexed = Vec::with_capacity(logical.len());
    for (sequence, ids, change) in logical.into_iter().rev() {
        let source: Vec<String> = serde_json::from_str(&ids)?;
        let mut parts = source
            .iter()
            .filter_map(|id| by_id.get(id))
            .collect::<Vec<_>>();
        parts.sort_by_key(|item| item.sequence);
        let Some(last) = parts.pop() else { continue };
        let mut metadata = last.clone();
        for earlier in parts.into_iter().rev() {
            fill_missing(&mut metadata.payload, &earlier.payload);
        }
        metadata.sequence = sequence;
        indexed.push((sequence, ids, change, metadata));
    }
    let mut metadata = indexed
        .iter()
        .map(|item| item.3.clone())
        .collect::<Vec<_>>();
    crate::read_model::attach_canonical_activity_projection(connection, &mut metadata, true)?;
    let mut associations = super::associations::carrier_targets(connection, &metadata)?;
    let is_standalone = |item: &AgentRunExecutionEvidenceView| {
        item.canonical.as_ref().is_some_and(|canonical| {
            delegations
                && canonical.source_authority == "core"
                && canonical.credibility == "core_verified"
                && item.payload["sourceAuthority"] == "core"
                && canonical.tool_name.as_deref() == item.payload["canonicalTool"].as_str()
                && matches!(
                    item.payload["canonicalTool"].as_str(),
                    Some("delegation.start" | "delegation.resume")
                )
        })
    };
    let standalone: std::collections::HashSet<(i64, String)> = metadata
        .iter()
        .filter(|item| is_standalone(item))
        .map(|item| {
            (
                item.execution_epoch,
                item.canonical.as_ref().unwrap().operation_id.clone(),
            )
        })
        .collect();
    if delegations {
        let mut counts: HashMap<(i64, String), usize> = HashMap::new();
        for item in &metadata {
            if let Some(target) = associations.get(&item.id) {
                *counts
                    .entry((item.execution_epoch, target.clone()))
                    .or_default() += 1;
            }
        }
        for item in &metadata {
            if let Some(target) = associations.get(&item.id) {
                let key = (item.execution_epoch, target.clone());
                if standalone.contains(&key) && counts.get(&key) != Some(&1) {
                    associations.remove(&item.id);
                }
            }
        }
    }
    let mut items = Vec::with_capacity(indexed.len());
    for ((sequence, ids, change, _), metadata) in indexed.into_iter().zip(metadata) {
        let tool = matches!(
            metadata.event_type.as_str(),
            "activity.started" | "activity.completed" | "runtime.action"
        );
        let carrier_for = associations.remove(&metadata.id);
        let support_only = carrier_for
            .as_ref()
            .is_some_and(|id| standalone.contains(&(metadata.execution_epoch, id.clone())));
        let visible = if tool {
            carrier_for.is_none() && visible_tool(&metadata)
        } else {
            metadata.payload["hasContent"].as_i64() == Some(1)
        };
        items.push(Item {
            sequence,
            ids,
            change,
            metadata,
            visible,
            carrier_for,
            support_only,
        });
    }
    if delegations {
        propagate_delegation_changes(&mut items, &is_standalone);
    }
    let mut supports: HashMap<(i64, String), Vec<Item>> = HashMap::new();
    let mut blocks: Vec<Block> = Vec::new();
    for item in items {
        let tool = matches!(
            item.metadata.event_type.as_str(),
            "activity.started" | "activity.completed" | "runtime.action"
        );
        let groupable = tool && !is_standalone(&item.metadata);
        if item.support_only {
            supports
                .entry((
                    item.metadata.execution_epoch,
                    item.carrier_for.clone().unwrap(),
                ))
                .or_default()
                .push(item.clone());
        }
        if groupable
            && blocks.last().is_some_and(|block| {
                block.tool
                    && block.items[0].metadata.execution_epoch == item.metadata.execution_epoch
            })
        {
            blocks.last_mut().unwrap().items.push(item);
        } else {
            blocks.push(Block {
                items: vec![item],
                tool: groupable,
                supporting: Vec::new(),
                projection_version: delegations.then_some(4),
            });
        }
    }
    for block in &mut blocks {
        let item = &block.items[0].metadata;
        if is_standalone(item) {
            let key = (
                item.execution_epoch,
                item.canonical.as_ref().unwrap().operation_id.clone(),
            );
            // Exactly one proven transport. Never guess among duplicate carriers.
            if let Some(items) = supports.remove(&key)
                && items.len() == 1
            {
                block.supporting = items;
            }
        }
    }
    Ok(blocks)
}

// A native correction can revoke a proof or introduce a second candidate. All
// temporally related rows must then refresh together, including the old native
// group. This dependency only advances read-time watermarks; it grants no proof.
fn propagate_delegation_changes(
    items: &mut [Item],
    standalone: &impl Fn(&AgentRunExecutionEvidenceView) -> bool,
) {
    let mut anchors: HashMap<i64, BTreeMap<i64, Vec<(usize, i64)>>> = HashMap::new();
    for (index, item) in items.iter().enumerate() {
        if standalone(&item.metadata) {
            let canonical = item.metadata.canonical.as_ref().unwrap();
            anchors
                .entry(item.metadata.execution_epoch)
                .or_default()
                .entry(canonical.first_evidence_sequence)
                .or_default()
                .push((index, canonical.last_evidence_sequence));
        }
    }
    if anchors.is_empty() {
        return;
    }
    let mut changes = items.iter().map(|item| item.change).collect::<Vec<_>>();
    let mut edges = Vec::new();
    for (index, item) in items.iter().enumerate() {
        if !matches!(
            item.metadata.event_type.as_str(),
            "activity.started" | "activity.completed" | "runtime.action"
        ) {
            continue;
        }
        let Some(canonical) =
            item.metadata.canonical.as_ref().filter(|value| {
                value.source_authority != "core" && value.activity_domain == "shell"
            })
        else {
            continue;
        };
        let Some(candidates) = anchors.get(&item.metadata.execution_epoch) else {
            continue;
        };
        let first = canonical.first_evidence_sequence;
        let last = canonical.last_evidence_sequence;
        let single = first == last;
        if first <= 0 || last < first || (!single && last - first <= 1) {
            continue;
        }
        let range = if single {
            first.saturating_sub(1)..=first.saturating_add(1)
        } else {
            first.saturating_add(1)..=last.saturating_sub(1)
        };
        for (sequence, entries) in candidates.range(range) {
            for (candidate, end) in entries {
                if !((single && *sequence == *end && (*sequence - first).abs() == 1)
                    || (!single && *end < last))
                {
                    continue;
                }
                edges.push((index, *candidate));
                // A carrier proof depends on its own row and candidate Core rows.
                changes[index] = changes[index].max(items[*candidate].change);
            }
        }
    }
    // Core support uniqueness depends on each candidate's proof. Native visibility
    // then depends on that uniqueness. Stop after these directed passes: a full
    // connected-component maximum would invalidate an entire alternating history.
    for &(native, core) in &edges {
        changes[core] = changes[core].max(changes[native]);
    }
    for &(native, core) in &edges {
        changes[native] = changes[native].max(changes[core]);
    }
    for (index, item) in items.iter_mut().enumerate() {
        item.change = changes[index];
    }
}

fn visible_tool(item: &AgentRunExecutionEvidenceView) -> bool {
    let native = item
        .payload
        .pointer("/item/type")
        .and_then(Value::as_str)
        .unwrap_or("");
    if matches!(
        native,
        "reasoning" | "agentMessage" | "userMessage" | "plan"
    ) {
        return false;
    }
    let has_diff = item
        .canonical
        .as_ref()
        .and_then(|canonical| canonical.diff_projection.as_ref())
        .is_some_and(|projection| projection.status == "available" && projection.entries.is_some());
    if native == "fileChange" && !has_diff {
        return false;
    }
    if !has_diff
        && [
            item.canonical
                .as_ref()
                .and_then(|canonical| canonical.tool_name.as_deref()),
            item.payload.pointer("/item/tool").and_then(Value::as_str),
            item.payload["tool"].as_str(),
            item.payload["title"].as_str(),
        ]
        .into_iter()
        .flatten()
        .any(|name| name.trim().eq_ignore_ascii_case("apply_patch"))
    {
        return false;
    }
    if let Some(canonical) = &item.canonical {
        if canonical.activity_domain == "shell"
            && status(item) == "running"
            && public_command(&item.payload).is_none()
        {
            let generic = |title: &str| {
                [
                    "bash",
                    "execute",
                    "exec_command",
                    "execute_command",
                    "run command",
                    "run_command",
                    "shell",
                    "terminal",
                    "执行 shell 命令",
                    "终端操作",
                ]
                .contains(&title.to_lowercase().as_str())
            };
            let specific = canonical
                .presentation_hint
                .as_deref()
                .filter(|title| !title.is_empty() && !generic(title))
                .or_else(|| {
                    canonical
                        .tool_name
                        .as_deref()
                        .filter(|title| !title.is_empty() && !generic(title))
                });
            if specific.is_none() {
                return false;
            }
        }
    }
    true
}

pub fn read_group_changes(
    database: &mut Database,
    thread: &str,
    run: &str,
    group: i64,
    after: i64,
    from: i64,
    to: Option<i64>,
    limit: i64,
) -> Result<ExecutionWindowChanges> {
    read_group_changes_for(
        database, thread, run, group, after, from, to, limit, false, None,
    )
}

pub fn read_delegation_group_changes(
    database: &mut Database,
    thread: &str,
    run: &str,
    group: i64,
    after: i64,
    from: i64,
    to: Option<i64>,
    limit: i64,
    after_entry: Option<i64>,
) -> Result<ExecutionWindowChanges> {
    read_group_changes_for(
        database,
        thread,
        run,
        group,
        after,
        from,
        to,
        limit,
        true,
        after_entry,
    )
}

fn read_group_changes_for(
    database: &mut Database,
    thread: &str,
    run: &str,
    group: i64,
    after: i64,
    from: i64,
    to: Option<i64>,
    limit: i64,
    delegations: bool,
    after_entry: Option<i64>,
) -> Result<ExecutionWindowChanges> {
    ensure!(
        group > 0 && from >= group && to.is_none_or(|end| end >= from),
        "Invalid execution group range"
    );
    let tx = database.connection_mut().transaction()?;
    let (through_sequence, through_change_sequence) = watermarks(&tx, thread, run)?;
    ensure!(
        after >= 0 && after <= through_change_sequence,
        "Invalid execution change cursor"
    );
    ensure!(
        after_entry.is_none_or(|entry| delegations
            && after > 0
            && entry >= from
            && entry <= through_sequence
            && to.is_none_or(|end| entry <= end)),
        "Invalid execution group tie cursor"
    );
    let blocks = index(&tx, run, delegations)?;
    let block = blocks
        .iter()
        .find(|block| block.tool && block.sequence() == group)
        .context("Execution group does not exist")?;
    let mut selected = block
        .items
        .iter()
        .filter(|item| {
            (item.change > after
                || after_entry.is_some_and(|entry| item.change == after && item.sequence > entry))
                && item.sequence >= from
                && to.is_none_or(|end| item.sequence <= end)
        })
        .collect::<Vec<_>>();
    selected.sort_by_key(|item| (item.change, item.sequence));
    let limit = limit.clamp(1, 96) as usize;
    let has_more = selected.len() > limit;
    selected.truncate(limit);
    let next = if has_more {
        selected.last().unwrap().change
    } else {
        through_change_sequence
    };
    let next_entry = if delegations && has_more {
        selected.last().map(|item| item.sequence)
    } else {
        None
    };
    let suppressed_operations = selected
        .iter()
        .filter(|item| item.support_only)
        .map(|item| SuppressedExecutionOperation {
            sequence: item.sequence,
            execution_epoch: item.metadata.execution_epoch,
            operation_id: item
                .metadata
                .canonical
                .as_ref()
                .unwrap()
                .operation_id
                .clone(),
        })
        .collect();
    let evidence = project_item_views(
        &tx,
        run,
        selected.iter().copied().filter(|item| !item.support_only),
        delegations,
    )?;
    tx.commit()?;
    Ok(ExecutionWindowChanges {
        schema_version: if delegations { 4 } else { 2 },
        requested_after_entry_sequence: after_entry,
        next_after_entry_sequence: next_entry,
        suppressed_operations,
        camp_id: thread.into(),
        agent_run_id: run.into(),
        requested_after_change_sequence: after,
        next_after_change_sequence: next,
        through_sequence,
        through_change_sequence,
        has_more,
        evidence,
        refreshed_evidence: Vec::new(),
    })
}

pub(super) fn public_command(payload: &Value) -> Option<&str> {
    [
        "/item/command",
        "/input",
        "/input/command",
        "/input/commandLine",
        "/input/CommandLine",
        "/input/cmd",
    ]
    .into_iter()
    .filter_map(|path| payload.pointer(path).and_then(Value::as_str))
    .find(|text| !text.trim().is_empty())
}

fn watermarks(connection: &Connection, thread: &str, run: &str) -> Result<(i64, i64)> {
    ensure!(
        agent_run_belongs_to_camp(connection, thread, run)?,
        "AgentRun does not exist in this Camp"
    );
    Ok(connection.query_row(
        "SELECT (SELECT COALESCE(MAX(sequence), 0) FROM agent_run_execution_evidence WHERE agent_run_id = ?1),
          execution_evidence_change_sequence FROM agent_run WHERE id = ?1", [run],
        |row| Ok((row.get(0)?, row.get(1)?)))?)
}

fn project(connection: &Connection, run: &str, block: &Block) -> Result<ExecutionBlock> {
    let mut counts = ToolCounts::default();
    let mut selected = BTreeMap::new();
    let visible = block.visible().collect::<Vec<_>>();
    if let Some(item) = visible.last() {
        selected.insert(item.sequence, item.ids.clone());
    }
    let mut last_active = None;
    if block.tool {
        for item in &visible {
            match status(&item.metadata) {
                "completed" => counts.completed += 1,
                "failed" => counts.failed += 1,
                "stopped" => counts.stopped += 1,
                "running" => {
                    counts.running += 1;
                    last_active = Some(item);
                }
                "waiting" => {
                    counts.waiting += 1;
                    last_active = Some(item);
                }
                _ => counts.recorded += 1,
            }
        }
        if let Some(item) = last_active {
            selected.insert(item.sequence, item.ids.clone());
        }
    }
    // Include a selected Core operation's proven Shell carrier so the existing renderer
    // preserves its command and result source. These supports never add to the block count.
    let operation_ids = block
        .items
        .iter()
        .filter(|item| selected.contains_key(&item.sequence))
        .filter_map(|item| {
            item.metadata
                .canonical
                .as_ref()
                .map(|canonical| canonical.operation_id.as_str())
        })
        .collect::<Vec<_>>();
    for operation_id in operation_ids {
        let carriers = block
            .items
            .iter()
            .filter(|item| item.carrier_for.as_deref() == Some(operation_id))
            .collect::<Vec<_>>();
        if let [carrier] = carriers.as_slice() {
            selected.insert(carrier.sequence, carrier.ids.clone());
        }
    }
    Ok(ExecutionBlock {
        projection_version: block.projection_version,
        key: format!(
            "{}:{}",
            if block.tool { "tools" } else { "item" },
            block.sequence()
        ),
        kind: if block.tool { "toolGroup" } else { "item" },
        sequence: block.sequence(),
        last_sequence: block.last_sequence(),
        change_sequence: block.change(),
        tool_count: if block.tool { visible.len() } else { 0 },
        counts,
        evidence: project_item_views(
            connection,
            run,
            block
                .items
                .iter()
                .filter(|item| selected.contains_key(&item.sequence)),
            block.projection_version.is_some(),
        )?,
        supporting_evidence: project_item_views(connection, run, block.supporting.iter(), true)?,
    })
}

fn project_item_views<'a>(
    connection: &Connection,
    run: &str,
    items: impl Iterator<Item = &'a Item>,
    derived: bool,
) -> Result<Vec<AgentRunExecutionEvidenceView>> {
    let items = items.collect::<Vec<_>>();
    let mut evidence = project_items(
        connection,
        run,
        items.iter().map(|item| (item.sequence, item.ids.clone())),
    )?;
    if derived {
        let changes: HashMap<_, _> = items
            .iter()
            .map(|item| (item.sequence, item.change))
            .collect();
        for item in &mut evidence {
            item.change_sequence = changes.get(&item.sequence).copied();
        }
    }
    Ok(evidence)
}

fn overlay(database: &Database, blocks: &mut [ExecutionBlock]) -> Result<()> {
    for block in blocks {
        crate::execution_text::overlay(database, &mut block.evidence)?;
        crate::execution_text::overlay(database, &mut block.supporting_evidence)?;
    }
    Ok(())
}

pub fn read_block_page(
    database: &mut Database,
    thread: &str,
    run: &str,
    before: Option<i64>,
    after: Option<i64>,
    limit: i64,
) -> Result<ExecutionBlockPage> {
    read_block_page_for(database, thread, run, before, after, limit, false)
}

pub fn read_delegation_block_page(
    database: &mut Database,
    thread: &str,
    run: &str,
    before: Option<i64>,
    after: Option<i64>,
    limit: i64,
) -> Result<ExecutionBlockPage> {
    read_block_page_for(database, thread, run, before, after, limit, true)
}

fn read_block_page_for(
    database: &mut Database,
    thread: &str,
    run: &str,
    before: Option<i64>,
    after: Option<i64>,
    limit: i64,
    delegations: bool,
) -> Result<ExecutionBlockPage> {
    ensure!(
        !(before.is_some() && after.is_some())
            && before.is_none_or(|n| n > 0)
            && after.is_none_or(|n| n >= 0),
        "Invalid execution block cursor"
    );
    let tx = database.connection_mut().transaction()?;
    let (through_sequence, through_change_sequence) = watermarks(&tx, thread, run)?;
    let blocks = index(&tx, run, delegations)?;
    let limit = limit.clamp(1, 48) as usize;
    let mut selected = blocks
        .iter()
        .filter(|block| {
            block.visible().next().is_some()
                && before.is_none_or(|cursor| block.sequence() < cursor)
                && after.is_none_or(|cursor| block.sequence() > cursor)
        })
        .collect::<Vec<_>>();
    if after.is_none() {
        selected.reverse();
    }
    let has_more = selected.len() > limit;
    selected.truncate(limit);
    let cursor = selected.last().map(|block| block.sequence());
    selected.sort_by_key(|block| block.sequence());
    let mut projected = selected
        .iter()
        .map(|block| project(&tx, run, block))
        .collect::<Result<Vec<_>>>()?;
    let running: bool = tx.query_row(
        "SELECT status IN ('running', 'waiting', 'queued') FROM agent_run WHERE id = ?1",
        [run],
        |row| row.get(0),
    )?;
    let mut active_blocks = if running && before.is_none() && after.is_none() {
        blocks
            .iter()
            .filter(|block| {
                block.active()
                    && !selected
                        .iter()
                        .any(|other| other.sequence() == block.sequence())
            })
            .map(|block| project(&tx, run, block))
            .collect::<Result<Vec<_>>>()?
    } else {
        Vec::new()
    };
    tx.commit()?;
    overlay(database, &mut projected)?;
    overlay(database, &mut active_blocks)?;
    Ok(ExecutionBlockPage {
        schema_version: if delegations { 4 } else { 3 },
        delegation_blocks_version: 4,
        thread_id: thread.into(),
        agent_run_id: run.into(),
        requested_before_sequence: before,
        requested_after_sequence: after,
        next_before_sequence: if has_more && after.is_none() {
            cursor
        } else {
            None
        },
        next_after_sequence: if has_more && after.is_some() {
            cursor
        } else {
            None
        },
        through_sequence,
        through_change_sequence,
        has_more,
        blocks: projected,
        active_blocks,
    })
}

pub fn read_block_changes(
    database: &mut Database,
    thread: &str,
    run: &str,
    after: i64,
    watched: &[String],
    limit: i64,
) -> Result<ExecutionBlockChanges> {
    read_block_changes_for(database, thread, run, after, watched, limit, false, None)
}

pub fn read_delegation_block_changes(
    database: &mut Database,
    thread: &str,
    run: &str,
    after: i64,
    watched: &[String],
    limit: i64,
    after_entry: Option<i64>,
) -> Result<ExecutionBlockChanges> {
    read_block_changes_for(
        database,
        thread,
        run,
        after,
        watched,
        limit,
        true,
        after_entry,
    )
}

fn read_block_changes_for(
    database: &mut Database,
    thread: &str,
    run: &str,
    after: i64,
    watched: &[String],
    limit: i64,
    delegations: bool,
    after_entry: Option<i64>,
) -> Result<ExecutionBlockChanges> {
    ensure!(watched.len() <= 256, "Invalid execution refresh set");
    let tx = database.connection_mut().transaction()?;
    let (through_sequence, through_change_sequence) = watermarks(&tx, thread, run)?;
    ensure!(
        after >= 0 && after <= through_change_sequence,
        "Invalid execution change cursor"
    );
    ensure!(
        after_entry
            .is_none_or(|block| delegations && after > 0 && block > 0 && block <= through_sequence),
        "Invalid execution block change cursor"
    );
    let blocks = index(&tx, run, delegations)?;
    let beyond = |block: &Block| {
        block.change() > after
            || after_entry
                .is_some_and(|sequence| block.change() == after && block.sequence() > sequence)
    };
    // Empty groups are tombstones when a formerly visible carrier becomes folded.
    let mut changed = blocks
        .iter()
        .filter(|block| beyond(block))
        .collect::<Vec<_>>();
    changed.sort_by_key(|block| (block.change(), block.sequence()));
    let limit = limit.clamp(1, 96) as usize;
    let has_more = changed.len() > limit;
    changed.truncate(limit);
    let next = if has_more {
        changed.last().unwrap().change()
    } else {
        through_change_sequence
    };
    let next_block = if delegations && has_more {
        changed.last().map(|block| block.sequence())
    } else {
        None
    };
    let mut refreshed_blocks = blocks
        .iter()
        .filter(|block| {
            !beyond(block)
                && block
                    .items
                    .iter()
                    .any(|item| watched.contains(&item.metadata.id))
        })
        .map(|block| project(&tx, run, block))
        .collect::<Result<Vec<_>>>()?;
    let mut blocks = changed
        .into_iter()
        .map(|block| project(&tx, run, block))
        .collect::<Result<Vec<_>>>()?;
    tx.commit()?;
    overlay(database, &mut blocks)?;
    overlay(database, &mut refreshed_blocks)?;
    Ok(ExecutionBlockChanges {
        schema_version: if delegations { 4 } else { 3 },
        requested_after_entry_sequence: after_entry,
        next_after_entry_sequence: next_block,
        thread_id: thread.into(),
        agent_run_id: run.into(),
        requested_after_change_sequence: after,
        next_after_change_sequence: next,
        through_sequence,
        through_change_sequence,
        has_more,
        blocks,
        refreshed_blocks,
    })
}

pub fn read_group_page(
    database: &mut Database,
    thread: &str,
    run: &str,
    group: i64,
    before: Option<i64>,
    after: Option<i64>,
    limit: i64,
) -> Result<ExecutionGroupPage> {
    read_group_page_for(database, thread, run, group, before, after, limit, false)
}

pub fn read_delegation_group_page(
    database: &mut Database,
    thread: &str,
    run: &str,
    group: i64,
    before: Option<i64>,
    after: Option<i64>,
    limit: i64,
) -> Result<ExecutionGroupPage> {
    read_group_page_for(database, thread, run, group, before, after, limit, true)
}

fn read_group_page_for(
    database: &mut Database,
    thread: &str,
    run: &str,
    group: i64,
    before: Option<i64>,
    after: Option<i64>,
    limit: i64,
    delegations: bool,
) -> Result<ExecutionGroupPage> {
    ensure!(
        group > 0
            && !(before.is_some() && after.is_some())
            && before.is_none_or(|n| n > 0)
            && after.is_none_or(|n| n >= 0),
        "Invalid execution group cursor"
    );
    let tx = database.connection_mut().transaction()?;
    let (_, through_change_sequence) = watermarks(&tx, thread, run)?;
    let blocks = index(&tx, run, delegations)?;
    let block = blocks
        .iter()
        .find(|block| block.tool && block.sequence() == group)
        .context("Execution group does not exist")?;
    let limit = limit.clamp(1, 96) as usize;
    let mut selected = block
        .items
        .iter()
        .filter(|item| {
            !item.support_only
                && before.is_none_or(|cursor| item.sequence < cursor)
                && after.is_none_or(|cursor| item.sequence > cursor)
        })
        .collect::<Vec<_>>();
    if after.is_none() {
        selected.reverse();
    }
    let has_more = selected.len() > limit;
    selected.truncate(limit);
    let cursor = selected.last().map(|item| item.sequence);
    selected.sort_by_key(|item| item.sequence);
    let evidence = project_item_views(&tx, run, selected.into_iter(), delegations)?;
    tx.commit()?;
    Ok(ExecutionGroupPage {
        schema_version: if delegations { 4 } else { 3 },
        thread_id: thread.into(),
        agent_run_id: run.into(),
        group_sequence: group,
        requested_before_sequence: before,
        requested_after_sequence: after,
        next_before_sequence: if has_more && after.is_none() {
            cursor
        } else {
            None
        },
        next_after_sequence: if has_more && after.is_some() {
            cursor
        } else {
            None
        },
        through_change_sequence,
        has_more,
        evidence,
    })
}
