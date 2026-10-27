// Extends the existing slow pagination owner and its SQLite fixture. These are
// presentation proofs, not actual delegation creation or Runtime execution.
fn assert_delegation_execution_blocks(
    database: &mut Database,
    camp: &str,
    run: &str,
    now: &str,
    directory: &Path,
) {
    use crate::execution_window::{
        read_block_page, read_delegation_block_changes, read_delegation_block_page,
        read_delegation_group_changes, read_delegation_group_page,
    };
    fn evidence(db: &Database, run: &str, now: &str, sequence: i64, event: &str, payload: Value) {
        db.connection().execute(
            "INSERT INTO agent_run_execution_evidence(id, agent_run_id, execution_epoch, sequence,
             event_type, kind, phase, payload_preview_json, content_byte_count, is_truncated,
             occurred_at, revision, change_sequence, updated_at)
             VALUES(?1, ?2, 0, ?3, ?4, ?5, 'completed', ?6, 100, 0, ?7, 1, ?3, ?7)",
            params![format!("delegation-block-{sequence}"), run, sequence, event,
                if event == "agent.text.block" { "narration" } else { "command" }, payload.to_string(), now],
        ).unwrap();
    }
    fn canonical(
        db: &Database,
        run: &str,
        now: &str,
        operation: &str,
        first: i64,
        last: i64,
        core: bool,
        name: &str,
    ) {
        let sources = if first == last {
            json!([format!("delegation-block-{first}")])
        } else {
            json!([
                format!("delegation-block-{first}"),
                format!("delegation-block-{last}")
            ])
        };
        db.connection().execute(
            "INSERT INTO canonical_runtime_activity(agent_run_id, execution_epoch, operation_id,
             classifier_version, activity_domain, phase, outcome, credibility, coverage_level,
             source_authority, source_evidence_ids_json, first_evidence_sequence, last_evidence_sequence,
             revision, created_at, updated_at, tool_name)
             VALUES(?1, 0, ?2, 'activity-v1', ?3, 'terminal', 'succeeded', ?4, 'fine_grained',
             ?5, ?6, ?7, ?8, 1, ?9, ?9, ?10)",
            params![run, operation, if core { "tool" } else { "shell" },
                if core { "core_verified" } else { "runtime_structured" },
                if core { "core" } else { "runtime" }, sources.to_string(), first, last, now, name],
        ).unwrap();
    }
    fn watermark(db: &Database, run: &str, value: i64) {
        db.connection()
            .execute(
                "UPDATE agent_run SET execution_evidence_change_sequence = ?1 WHERE id = ?2",
                params![value, run],
            )
            .unwrap();
    }
    fn change(db: &Database, run: &str, sequence: i64, value: i64, payload: Value) {
        db.connection().execute("UPDATE agent_run_execution_evidence SET payload_preview_json = ?1,
            revision = revision + 1, change_sequence = ?2 WHERE agent_run_id = ?3 AND sequence = ?4",
            params![payload.to_string(), value, run, sequence]).unwrap();
        watermark(db, run, value);
    }
    let base: i64 = database
        .connection()
        .query_row(
            "SELECT MAX(sequence) + 10 FROM agent_run_execution_evidence WHERE agent_run_id = ?1",
            [run],
            |row| row.get(0),
        )
        .unwrap();
    let digest = "a".repeat(64);
    let command = "coaxi delegation start --agent-type codex --task fixture";
    let shell = |command: &str, digest: &str| {
        json!({"resultDigest":digest,
        "item":{"type":"commandExecution","command":command,"aggregatedOutput":"原始完整结果"}})
    };
    let core = |operation: &str| {
        json!({"sourceAuthority":"core", "canonicalTool":operation,
        "agentOutputDigest":digest, "coreEnvelope":{"ok":true,"operation":operation}})
    };
    evidence(
        database,
        run,
        now,
        base,
        "agent.text.block",
        json!({"text":"delegation boundary"}),
    );
    evidence(
        database,
        run,
        now,
        base + 1,
        "activity.started",
        shell(command, &digest),
    );
    evidence(
        database,
        run,
        now,
        base + 2,
        "activity.completed",
        shell("echo before", "ordinary"),
    );
    evidence(
        database,
        run,
        now,
        base + 3,
        "runtime.action",
        core("delegation.start"),
    );
    evidence(
        database,
        run,
        now,
        base + 4,
        "activity.completed",
        shell("echo after", "ordinary"),
    );
    evidence(
        database,
        run,
        now,
        base + 5,
        "activity.completed",
        shell(command, &digest),
    );
    canonical(
        database,
        run,
        now,
        "delegation-block-shell",
        base + 1,
        base + 5,
        false,
        "shell",
    );
    canonical(
        database,
        run,
        now,
        "delegation-block-before",
        base + 2,
        base + 2,
        false,
        "shell",
    );
    canonical(
        database,
        run,
        now,
        "delegation-block-core",
        base + 3,
        base + 3,
        true,
        "delegation.start",
    );
    canonical(
        database,
        run,
        now,
        "delegation-block-after",
        base + 4,
        base + 4,
        false,
        "shell",
    );
    watermark(database, run, base + 5);
    let writes = database.connection().total_changes();
    let legacy = read_block_page(database, camp, run, None, None, 1).unwrap();
    assert_eq!(
        (legacy.schema_version, legacy.delegation_blocks_version),
        (3, 4)
    );
    assert_eq!(legacy.blocks[0].tool_count, 3);
    assert!(legacy.blocks[0].projection_version.is_none());
    assert!(legacy.blocks[0].supporting_evidence.is_empty());
    let page = read_delegation_block_page(database, camp, run, None, None, 3).unwrap();
    assert_eq!(page.schema_version, 4);
    assert_eq!(
        page.blocks
            .iter()
            .map(|item| item.sequence)
            .collect::<Vec<_>>(),
        vec![base + 1, base + 3, base + 4]
    );
    assert_eq!(page.blocks[0].tool_count, 1);
    let source = &page.blocks[1];
    assert_eq!(
        (source.kind, source.sequence, source.last_sequence),
        ("item", base + 3, base + 3)
    );
    assert_eq!(source.projection_version, Some(4));
    assert_eq!(source.evidence.len(), 1);
    assert_eq!(source.evidence[0].sequence, base + 3);
    assert_eq!(source.supporting_evidence.len(), 1);
    assert_eq!(source.supporting_evidence[0].sequence, base + 1);
    assert_eq!(
        source.supporting_evidence[0].payload["executionWindowBuiltinOperation"],
        "delegation.start"
    );
    assert!(
        source.supporting_evidence[0].payload["item"]["aggregatedOutput"].is_null(),
        "folded transport must not prefetch output"
    );
    let stored = crate::execution_evidence::ExecutionEvidenceService
        .read_full_payload(
            database,
            &crate::managed_blob::ManagedBlobStore::new(directory),
            camp,
            &source.supporting_evidence[0].id,
        )
        .unwrap();
    assert_eq!(stored["item"]["aggregatedOutput"], "原始完整结果");
    assert_eq!(source.change_sequence, base + 5);
    let children =
        read_delegation_group_page(database, camp, run, base + 1, None, None, 1).unwrap();
    assert_eq!(children.schema_version, 4);
    assert_eq!(
        children
            .evidence
            .iter()
            .map(|item| item.sequence)
            .collect::<Vec<_>>(),
        vec![base + 2]
    );
    assert!(!children.has_more);
    assert!(read_delegation_group_page(database, camp, run, base + 3, None, None, 1).is_err());
    assert!(read_delegation_block_page(database, "other-thread", run, None, None, 1).is_err());
    assert!(
        read_delegation_group_page(database, "other-thread", run, base + 1, None, None, 1).is_err()
    );
    let older = read_delegation_block_page(database, camp, run, Some(base + 3), None, 1).unwrap();
    assert_eq!(older.blocks[0].sequence, base + 1);
    let newer = read_delegation_block_page(database, camp, run, None, Some(base + 1), 1).unwrap();
    assert_eq!(newer.blocks[0].sequence, base + 3);
    assert_eq!(newer.next_after_sequence, Some(base + 3));
    assert_eq!(
        database.connection().total_changes(),
        writes,
        "projection reads never write"
    );

    // Starting/resuming is standalone before a result exists. A Runtime merely
    // using the same name and payload cannot acquire Core presentation authority.
    evidence(
        database,
        run,
        now,
        base + 6,
        "runtime.action",
        core("delegation.resume"),
    );
    canonical(
        database,
        run,
        now,
        "delegation-block-resume",
        base + 6,
        base + 6,
        true,
        "delegation.resume",
    );
    database
        .connection()
        .execute(
            "UPDATE canonical_runtime_activity SET phase = 'started', outcome = 'unknown'
        WHERE agent_run_id = ?1 AND operation_id = 'delegation-block-resume'",
            [run],
        )
        .unwrap();
    evidence(
        database,
        run,
        now,
        base + 7,
        "runtime.action",
        core("delegation.resume"),
    );
    canonical(
        database,
        run,
        now,
        "delegation-block-spoof",
        base + 7,
        base + 7,
        false,
        "delegation.resume",
    );
    watermark(database, run, base + 7);
    let tail = read_delegation_block_page(database, camp, run, None, None, 2).unwrap();
    assert_eq!(tail.blocks[0].kind, "item");
    assert_eq!(tail.blocks[0].sequence, base + 6);
    assert!(tail.blocks[0].supporting_evidence.is_empty());
    assert_eq!(tail.blocks[1].kind, "toolGroup");

    // Providers may reuse operation ids in another execution epoch. That cannot
    // consume the earlier card's support or create a false uniqueness conflict.
    evidence(
        database,
        run,
        now,
        base + 8,
        "runtime.action",
        core("delegation.start"),
    );
    canonical(
        database,
        run,
        now,
        "delegation-block-other-epoch",
        base + 8,
        base + 8,
        true,
        "delegation.start",
    );
    database
        .connection()
        .execute(
            "UPDATE agent_run_execution_evidence SET execution_epoch = 1
        WHERE agent_run_id = ?1 AND sequence = ?2",
            params![run, base + 8],
        )
        .unwrap();
    database
        .connection()
        .execute(
            "UPDATE canonical_runtime_activity SET execution_epoch = 1,
        operation_id = 'delegation-block-core' WHERE agent_run_id = ?1
        AND operation_id = 'delegation-block-other-epoch'",
            [run],
        )
        .unwrap();
    watermark(database, run, base + 8);
    let epoch = read_delegation_block_page(database, camp, run, None, None, 1).unwrap();
    assert_eq!(epoch.blocks[0].sequence, base + 8);
    assert!(epoch.blocks[0].supporting_evidence.is_empty());
    let earlier = read_delegation_block_page(database, camp, run, Some(base + 4), None, 1).unwrap();
    assert_eq!(earlier.blocks[0].supporting_evidence.len(), 1);

    // A second matching transport invalidates both the Core support and the
    // original carrier's group, even though only the second raw row changed.
    let ambiguous = base + 9;
    change(database, run, base + 4, ambiguous, shell(command, &digest));
    let writes = database.connection().total_changes();
    let page = read_delegation_block_page(database, camp, run, None, None, 6).unwrap();
    let source = page
        .blocks
        .iter()
        .find(|item| item.sequence == base + 3)
        .unwrap();
    assert!(source.supporting_evidence.is_empty());
    assert_eq!(source.change_sequence, ambiguous);
    let before = page
        .blocks
        .iter()
        .find(|item| item.sequence == base + 1)
        .unwrap();
    assert_eq!(before.tool_count, 2);
    assert_eq!(
        before.change_sequence, ambiguous,
        "old carrier must become visible in a delta"
    );
    let mut after = base + 8;
    let mut entry = None;
    let mut seen = Vec::new();
    for _ in 0..8 {
        let delta =
            read_delegation_block_changes(database, camp, run, after, &[], 1, entry).unwrap();
        assert_eq!(delta.schema_version, 4);
        assert!(delta.blocks.len() <= 1);
        seen.extend(delta.blocks.iter().map(|item| item.sequence));
        if !delta.has_more {
            assert!(delta.next_after_entry_sequence.is_none());
            break;
        }
        let next = (
            delta.next_after_change_sequence,
            delta.next_after_entry_sequence,
        );
        assert_ne!(next, (after, entry));
        (after, entry) = next;
    }
    assert_eq!(seen, vec![base + 1, base + 3, base + 4]);
    assert_eq!(database.connection().total_changes(), writes);

    // Restoring uniqueness removes a cached native operation using a bounded
    // suppression descriptor. Shared derived watermarks also need a tie cursor
    // inside an expanded group, not only in the top-level block stream.
    let restored = base + 10;
    change(
        database,
        run,
        base + 4,
        restored,
        shell("echo after", "ordinary"),
    );
    let first = read_delegation_group_changes(
        database,
        camp,
        run,
        base + 1,
        ambiguous,
        base + 1,
        Some(base + 2),
        1,
        None,
    )
    .unwrap();
    assert_eq!(first.schema_version, 4);
    assert!(first.evidence.is_empty());
    assert_eq!(first.suppressed_operations.len(), 1);
    assert_eq!(first.suppressed_operations[0].sequence, base + 1);
    assert_eq!(
        first.suppressed_operations[0].operation_id,
        "delegation-block-shell"
    );
    assert_eq!(first.next_after_change_sequence, restored);
    assert!(first.has_more);
    assert_eq!(first.next_after_entry_sequence, Some(base + 1));
    let second = read_delegation_group_changes(
        database,
        camp,
        run,
        base + 1,
        first.next_after_change_sequence,
        base + 1,
        Some(base + 2),
        1,
        first.next_after_entry_sequence,
    )
    .unwrap();
    assert_eq!(second.evidence.len(), 1);
    assert_eq!(second.evidence[0].sequence, base + 2);
    assert_eq!(second.evidence[0].change_sequence, Some(restored));
    assert!(second.suppressed_operations.is_empty());
    assert!(!second.has_more);
    assert!(second.next_after_entry_sequence.is_none());
    let source = read_delegation_block_page(database, camp, run, Some(base + 4), None, 1).unwrap();
    assert_eq!(source.blocks[0].supporting_evidence.len(), 1);
    assert_eq!(source.blocks[0].change_sequence, restored);
    assert!(read_delegation_block_changes(database, camp, run, 0, &[], 1, Some(base + 1)).is_err());
    assert!(
        read_delegation_group_changes(
            database,
            camp,
            run,
            base + 1,
            ambiguous,
            base + 2,
            Some(base + 2),
            1,
            Some(base + 1)
        )
        .is_err()
    );

    // Correcting the original result revokes the support without regressing
    // either cache watermark, and reveals the original full native output.
    let revoked = base + 11;
    change(
        database,
        run,
        base + 5,
        revoked,
        shell(command, &"b".repeat(64)),
    );
    let delta = read_delegation_block_changes(database, camp, run, restored, &[], 8, None).unwrap();
    let source = delta
        .blocks
        .iter()
        .find(|item| item.sequence == base + 3)
        .unwrap();
    assert_eq!(source.change_sequence, revoked);
    assert!(source.supporting_evidence.is_empty());
    let group = read_delegation_group_page(database, camp, run, base + 1, None, None, 2).unwrap();
    assert_eq!(group.evidence.len(), 2);
    assert_eq!(group.evidence[0].sequence, base + 1);
    assert_eq!(group.evidence[0].change_sequence, Some(revoked));
    let old_refresh = read_delegation_block_changes(
        database,
        camp,
        run,
        revoked,
        &[format!("delegation-block-{}", base + 3)],
        1,
        None,
    )
    .unwrap();
    assert!(old_refresh.blocks.is_empty());
    assert_eq!(old_refresh.refreshed_blocks.len(), 1);
    assert!(
        old_refresh.refreshed_blocks[0]
            .supporting_evidence
            .is_empty()
    );

    // A second Core callback inside the same native lifetime is ambiguous too.
    // All participants refresh; proof and temporal dependency are kept separate.
    let competing = base + 13;
    change(
        database,
        run,
        base + 5,
        competing - 1,
        shell(command, &digest),
    );
    change(database, run, base + 2, competing, core("delegation.start"));
    database
        .connection()
        .execute(
            "UPDATE canonical_runtime_activity SET activity_domain = 'tool',
        source_authority = 'core', credibility = 'core_verified', tool_name = 'delegation.start'
        WHERE agent_run_id = ?1 AND operation_id = 'delegation-block-before'",
            [run],
        )
        .unwrap();
    let delta = read_delegation_block_changes(database, camp, run, revoked, &[], 8, None).unwrap();
    for sequence in [base + 2, base + 3] {
        let source = delta
            .blocks
            .iter()
            .find(|item| item.sequence == sequence)
            .unwrap();
        assert_eq!(source.kind, "item");
        assert!(source.supporting_evidence.is_empty());
        assert_eq!(source.change_sequence, competing);
    }
    let group = delta
        .blocks
        .iter()
        .find(|item| item.sequence == base + 1)
        .unwrap();
    assert_eq!(group.tool_count, 1);
    assert_eq!(
        group.last_sequence,
        base + 1,
        "support lifetime never expands a primary block"
    );
    assert_eq!(group.change_sequence, competing);

    // Reuse the existing 10,000-operation performance fixture, now with 5,000
    // potential delegation anchors. No new database, process or Rust test owner.
    database
        .connection()
        .execute(
            "UPDATE agent_run_execution_evidence SET payload_preview_json =
        CASE WHEN sequence % 2 = 0 THEN json_set(payload_preview_json, '$.item.command',
          'coaxi delegation start --agent-type codex --task fixture')
        ELSE json_set(payload_preview_json, '$.canonicalTool', 'delegation.start',
          '$.coreEnvelope.operation', 'delegation.start') END
        WHERE agent_run_id = ?1 AND sequence BETWEEN 102 AND 10101",
            [run],
        )
        .unwrap();
    database.connection().execute("UPDATE canonical_runtime_activity SET tool_name = 'delegation.start'
        WHERE agent_run_id = ?1 AND source_authority = 'core' AND first_evidence_sequence BETWEEN 102 AND 10101", [run]).unwrap();
    let writes = database.connection().total_changes();
    let started = std::time::Instant::now();
    let page = read_delegation_block_page(database, camp, run, Some(base), None, 48).unwrap();
    let elapsed = started.elapsed();
    assert_eq!(page.blocks.len(), 48);
    assert!(page.has_more);
    let bytes = serde_json::to_vec(&page).unwrap().len();
    assert!(
        bytes < 262_144,
        "bounded projection over 10,000 operations: {bytes}"
    );
    assert_eq!(database.connection().total_changes(), writes);
    eprintln!(
        "delegation block probe: operations=10000 anchors=5000 page_blocks=48 response_bytes={bytes} read_ms={}",
        elapsed.as_millis()
    );
    let local_change = competing + 1;
    database
        .connection()
        .execute(
            "UPDATE agent_run_execution_evidence SET revision = revision + 1,
        change_sequence = ?1 WHERE agent_run_id = ?2 AND sequence = 5001",
            params![local_change, run],
        )
        .unwrap();
    watermark(database, run, local_change);
    let writes = database.connection().total_changes();
    let local =
        read_delegation_block_changes(database, camp, run, competing, &[], 96, None).unwrap();
    assert!(
        !local.has_more,
        "one corrected callback must not invalidate all 10,000 operations"
    );
    assert!(!local.blocks.is_empty());
    assert!(local.blocks.len() <= 8);
    assert!(
        local
            .blocks
            .iter()
            .all(|block| (4998..=5004).contains(&block.sequence))
    );
    assert_eq!(database.connection().total_changes(), writes);
    eprintln!(
        "delegation delta locality probe: operations=10000 changed_blocks={}",
        local.blocks.len()
    );
}
