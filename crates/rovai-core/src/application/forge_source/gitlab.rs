//! GitLab REST v4 for the workbench and the delivery path.
//!
//! GitLab is not "GitHub with different words" in the four places that matter
//! here, and each of them is a silent-wrong-answer if you assume otherwise:
//!
//! 1. **Issues and merge requests are separate collections.** GitHub serves
//!    both from `/issues` and splits on a `pull_request` key; here each tab is
//!    its own endpoint, and so is each comment target (`/issues/{iid}/notes`
//!    vs `/merge_requests/{iid}/notes`).
//! 2. **The project is a path, not `{owner}/{repo}`.** Subgroups nest
//!    arbitrarily deep, and the whole path goes into ONE percent-encoded path
//!    segment (`group%2Fsub%2Fproj`).
//! 3. **`merged` is a state, not a flag.** `state=closed` excludes merged
//!    merge requests, so the workbench's "closed" tab asks for everything and
//!    filters locally — otherwise a merged merge request would simply vanish
//!    from a list where GitHub shows it.
//! 4. **Numbers are `iid`, not `id`.** `id` is globally unique and appears in
//!    no URL a human ever sees; addressing by it silently reads another
//!    project's work item.
//!
//! Pagination is offset-based (`page` + `per_page`), and the totals are the
//! one place it differs from the GitHub client: `X-Total` / `X-Total-Pages`
//! are OPTIONAL — GitLab omits them past 10,000 rows on purpose — so
//! `X-Next-Page` is the only signal always present. Rate-limit classification
//! is shared with the GitHub client.

use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

use serde::Deserialize;

use super::auth::ResolvedAuth;
use super::deliver::{ForgePr, NewPullRequest};
use super::{
    BODY_CAP, ForgeChangeDetail, ForgeChangedFile, ForgeChangedFileList, ForgeCheck,
    ForgeCheckList, ForgeCheckState, ForgeComment, ForgeCommentList, ForgeError, ForgeFileStatus,
    ForgeIssueList, ForgeIssueRow, ForgeItemKind, ForgeLabel, ForgeLabelList, ForgeMergeMethod,
    ForgeMergeOptions, ForgeMergeStrategy, ForgeSort, ForgeStateAction, LABEL_PAGE_SIZE,
    ListIssuesRequest, ResolvedNewIssue, count_diff_lines, sanitize_web_url, truncate_chars,
    urlencode_path, urlencode_query, validate_state_filter, web_origin,
};

// ── reads ───────────────────────────────────────────────────────────────────

pub async fn list_issues(
    auth: &ResolvedAuth,
    req: &ListIssuesRequest,
) -> Result<ForgeIssueList, ForgeError> {
    let project = project_ref(&req.owner_repo)?;
    validate_state_filter(&req.state)?;

    let (page, per_page) = req.clamped();

    let collection = match req.tab {
        super::ForgeTab::Issues => "issues",
        super::ForgeTab::Prs => "merge_requests",
    };
    // `with_labels_details=true` turns each entry of `labels` from a bare name
    // into an object carrying the label's colour — the chip the workbench draws
    // is the project's own, as it is on GitHub. Instances that predate the
    // parameter ignore it and keep sending strings, which `RawItemLabel` reads
    // just as happily.
    let mut url = format!(
        "{}/projects/{project}/{collection}?state={}&order_by={}&sort={}\
         &with_labels_details=true&page={page}&per_page={per_page}",
        auth.api_base,
        wire_state(req.tab, &req.state),
        order_by(req.sort),
        req.sort.direction()
    );
    if req.assigned_me {
        // `assignee_username` takes the literal login; there is no `@me`
        // shorthand on the collection endpoints.
        let login = current_login(auth).await?;
        url.push_str(&format!("&assignee_username={}", urlencode_query(&login)));
    }
    if !req.labels.is_empty() {
        // Comma-joined, which is lossless here: GitLab forbids commas in label
        // titles, so no name can be split by this.
        url.push_str(&format!(
            "&labels={}",
            urlencode_query(&req.labels.join(","))
        ));
    }
    if let Some(text) = req.search.as_deref() {
        // `search` is plain text on both collections, so unlike GitHub's `q`
        // there is no query syntax to strip. `in` is title+description by
        // default and is stated anyway: the box promises that scope, and a
        // promise resting on somebody else's default is one release away from
        // being wrong.
        url.push_str(&format!(
            "&search={}&in=title,description",
            urlencode_query(text)
        ));
    }

    let response = api_get(auth, &url).await?;
    let total_count = header_i64(response.headers(), "x-total");
    // Empty means "no page after this one" — the one pagination signal GitLab
    // always sends, even where it declines to count.
    let has_next =
        header_str(response.headers(), "x-next-page").is_some_and(|v| !v.trim().is_empty());
    let raw: Vec<RawItem> = response
        .json()
        .await
        .map_err(|e| ForgeError::Network(format!("bad list payload: {e}")))?;

    let is_pr = req.tab == super::ForgeTab::Prs;
    // Whether this query is narrowed AFTER it arrives — structural, not
    // data-dependent: a page that happens to hold no open rows is still part
    // of a result set whose count includes them elsewhere.
    let filters_locally = wire_state(req.tab, &req.state) == "all" && req.state != "all";
    let rows = raw
        .into_iter()
        .filter(|item| keeps(&req.state, &item.state))
        .map(|item| item.into_row(is_pr))
        .collect();

    Ok(ForgeIssueList {
        rows,
        page,
        per_page,
        // Withheld rather than wrong. Two ways the count stops being true:
        // GitLab omits `X-Total` past 10k rows (documented, for performance),
        // and the closed-merge-request query above asks for `state=all` and
        // filters here — so its total counts open rows the user cannot see.
        // `None` is the signal the UI degrades to previous/next on.
        total_count: total_count.filter(|_| !filters_locally),
        // GitLab paginates the whole collection — no equivalent of GitHub
        // search's first-thousand ceiling, so every match is reachable.
        reachable_count: None,
        has_next,
        // GitLab has no partial-result flag; only GitHub search does.
        incomplete: false,
    })
}

/// GitLab's spelling of the sort field. It is the SAME two fields GitHub
/// offers, suffixed — and the only two both collections (issues and merge
/// requests) accept, which is what makes the workbench's four orders portable.
fn order_by(sort: ForgeSort) -> &'static str {
    match sort.field() {
        "updated" => "updated_at",
        _ => "created_at",
    }
}

/// The project's labels, for the workbench's label filter.
///
/// `with_counts=false` on purpose: the counts are an extra aggregation per
/// label on the server and nothing here shows them.
pub async fn list_labels(
    auth: &ResolvedAuth,
    owner_repo: &str,
) -> Result<ForgeLabelList, ForgeError> {
    let project = project_ref(owner_repo)?;
    let url = format!(
        "{}/projects/{project}/labels?per_page={LABEL_PAGE_SIZE}&with_counts=false",
        auth.api_base
    );
    #[derive(Deserialize)]
    struct RawProjectLabel {
        #[serde(default)]
        name: String,
        /// `#rrggbb` here, unlike GitHub's bare digits.
        #[serde(default)]
        color: Option<String>,
    }
    let raw: Vec<RawProjectLabel> = api_get(auth, &url)
        .await?
        .json()
        .await
        .map_err(|e| ForgeError::Network(format!("bad labels payload: {e}")))?;
    let truncated = raw.len() >= LABEL_PAGE_SIZE;
    Ok(ForgeLabelList {
        labels: raw
            .into_iter()
            .filter_map(|l| ForgeLabel::parse(l.name, l.color.as_deref()))
            .collect(),
        truncated,
    })
}

fn header_str<'h>(headers: &'h reqwest::header::HeaderMap, name: &str) -> Option<&'h str> {
    headers.get(name)?.to_str().ok()
}

fn header_i64(headers: &reqwest::header::HeaderMap, name: &str) -> Option<i64> {
    header_str(headers, name)?.trim().parse().ok()
}

/// One page of an item's discussion, system events removed.
///
/// Two GitLab facts shape everything here:
///
/// 1. **The collection is part of the path.** `/issues/{iid}/notes` and
///    `/merge_requests/{iid}/notes` are different endpoints over different
///    numbering, so asking the wrong one either 404s or — worse — answers with
///    the discussion of a real item that is not the one on screen.
/// 2. **`notes` is not "comments".** It also carries the system events
///    ("changed the milestone", "assigned to @bob"), which is exactly the
///    difference between `notes` and the `user_notes_count` the row shows. They
///    are dropped HERE so the thread matches the count above it.
///
/// Dropping them locally is why `has_next` comes from `X-Next-Page` rather
/// than from how many rows survived: a page of nothing but system events is
/// empty of comments and still has a discussion behind it.
pub async fn list_notes(
    auth: &ResolvedAuth,
    owner_repo: &str,
    kind: ForgeItemKind,
    iid: i64,
    page: u32,
    per_page: u32,
) -> Result<ForgeCommentList, ForgeError> {
    let repo = super::normalize_repo(owner_repo)
        .ok_or_else(|| ForgeError::Invalid(format!("bad repository path: {owner_repo}")))?;
    let project = project_ref(owner_repo)?;
    if iid <= 0 {
        return Err(ForgeError::Invalid(format!("bad work item number: {iid}")));
    }
    let collection = collection_of(kind);
    // Ascending, said out loud: GitLab's own default for notes is DESCENDING,
    // so without this the thread would read backwards and "load more" would
    // append older comments under newer ones.
    let url = format!(
        "{}/projects/{project}/{collection}/{iid}/notes\
         ?order_by=created_at&sort=asc&page={page}&per_page={per_page}",
        auth.api_base
    );
    let response = api_get(auth, &url).await?;
    let has_next =
        header_str(response.headers(), "x-next-page").is_some_and(|v| !v.trim().is_empty());
    let raw: Vec<RawNote> = response
        .json()
        .await
        .map_err(|e| ForgeError::Network(format!("bad notes payload: {e}")))?;

    let anchor = NoteAnchor {
        origin: web_origin(auth),
        repo,
        collection,
        iid,
    };
    let comments = raw
        .into_iter()
        .filter(|note| !note.system)
        .map(|note| note.into_comment(&anchor))
        .collect();

    Ok(ForgeCommentList {
        comments,
        page,
        per_page,
        has_next,
    })
}

/// One merge request by `iid` — what turns "!12" into something checkoutable.
///
/// A merge request opened from a fork reports only the numeric id of its
/// source project, so the fork's path is resolved here (one extra request, and
/// only for forks) — that name goes straight into the refusal the user reads,
/// and "project 4711" would be a worse answer than the truth.
pub async fn get_merge_request(
    auth: &ResolvedAuth,
    owner_repo: &str,
    iid: i64,
) -> Result<ForgePr, ForgeError> {
    let project = project_ref(owner_repo)?;
    if iid <= 0 {
        return Err(ForgeError::Invalid(format!("bad work item number: {iid}")));
    }
    let url = format!("{}/projects/{project}/merge_requests/{iid}", auth.api_base);
    let raw: RawMergeRequest = api_get(auth, &url)
        .await?
        .json()
        .await
        .map_err(|e| ForgeError::Network(format!("bad merge request payload: {e}")))?;
    let foreign_project = raw
        .source_project_id
        .filter(|src| Some(*src) != raw.target_project_id);
    let mut pr = map_merge_request(raw, owner_repo);
    if let Some(id) = foreign_project {
        if let Some(path) = project_path(auth, id).await {
            pr.head_repo = path;
        }
    }
    Ok(pr)
}

/// Merge requests whose source branch is `source_branch`, in ANY state — a
/// merged or closed one is exactly what recovery must be able to see.
///
/// Fork sources keep their placeholder head repository here: this list only
/// ever feeds the four-way match, which refuses anything that is not the
/// source project anyway, so resolving each fork's real path would be a
/// request per row spent on rows that cannot be adopted.
pub async fn find_merge_requests(
    auth: &ResolvedAuth,
    owner_repo: &str,
    source_branch: &str,
) -> Result<Vec<ForgePr>, ForgeError> {
    let project = project_ref(owner_repo)?;
    let url = format!(
        "{}/projects/{project}/merge_requests?source_branch={}&state=all&per_page=100",
        auth.api_base,
        urlencode_query(source_branch)
    );
    let raw: Vec<RawMergeRequest> = api_get(auth, &url)
        .await?
        .json()
        .await
        .map_err(|e| ForgeError::Network(format!("bad merge requests payload: {e}")))?;
    Ok(raw
        .into_iter()
        .map(|m| map_merge_request(m, owner_repo))
        .collect())
}

// ── writes ──────────────────────────────────────────────────────────────────

/// `POST /projects/{p}/merge_requests`.
///
/// GitLab has no `draft` parameter: a draft IS a title starting with `Draft:`,
/// which is also how its UI toggles the state. Prefixing is therefore the
/// supported way to open one, not a workaround.
pub async fn create_merge_request(
    auth: &ResolvedAuth,
    owner_repo: &str,
    req: &NewPullRequest<'_>,
) -> Result<ForgePr, ForgeError> {
    let project = project_ref(owner_repo)?;
    let url = format!("{}/projects/{project}/merge_requests", auth.api_base);
    let title = if req.draft && !is_draft_title(req.title) {
        format!("Draft: {}", req.title)
    } else {
        req.title.to_string()
    };
    let body = serde_json::json!({
        "source_branch": req.head,
        "target_branch": req.base,
        "title": title,
        "description": req.body,
    });
    let raw: RawMergeRequest = api_post(auth, &url, &body)
        .await?
        .json()
        .await
        .map_err(|e| ForgeError::Network(format!("bad merge request payload: {e}")))?;
    Ok(map_merge_request(raw, owner_repo))
}

/// `POST /projects/{p}/{issues|merge_requests}/{iid}/notes`.
///
/// The collection is part of the path here — unlike GitHub, where a pull
/// request is an issue and one endpoint serves both. Posting an issue note to
/// the merge-request collection (or the reverse) is a 404 against a number
/// that may well exist in the other collection.
pub async fn create_note(
    auth: &ResolvedAuth,
    owner_repo: &str,
    kind: ForgeItemKind,
    iid: i64,
    body: &str,
) -> Result<ForgeComment, ForgeError> {
    let repo = super::normalize_repo(owner_repo)
        .ok_or_else(|| ForgeError::Invalid(format!("bad repository path: {owner_repo}")))?;
    let project = project_ref(owner_repo)?;
    if iid <= 0 {
        return Err(ForgeError::Invalid(format!("bad work item number: {iid}")));
    }
    let collection = collection_of(kind);
    let url = format!(
        "{}/projects/{project}/{collection}/{iid}/notes",
        auth.api_base
    );
    let created: RawNote = api_post(auth, &url, &serde_json::json!({ "body": body }))
        .await?
        .json()
        .await
        .map_err(|e| ForgeError::Network(format!("bad note payload: {e}")))?;
    // Through the same mapper the READER uses, anchor included: a comment that
    // came back from the composer and the same comment on the next page of the
    // thread have to be one object, or the optimistic append shows a second
    // copy of what was just posted.
    Ok(created.into_comment(&NoteAnchor {
        origin: web_origin(auth),
        repo,
        collection,
        iid,
    }))
}

/// Close or reopen one item, and hand back the row as the project now sees it.
///
/// `PUT /projects/{p}/{issues|merge_requests}/{iid}` with a `state_event` — a
/// VERB, not a target state. Note the returned row's labels arrive as bare
/// NAMES: `with_labels_details` is a list-endpoint parameter, so a single item
/// never carries its labels' colours. The panel keeps the ones it already had
/// (see `mergeForgeRowUpdate` in the frontend) rather than dropping every chip
/// to grey on a close.
pub async fn set_item_state(
    auth: &ResolvedAuth,
    owner_repo: &str,
    kind: ForgeItemKind,
    iid: i64,
    action: ForgeStateAction,
) -> Result<ForgeIssueRow, ForgeError> {
    let project = project_ref(owner_repo)?;
    if iid <= 0 {
        return Err(ForgeError::Invalid(format!("bad work item number: {iid}")));
    }
    let collection = collection_of(kind);
    let url = format!("{}/projects/{project}/{collection}/{iid}", auth.api_base);
    let raw: RawItem = api_put(
        auth,
        &url,
        &serde_json::json!({ "state_event": action.gitlab_event() }),
    )
    .await?
    .json()
    .await
    .map_err(|e| ForgeError::Network(format!("bad item payload: {e}")))?;
    Ok(raw.into_row(kind == ForgeItemKind::Change))
}

/// `POST /projects/{p}/issues` — open an issue, and hand back its row.
///
/// Labels are COMMA-JOINED, which is lossless: GitLab forbids commas in label
/// titles, so no name can be split by this (the same property the list filter
/// leans on).
pub async fn create_issue(
    auth: &ResolvedAuth,
    owner_repo: &str,
    draft: &ResolvedNewIssue,
) -> Result<ForgeIssueRow, ForgeError> {
    let project = project_ref(owner_repo)?;
    let url = format!("{}/projects/{project}/issues", auth.api_base);
    let mut payload = serde_json::json!({ "title": draft.title });
    if let Some(body) = draft.body.as_deref() {
        payload["description"] = serde_json::Value::String(body.to_string());
    }
    if !draft.labels.is_empty() {
        payload["labels"] = serde_json::Value::String(draft.labels.join(","));
    }
    let raw: RawItem = api_post(auth, &url, &payload)
        .await?
        .json()
        .await
        .map_err(|e| ForgeError::Network(format!("bad issue payload: {e}")))?;
    Ok(raw.into_row(false))
}

// ── proposed changes ────────────────────────────────────────────────────────

/// One merge request's branches, size and CI.
///
/// Two requests: the merge request itself, then its head pipeline's jobs.
/// Almost every counter GitHub's pull object carries is simply absent here —
/// GitLab reports neither additions/deletions nor a commit count on a merge
/// request — so those stay `None` rather than being invented from a diff this
/// call deliberately does not fetch.
pub async fn change_detail(
    auth: &ResolvedAuth,
    owner_repo: &str,
    iid: i64,
) -> Result<ForgeChangeDetail, ForgeError> {
    let project = project_ref(owner_repo)?;
    if iid <= 0 {
        return Err(ForgeError::Invalid(format!("bad work item number: {iid}")));
    }
    let url = format!("{}/projects/{project}/merge_requests/{iid}", auth.api_base);
    let raw: RawMergeRequestDetail = api_get(auth, &url)
        .await?
        .json()
        .await
        .map_err(|e| ForgeError::Network(format!("bad merge request payload: {e}")))?;

    let checks = match raw.head_pipeline.as_ref().map(|p| p.id) {
        Some(pipeline) => pipeline_jobs(auth, &project, pipeline).await,
        // GitLab answered and named no pipeline: there is nothing to run, which
        // is a fact — not the same as a request that could not be made.
        None => ForgeCheckList::available(Vec::new()),
    };

    // A fork's real path costs one extra request and is only spent when the
    // source project genuinely differs — the same rule `get_merge_request`
    // follows, and for the same reason: "project 4711" is a worse answer than
    // the truth in the one place a reader needs it.
    let head_repo = match (raw.source_project_id, raw.target_project_id) {
        (Some(source), Some(target)) if source != target => project_path(auth, source).await,
        _ => None,
    };

    Ok(ForgeChangeDetail {
        number: if raw.iid > 0 { raw.iid } else { iid },
        base_ref: raw.target_branch,
        head_ref: raw.source_branch,
        head_repo,
        head_sha: raw
            .diff_refs
            .and_then(|d| d.head_sha)
            .or(raw.sha)
            .filter(|sha| !sha.is_empty()),
        draft: raw.draft || raw.work_in_progress,
        state: display_state(&raw.state),
        mergeable: mergeable(raw.merge_status.as_deref(), raw.has_conflicts),
        // The detailed status where the instance has one (GitLab 15.6+, and it
        // says WHY — `not_approved`, `conflict`, `ci_still_running`), the older
        // three-value one otherwise.
        merge_state: raw
            .detailed_merge_status
            .or(raw.merge_status)
            .filter(|s| !s.is_empty()),
        additions: None,
        deletions: None,
        changed_files: raw.changes_count.as_deref().and_then(exact_changes_count),
        commits: None,
        checks,
    })
}

/// GitLab's merge status, as a tri-state.
///
/// `unchecked` / `checking` / `cannot_be_merged_recheck` all mean the server
/// has not finished working it out — the same "ask again shortly" GitHub
/// answers with a null `mergeable`, and emphatically not "cannot be merged".
fn mergeable(merge_status: Option<&str>, has_conflicts: bool) -> Option<bool> {
    if has_conflicts {
        return Some(false);
    }
    match merge_status? {
        "can_be_merged" => Some(true),
        "cannot_be_merged" => Some(false),
        _ => None,
    }
}

/// `changes_count` as a number, but only when it IS one.
///
/// GitLab sends this as a string and suffixes it with `+` once the diff hits
/// its own limit ("1000+"). Parsing the digits off that would print "1000
/// files" for a change that touches more, so a truncated count is reported as
/// no count at all.
fn exact_changes_count(raw: &str) -> Option<i64> {
    raw.trim().parse::<i64>().ok().filter(|n| *n >= 0)
}

/// The head pipeline's jobs, as checks.
///
/// One request, and its failure is swallowed the same way GitHub's is: a token
/// that cannot read pipelines still reads the merge request perfectly well, and
/// losing the panel over the CI section would be the worse answer.
async fn pipeline_jobs(auth: &ResolvedAuth, project: &str, pipeline: i64) -> ForgeCheckList {
    if pipeline <= 0 {
        return ForgeCheckList::unavailable();
    }
    let url = format!(
        "{}/projects/{project}/pipelines/{pipeline}/jobs?per_page={LABEL_PAGE_SIZE}",
        auth.api_base
    );
    let raw: Option<Vec<RawJob>> =
        async { api_get(auth, &url).await.ok()?.json().await.ok() }.await;
    match raw {
        Some(jobs) => ForgeCheckList::available(
            jobs.into_iter()
                .map(|job| ForgeCheck {
                    id: format!("job-{}", job.id),
                    state: job_state(&job.status),
                    // The stage, which is what turns a column of job names into
                    // a pipeline you can read ("test", "build", "deploy").
                    summary: job.stage.filter(|stage| !stage.trim().is_empty()),
                    url: job.web_url.as_deref().and_then(sanitize_web_url),
                    name: job.name,
                    allow_failure: job.allow_failure,
                })
                .collect(),
        ),
        None => ForgeCheckList::unavailable(),
    }
}

/// GitLab's eleven job statuses in the five the strip draws.
fn job_state(status: &str) -> ForgeCheckState {
    match status {
        "success" => ForgeCheckState::Success,
        "failed" => ForgeCheckState::Failure,
        "running" | "preparing" => ForgeCheckState::Running,
        "created" | "pending" | "scheduled" | "waiting_for_resource" => ForgeCheckState::Queued,
        // canceled, skipped, manual — ran (or deliberately did not) and
        // produced no verdict.
        _ => ForgeCheckState::Neutral,
    }
}

/// One page of the files a merge request touches.
///
/// `/diffs` is the paginated endpoint (GitLab 15.7+); older instances answer
/// 404 and are served from `/changes`, which returns the WHOLE diff in one
/// payload and is sliced locally so the panel's footer behaves the same either
/// way. GitLab counts nothing per file, so the `+`/`−` beside each path is
/// counted off the diff hunk it ships (see [`count_diff_lines`]).
pub async fn list_change_files(
    auth: &ResolvedAuth,
    owner_repo: &str,
    iid: i64,
    page: u32,
    per_page: u32,
) -> Result<ForgeChangedFileList, ForgeError> {
    let project = project_ref(owner_repo)?;
    if iid <= 0 {
        return Err(ForgeError::Invalid(format!("bad work item number: {iid}")));
    }
    let url = format!(
        "{}/projects/{project}/merge_requests/{iid}/diffs?page={page}&per_page={per_page}",
        auth.api_base
    );
    match api_get(auth, &url).await {
        Ok(response) => {
            let has_next =
                header_str(response.headers(), "x-next-page").is_some_and(|v| !v.trim().is_empty());
            let raw: Vec<RawDiff> = response
                .json()
                .await
                .map_err(|e| ForgeError::Network(format!("bad diffs payload: {e}")))?;
            Ok(ForgeChangedFileList {
                files: raw.into_iter().map(RawDiff::into_file).collect(),
                page,
                per_page,
                has_next,
            })
        }
        Err(ForgeError::NotFound) => legacy_change_files(auth, &project, iid, page, per_page).await,
        Err(other) => Err(other),
    }
}

/// The pre-15.7 spelling: one unpaginated payload, paged here.
async fn legacy_change_files(
    auth: &ResolvedAuth,
    project: &str,
    iid: i64,
    page: u32,
    per_page: u32,
) -> Result<ForgeChangedFileList, ForgeError> {
    #[derive(Deserialize)]
    struct RawChanges {
        #[serde(default)]
        changes: Vec<RawDiff>,
    }
    let url = format!(
        "{}/projects/{project}/merge_requests/{iid}/changes",
        auth.api_base
    );
    let raw: RawChanges = api_get(auth, &url)
        .await?
        .json()
        .await
        .map_err(|e| ForgeError::Network(format!("bad changes payload: {e}")))?;
    let total = raw.changes.len();
    // `saturating_mul` rather than a bare product: `page` is clamped to at
    // least 1 but has no ceiling, and a 32-bit overflow here would wrap round
    // to the FIRST page of a list the caller asked to be past the end of.
    let skip = (page as usize)
        .saturating_sub(1)
        .saturating_mul(per_page as usize);
    Ok(ForgeChangedFileList {
        files: raw
            .changes
            .into_iter()
            .skip(skip)
            .take(per_page as usize)
            .map(RawDiff::into_file)
            .collect(),
        page,
        per_page,
        has_next: total > skip.saturating_add(per_page as usize),
    })
}

// ── merging ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct RawProjectMergeSettings {
    /// `merge` | `rebase_merge` | `ff`.
    #[serde(default)]
    merge_method: Option<String>,
    /// `never` | `always` | `default_on` | `default_off`.
    #[serde(default)]
    squash_option: Option<String>,
}

/// GitLab's three project-level merge methods, as the shared strategy.
///
/// This is NOT a choice the caller gets — the API takes no method — so it only
/// ever describes what will happen. Describing it is the point: a project set
/// to `ff` that was offered "Create a merge commit" would be told its history
/// gets a merge commit it will never have.
fn merge_strategy(merge_method: Option<&str>) -> ForgeMergeStrategy {
    match merge_method {
        Some("ff") => ForgeMergeStrategy::FastForward,
        Some("rebase_merge") => ForgeMergeStrategy::RebaseMerge,
        _ => ForgeMergeStrategy::MergeCommit,
    }
}

/// Which merge methods this project permits.
///
/// GitLab's merge endpoint takes NO method: the project's own `merge_method`
/// setting decides between a merge commit, a rebase-merge and a fast-forward,
/// and the caller cannot override it. So the only real choice here is whether
/// to squash first, and the menu is at most two entries deep.
///
/// [`ForgeMergeMethod::Rebase`] is never offered. GitLab does rebase a merge
/// request, but through `PUT /merge_requests/{iid}/rebase` — a different
/// operation, asynchronous, that this module does not call. An entry for it
/// would be an offer to make a request that is never made.
pub async fn merge_options(
    auth: &ResolvedAuth,
    owner_repo: &str,
) -> Result<ForgeMergeOptions, ForgeError> {
    let project = project_ref(owner_repo)?;
    let url = format!("{}/projects/{project}", auth.api_base);
    let raw: RawProjectMergeSettings = api_get(auth, &url)
        .await?
        .json()
        .await
        .map_err(|e| ForgeError::Network(format!("bad project payload: {e}")))?;

    // The presence of EITHER key is what says this payload really is a
    // project's settings — an instance old enough to omit both tells us
    // nothing, and inventing "squash is available" from that would put an entry
    // in the menu that 422s.
    let squash = match (raw.merge_method.as_deref(), raw.squash_option.as_deref()) {
        (None, None) => return Ok(ForgeMergeOptions::unknown()),
        (_, option) => option.unwrap_or("default_off"),
    };
    let strategy = merge_strategy(raw.merge_method.as_deref());
    Ok(match squash {
        // The project REQUIRES squashing: offering "Merge" as well would offer
        // a request GitLab rewrites into the other one.
        "always" => ForgeMergeOptions {
            methods: vec![ForgeMergeMethod::Squash],
            default_method: ForgeMergeMethod::Squash,
            merge_strategy: strategy,
        },
        "never" => ForgeMergeOptions {
            methods: vec![ForgeMergeMethod::Merge],
            default_method: ForgeMergeMethod::Merge,
            merge_strategy: strategy,
        },
        // Both offered; the project says which one the box opens on.
        other => ForgeMergeOptions {
            methods: vec![ForgeMergeMethod::Merge, ForgeMergeMethod::Squash],
            default_method: if other == "default_on" {
                ForgeMergeMethod::Squash
            } else {
                ForgeMergeMethod::Merge
            },
            merge_strategy: strategy,
        },
    })
}

/// Merge one merge request, and hand back the row the project now serves.
///
/// ONE request, unlike GitHub's two: `PUT /merge_requests/{iid}/merge` answers
/// with the merge request itself, so the row the panel adopts comes straight
/// out of the write through the same [`RawItem`] mapper the list uses.
/// `Ok(None)` is therefore only ever "it merged and the answer did not parse",
/// which is not an error — the change has landed, and reporting a failure would
/// invite somebody to try an irreversible operation a second time.
///
/// [`ForgeMergeMethod::Rebase`] is REFUSED rather than quietly treated as a
/// plain merge. GitLab rebases through `PUT /merge_requests/{iid}/rebase`, an
/// asynchronous operation this module does not call, and a caller that asked
/// for one and got a merge commit was told the wrong thing about its own
/// history. The panel never offers it (see [`merge_options`]); this is the
/// guard for every other caller, the server binary's HTTP surface included.
///
/// `head_sha`, when given, is the commit the caller was looking at. GitLab
/// refuses with a 409 if the source branch has moved since — which is the whole
/// point of passing it.
///
/// A refusal keeps GitLab's own words — 405 for a merge request that cannot be
/// merged (closed, draft, pipeline still running under "merge when pipeline
/// succeeds"), 406 for one that conflicts — because `finish` files anything
/// that is not a rate limit or an auth failure as [`ForgeError::Api`] with the
/// body attached.
pub async fn merge_change(
    auth: &ResolvedAuth,
    owner_repo: &str,
    iid: i64,
    method: ForgeMergeMethod,
    head_sha: Option<&str>,
) -> Result<Option<ForgeIssueRow>, ForgeError> {
    let project = project_ref(owner_repo)?;
    if iid <= 0 {
        return Err(ForgeError::Invalid(format!("bad work item number: {iid}")));
    }
    if method == ForgeMergeMethod::Rebase {
        return Err(ForgeError::Invalid(
            "GitLab rebases through its own endpoint; it cannot be a merge method".to_string(),
        ));
    }
    let url = format!(
        "{}/projects/{project}/merge_requests/{iid}/merge",
        auth.api_base
    );
    // `squash` is sent EXPLICITLY either way rather than only when squashing.
    // The project's `squash_option` can default it to on, and omitting the
    // field on a "Merge" would then squash a change whose author asked for the
    // commits to be kept.
    let mut payload = serde_json::json!({ "squash": method == ForgeMergeMethod::Squash });
    if let Some(sha) = head_sha {
        payload["sha"] = serde_json::Value::String(sha.to_string());
    }
    let response = api_put(auth, &url, &payload).await?;

    // Past this line the change HAS landed, so nothing below may return `Err`.
    let raw: Option<RawItem> = response.json().await.ok();
    Ok(raw.map(|raw| raw.into_row(true)))
}

// ── plumbing ────────────────────────────────────────────────────────────────

/// `GET {api_base}/user` → username, cached per `(api_base, account)`.
static LOGIN_CACHE: LazyLock<RwLock<HashMap<String, String>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

async fn current_login(auth: &ResolvedAuth) -> Result<String, ForgeError> {
    let cache_key = format!("{}\n{}", auth.api_base, auth.account_id);
    if let Some(hit) = LOGIN_CACHE
        .read()
        .ok()
        .and_then(|c| c.get(&cache_key).cloned())
    {
        return Ok(hit);
    }
    #[derive(Deserialize)]
    struct User {
        username: String,
    }
    let user: User = api_get(auth, &format!("{}/user", auth.api_base))
        .await?
        .json()
        .await
        .map_err(|e| ForgeError::Network(format!("bad /user payload: {e}")))?;
    if let Ok(mut cache) = LOGIN_CACHE.write() {
        cache.insert(cache_key, user.username.clone());
    }
    Ok(user.username)
}

/// `path_with_namespace` of a project id. Best-effort: it only ever improves
/// the wording of a refusal that has already been decided.
async fn project_path(auth: &ResolvedAuth, project_id: i64) -> Option<String> {
    #[derive(Deserialize)]
    struct RawProject {
        #[serde(default)]
        path_with_namespace: String,
    }
    let url = format!("{}/projects/{project_id}", auth.api_base);
    let project: RawProject = api_get(auth, &url).await.ok()?.json().await.ok()?;
    Some(project.path_with_namespace).filter(|p| !p.is_empty())
}

pub(crate) async fn api_get(
    auth: &ResolvedAuth,
    url: &str,
) -> Result<reqwest::Response, ForgeError> {
    let response = super::http_client()?
        .get(url)
        .header("PRIVATE-TOKEN", &auth.token)
        .header("User-Agent", "codeg")
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| ForgeError::Network(e.to_string()))?;
    finish(response).await
}

pub(crate) async fn api_post(
    auth: &ResolvedAuth,
    url: &str,
    body: &serde_json::Value,
) -> Result<reqwest::Response, ForgeError> {
    send(super::http_client()?.post(url), auth, body).await
}

/// Authenticated PUT — how GitLab spells "edit an existing thing" (GitHub uses
/// PATCH for the same operation; neither accepts the other's method).
pub(crate) async fn api_put(
    auth: &ResolvedAuth,
    url: &str,
    body: &serde_json::Value,
) -> Result<reqwest::Response, ForgeError> {
    send(super::http_client()?.put(url), auth, body).await
}

async fn send(
    request: reqwest::RequestBuilder,
    auth: &ResolvedAuth,
    body: &serde_json::Value,
) -> Result<reqwest::Response, ForgeError> {
    let response = request
        .header("PRIVATE-TOKEN", &auth.token)
        .header("User-Agent", "codeg")
        .header("Accept", "application/json")
        .json(body)
        .send()
        .await
        .map_err(|e| ForgeError::Network(e.to_string()))?;
    finish(response).await
}

/// Success through, everything else classified. GitLab signals its rate limit
/// with 429 (and `Retry-After`), never with the 403-plus-quota-header shape
/// GitHub uses — so a 403 here really is about the credential.
async fn finish(response: reqwest::Response) -> Result<reqwest::Response, ForgeError> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status().as_u16();
    let retry_after = response
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    Err(match status {
        429 => ForgeError::RateLimited { retry_after },
        401 | 403 => ForgeError::Auth(format!("GitLab returned {status}")),
        404 => ForgeError::NotFound,
        _ => {
            let message = response
                .text()
                .await
                .unwrap_or_default()
                .chars()
                .take(300)
                .collect();
            ForgeError::Api { status, message }
        }
    })
}

/// The project as ONE percent-encoded path segment. `normalize_repo` runs
/// first, so what gets encoded has already been checked for URL
/// metacharacters — the encoding is for the slashes of a subgroup path, not a
/// substitute for validation.
fn project_ref(owner_repo: &str) -> Result<String, ForgeError> {
    let repo = super::normalize_repo(owner_repo)
        .ok_or_else(|| ForgeError::Invalid(format!("bad repository path: {owner_repo}")))?;
    Ok(urlencode_path(&repo))
}

fn collection_of(kind: ForgeItemKind) -> &'static str {
    match kind {
        ForgeItemKind::Issue => "issues",
        ForgeItemKind::Change => "merge_requests",
    }
}

/// Our state filter in GitLab's vocabulary. "closed" asks for everything on
/// the merge-request collection because GitLab's own `closed` excludes merged
/// ones — see [`keeps`] for the other half.
fn wire_state(tab: super::ForgeTab, state: &str) -> &'static str {
    match (tab, state) {
        (_, "open") => "opened",
        (super::ForgeTab::Prs, "closed") => "all",
        (_, "closed") => "closed",
        _ => "all",
    }
}

/// Whether a row survives the filter the API could not fully express. Mirrors
/// [`display_state`]: `locked` DISPLAYS as open, so the closed tab must drop it
/// too — otherwise the merged-inclusive query leaks a row the icon then draws
/// as open into a list of closed ones.
fn keeps(requested: &str, actual: &str) -> bool {
    match requested {
        "closed" => !matches!(actual, "opened" | "locked"),
        _ => true,
    }
}

/// GitLab's `opened`/`locked`/`merged`/`closed` in the three words the rest of
/// codeg (and the workbench row's icon) understands. `merged` survives as
/// itself: it is a real state here, unlike GitHub where it has to be inferred.
fn display_state(state: &str) -> String {
    match state {
        "opened" | "locked" => "open".to_string(),
        "merged" => "merged".to_string(),
        _ => "closed".to_string(),
    }
}

/// Whether a title already declares itself a draft, so it is not prefixed
/// twice. `get(..6)` rather than `[..6]`: a title is arbitrary user text, and
/// slicing bytes through the middle of a code point PANICS — six bytes lands
/// mid-character on plenty of real titles.
fn is_draft_title(title: &str) -> bool {
    title
        .trim_start()
        .get(..6)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("draft:"))
}

fn map_merge_request(raw: RawMergeRequest, owner_repo: &str) -> ForgePr {
    let same_project = match (raw.source_project_id, raw.target_project_id) {
        (Some(src), Some(dst)) => src == dst,
        // A payload that does not say cannot be shown to be the source
        // project, and "unknown" must not read as "ours".
        _ => false,
    };
    ForgePr {
        number: raw.iid,
        html_url: raw.web_url,
        state: display_state(&raw.state),
        merged: raw.state == "merged",
        // `diff_refs` is the precise head of the diff under review and is
        // present on the single-merge-request payload; the list payload only
        // carries `sha`, which is the same commit.
        head_sha: raw
            .diff_refs
            .and_then(|d| d.head_sha)
            .filter(|s| !s.is_empty())
            .or(raw.sha)
            .unwrap_or_default(),
        head_ref: raw.source_branch,
        head_repo: if same_project {
            owner_repo.to_string()
        } else {
            // Deliberately not a repository path: `same_repo` rejects it, so
            // every gate reads this as "somewhere else" without having to
            // spend a request to learn the fork's name.
            format!("project-{}", raw.source_project_id.unwrap_or_default())
        },
        base_ref: raw.target_branch,
    }
}

#[derive(Debug, Deserialize)]
struct RawItem {
    iid: i64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    state: String,
    /// Merge requests only. GitLab renamed `work_in_progress` to `draft`; old
    /// self-managed instances still send only the former, so both are read.
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    work_in_progress: bool,
    #[serde(default)]
    labels: Vec<RawItemLabel>,
    #[serde(default)]
    author: Option<RawUser>,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    web_url: String,
    /// Human comments. GitLab's own name for it: `notes` also covers the
    /// system events ("changed the milestone"), which nobody wants counted.
    #[serde(default)]
    user_notes_count: i64,
}

impl RawItem {
    /// One issue or merge request, as the workbench row.
    ///
    /// `is_pr` is an argument because the collection the payload came from is
    /// the only thing that says so — an item carries no field distinguishing
    /// the two, and the draft flag only means anything for a merge request.
    fn into_row(self, is_pr: bool) -> ForgeIssueRow {
        ForgeIssueRow {
            is_pr,
            number: self.iid,
            title: self.title,
            body: self.description.map(|b| truncate_chars(&b, BODY_CAP)),
            state: display_state(&self.state),
            draft: is_pr && (self.draft || self.work_in_progress),
            labels: self
                .labels
                .into_iter()
                .filter_map(RawItemLabel::into_label)
                .collect(),
            author_avatar: self
                .author
                .as_ref()
                .and_then(|a| a.avatar_url.as_deref())
                .and_then(sanitize_web_url),
            author: self.author.map(|a| a.username),
            updated_at: self.updated_at,
            html_url: self.web_url,
            comments: self.user_notes_count,
        }
    }
}

/// A label on a collection item, in either shape GitLab may send it.
///
/// With `with_labels_details=true` (which the list URL asks for) each entry is
/// an object carrying the colour; without it — and on any instance old enough
/// to ignore the parameter — it is the bare name. Reading both is what keeps a
/// self-managed GitLab listing at all: a hard-coded object shape would turn
/// every row on such an instance into a deserialization failure.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawItemLabel {
    Name(String),
    Detail {
        #[serde(default)]
        name: String,
        #[serde(default)]
        color: Option<String>,
    },
}

impl RawItemLabel {
    fn into_label(self) -> Option<ForgeLabel> {
        match self {
            Self::Name(name) => ForgeLabel::parse(name, None),
            Self::Detail { name, color } => ForgeLabel::parse(name, color.as_deref()),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawUser {
    #[serde(default)]
    username: String,
    /// GitLab hands back a gravatar.com URL for accounts that never uploaded
    /// one, so this can point somewhere outside the instance entirely — the
    /// avatar falls back to initials when it does not load.
    #[serde(default)]
    avatar_url: Option<String>,
}

/// One entry of a `notes` collection.
///
/// `system` is the load-bearing field: GitLab files "changed the milestone"
/// and "assigned to @bob" as notes too, and they are what `user_notes_count`
/// (the number the row shows) leaves out.
#[derive(Debug, Deserialize)]
struct RawNote {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    body: Option<String>,
    /// Defaults to FALSE, which is the safe direction to be wrong in: a
    /// payload missing the field keeps a real comment rather than hiding one.
    #[serde(default)]
    system: bool,
    #[serde(default)]
    author: Option<RawUser>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
}

/// Everything needed to build a note's permalink. GitLab notes carry no web
/// URL of their own, so the anchor on the item's own page is the only link a
/// human can follow — and it is assembled from four values that all four
/// callers (reader and composer, issue and merge request) must agree on.
struct NoteAnchor {
    origin: String,
    /// Lowercased `owner/repo`, as it appears in a web path.
    repo: String,
    /// `issues` | `merge_requests` — the item's WEB segment, which is the same
    /// word as its API collection.
    collection: &'static str,
    iid: i64,
}

impl RawNote {
    fn into_comment(self, anchor: &NoteAnchor) -> ForgeComment {
        ForgeComment {
            author: ForgeComment::author_name(self.author.as_ref().map(|a| a.username.clone())),
            author_avatar: self
                .author
                .as_ref()
                .and_then(|a| a.avatar_url.as_deref())
                .and_then(sanitize_web_url),
            body: truncate_chars(self.body.as_deref().unwrap_or_default(), BODY_CAP),
            updated_at: ForgeComment::edited_at(self.created_at.as_deref(), self.updated_at),
            created_at: self.created_at,
            html_url: Some(format!(
                "{}/{}/-/{}/{}#note_{}",
                anchor.origin, anchor.repo, anchor.collection, anchor.iid, self.id
            )),
            id: self.id.to_string(),
        }
    }
}

/// The SINGLE merge request payload, which carries a good deal the list one
/// does not: the merge status, a head pipeline, and a `changes_count`.
#[derive(Debug, Deserialize)]
struct RawMergeRequestDetail {
    #[serde(default)]
    iid: i64,
    #[serde(default)]
    state: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    work_in_progress: bool,
    #[serde(default)]
    source_branch: String,
    #[serde(default)]
    target_branch: String,
    #[serde(default)]
    sha: Option<String>,
    #[serde(default)]
    diff_refs: Option<RawDiffRefs>,
    #[serde(default)]
    source_project_id: Option<i64>,
    #[serde(default)]
    target_project_id: Option<i64>,
    /// `can_be_merged` | `cannot_be_merged` | `unchecked` | `checking` |
    /// `cannot_be_merged_recheck`.
    #[serde(default)]
    merge_status: Option<String>,
    /// GitLab 15.6+, and it says WHY (`not_approved`, `conflict`,
    /// `ci_still_running`) — absent on older instances.
    #[serde(default)]
    detailed_merge_status: Option<String>,
    #[serde(default)]
    has_conflicts: bool,
    /// A STRING, and suffixed with `+` once the diff hits GitLab's own limit.
    #[serde(default)]
    changes_count: Option<String>,
    #[serde(default)]
    head_pipeline: Option<RawPipeline>,
}

#[derive(Debug, Deserialize)]
struct RawPipeline {
    #[serde(default)]
    id: i64,
}

#[derive(Debug, Deserialize)]
struct RawJob {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    stage: Option<String>,
    #[serde(default)]
    status: String,
    #[serde(default)]
    web_url: Option<String>,
    /// A job the pipeline is allowed to fail on. Worth carrying: a red
    /// indicator that cannot block anything is a different fact from one that
    /// can, and treating them alike sends people to look at the wrong job.
    #[serde(default)]
    allow_failure: bool,
}

/// One file's entry in `/diffs` (and, identically, in the legacy `/changes`).
#[derive(Debug, Deserialize)]
struct RawDiff {
    #[serde(default)]
    old_path: String,
    #[serde(default)]
    new_path: String,
    #[serde(default)]
    new_file: bool,
    #[serde(default)]
    renamed_file: bool,
    #[serde(default)]
    deleted_file: bool,
    /// The unified diff, as text. GitLab counts nothing per file, so this is
    /// where the `+`/`−` numbers come from — and an EMPTY one is how binary
    /// content arrives (there is nothing textual to send).
    #[serde(default)]
    diff: String,
}

impl RawDiff {
    fn into_file(self) -> ForgeChangedFile {
        let status = if self.new_file {
            ForgeFileStatus::Added
        } else if self.deleted_file {
            ForgeFileStatus::Removed
        } else if self.renamed_file {
            ForgeFileStatus::Renamed
        } else {
            ForgeFileStatus::Modified
        };
        // No hunk to count. Binary content arrives this way — and so does an
        // empty file, which from here is indistinguishable and equally has no
        // line counts to show.
        let binary = self.diff.trim().is_empty();
        let (additions, deletions) = count_diff_lines(&self.diff);
        ForgeChangedFile {
            // The path AFTER the change; GitLab repeats it in `old_path` for a
            // deletion, so this is right for every status.
            path: if self.new_path.is_empty() {
                self.old_path.clone()
            } else {
                self.new_path
            },
            previous_path: (status == ForgeFileStatus::Renamed && !self.old_path.is_empty())
                .then_some(self.old_path),
            status,
            additions: (!binary).then_some(additions),
            deletions: (!binary).then_some(deletions),
            binary,
            // The same text the counters above were counted off. Blank is
            // already what `binary` keys off here, so the two agree by
            // construction: a row with no counts also has nothing to open.
            patch: (!binary).then_some(self.diff),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawMergeRequest {
    iid: i64,
    #[serde(default)]
    web_url: String,
    /// `opened` | `locked` | `merged` | `closed`.
    #[serde(default)]
    state: String,
    #[serde(default)]
    sha: Option<String>,
    #[serde(default)]
    diff_refs: Option<RawDiffRefs>,
    #[serde(default)]
    source_branch: String,
    #[serde(default)]
    target_branch: String,
    #[serde(default)]
    source_project_id: Option<i64>,
    #[serde(default)]
    target_project_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RawDiffRefs {
    #[serde(default)]
    head_sha: Option<String>,
}
