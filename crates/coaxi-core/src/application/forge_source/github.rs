//! GitHub REST reads for the workbench. Four hard-won rules are load-bearing
//! here (see `.docs/architecture/2026-08-17-Orca-GitHub-GitLab集成分析.md`):
//!
//! 1. The list comes from `/search/issues`, NOT `/repos/{o}/{r}/issues`. This
//!    is what github.com's own issue list is backed by, and the reason is
//!    pagination: `/issues` serves issues and pull requests from one feed with
//!    no way to ask for one kind, so splitting them client-side made every
//!    page sparse ("API page 3" was not "Issues page 3") and no total existed.
//!    `is:issue` / `is:pr` filters server-side and `total_count` comes back,
//!    which is what makes real page numbers possible.
//! 2. `/pulls` is still never an option for the PR tab: it silently ignores
//!    `assignee`/`labels` filters (HTTP 200, unfiltered rows).
//! 3. Search costs more than it looks: a SEPARATE 30-requests-per-minute quota
//!    (core is 5000/hour), and only the first [`SEARCH_RESULT_CAP`] results are
//!    reachable — the same ceiling github.com shows. `incomplete_results` means
//!    the query timed out and the page is partial; it is surfaced, not hidden.
//! 4. `assignee=@me` is a 422 on `/issues`; the literal login is resolved via
//!    `GET /user` (cached per api_base+account) and reused for the `assignee:`
//!    qualifier here.
//!
//! One limit is deliberately NOT worked around: `q` may not exceed 256
//! characters. Ten long label names plus a full-length text filter can pass it,
//! and GitHub answers 422 with a message that says exactly that — which is
//! surfaced. Silently dropping the last few qualifiers would instead show a
//! list that quietly ignores part of what the user asked for.

use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

use serde::Deserialize;

use super::auth::ResolvedAuth;
use super::{
    BODY_CAP, ForgeChangeDetail, ForgeChangedFile, ForgeChangedFileList, ForgeCheck,
    ForgeCheckList, ForgeCheckState, ForgeComment, ForgeCommentList, ForgeError, ForgeFileStatus,
    ForgeIssueList, ForgeIssueRow, ForgeItemKind, ForgeLabel, ForgeLabelList, ForgeMergeMethod,
    ForgeMergeOptions, ForgeMergeStrategy, ForgeProvider, ForgeStateAction, ForgeTab,
    LABEL_PAGE_SIZE, ListIssuesRequest, ResolvedNewIssue, sanitize_web_url, truncate_chars,
    urlencode_query, validate_state_filter,
};

/// How deep search pagination goes, per GitHub's documented limit. Beyond this
/// the API errors instead of paging, so `has_next` has to stop here — the same
/// ceiling the github.com issue list has.
pub(crate) const SEARCH_RESULT_CAP: i64 = 1000;

#[derive(Debug, Deserialize)]
struct SearchPage {
    #[serde(default)]
    total_count: i64,
    #[serde(default)]
    incomplete_results: bool,
    #[serde(default)]
    items: Vec<RawIssue>,
}

#[derive(Debug, Deserialize)]
struct RawIssue {
    number: i64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    state: String,
    #[serde(default)]
    draft: bool,
    /// Human comments; excludes the timeline's system events.
    #[serde(default)]
    comments: i64,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    user: Option<RawUser>,
    #[serde(default)]
    labels: Vec<RawLabel>,
    /// Presence of this key is what makes a search hit a pull request.
    #[serde(default)]
    pull_request: Option<RawPullRequestRef>,
    /// Set only on a PULL object — `PATCH /pulls/{n}` answers with one, where
    /// the merge stamp sits at the top level instead of under `pull_request`.
    /// An issue never carries it.
    #[serde(default)]
    merged_at: Option<String>,
}

impl RawIssue {
    /// One issue or pull, as the workbench row.
    ///
    /// `is_pr` is an ARGUMENT rather than `pull_request.is_some()`, because the
    /// two endpoints that answer with this shape disagree about it: a search
    /// hit for a pull request carries the `pull_request` key, and the pull
    /// object `PATCH /pulls/{n}` returns does not carry it at all. Deriving it
    /// would turn every state change on a pull request into a row that claims
    /// to be an issue — wrong glyph, wrong link, wrong comment collection.
    fn into_row(self, is_pr: bool) -> ForgeIssueRow {
        let merged = self.merged_at.is_some()
            || self
                .pull_request
                .as_ref()
                .is_some_and(|p| p.merged_at.is_some());
        let (author, author_avatar) = match self.user {
            Some(user) => (
                Some(user.login),
                user.avatar_url.as_deref().and_then(sanitize_web_url),
            ),
            None => (None, None),
        };
        ForgeIssueRow {
            is_pr,
            number: self.number,
            title: self.title,
            body: self.body.map(|b| truncate_chars(&b, BODY_CAP)),
            state: if merged {
                "merged".to_string()
            } else {
                self.state
            },
            draft: is_pr && self.draft,
            labels: self
                .labels
                .into_iter()
                .filter_map(|l| ForgeLabel::parse(l.name, l.color.as_deref()))
                .collect(),
            author,
            author_avatar,
            updated_at: self.updated_at,
            html_url: self.html_url,
            comments: self.comments,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawPullRequestRef {
    /// Set only once the pull request is merged. GitHub has no `merged` STATE
    /// — a merged pull request reports `state: "closed"` like any other — so
    /// this timestamp is the only thing that tells the two apart.
    #[serde(default)]
    merged_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawUser {
    login: String,
    /// Shipped with every list row, so the panel's author avatar costs no
    /// request of its own. Sanitized on the way out, never here.
    #[serde(default)]
    avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawLabel {
    #[serde(default)]
    name: String,
    /// Six hex digits with NO leading `#` — GitHub's own spelling, which is
    /// why the normalizer accepts the hash rather than requiring it.
    #[serde(default)]
    color: Option<String>,
}

/// A label name as a quoted `label:` qualifier. Always quoted (names carry
/// spaces — `help wanted` — and an unquoted one would split into two terms),
/// with the two characters that end a quoted string escaped.
fn label_qualifier(name: &str) -> String {
    let escaped = name.replace('\\', "\\\\").replace('"', "\\\"");
    format!("label:\"{escaped}\"")
}

/// The user's free text, stripped of query SYNTAX.
///
/// The box is labelled "search title and description", so that is what it has
/// to do: typing `is:closed` must search for those words, not silently widen
/// the list to closed items. `:` and `"` are what form a qualifier and a
/// phrase, and a leading `-` is search's negation — all three go, and runs of
/// whitespace collapse so the result is a clean list of terms (ANDed, which is
/// what a filter box is expected to do).
fn search_terms(raw: &str) -> String {
    raw.split_whitespace()
        .map(|word| word.trim_start_matches(['-', '+']).replace([':', '"'], ""))
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The `q` value for one tab+filter combination.
///
/// Exactly ONE `repo:` qualifier, which is what keeps this immune to the
/// advanced-search change of 2025-09-04 (a space between MULTIPLE `repo:` /
/// `org:` / `user:` qualifiers flipped from OR to AND).
fn search_query(
    repo: &str,
    tab: ForgeTab,
    state: &str,
    assignee: Option<&str>,
    labels: &[String],
    search: Option<&str>,
) -> String {
    let mut q = format!(
        "repo:{repo} is:{}",
        match tab {
            ForgeTab::Issues => "issue",
            ForgeTab::Prs => "pr",
        }
    );
    // "all" is the absence of a state qualifier, not a value search understands.
    if state == "open" || state == "closed" {
        q.push_str(&format!(" state:{state}"));
    }
    if let Some(login) = assignee {
        q.push_str(&format!(" assignee:{login}"));
    }
    for label in labels {
        q.push(' ');
        q.push_str(&label_qualifier(label));
    }
    if let Some(raw) = search {
        let terms = search_terms(raw);
        if !terms.is_empty() {
            // Scope said out loud. Search's DEFAULT for free text is title,
            // body AND comments, while the box promises "title and
            // description" — without this qualifier a hit whose only match
            // lives in a comment appears with nothing in it to see, and
            // inflates the count and the page numbers with it. GitLab's
            // `search` already defaults to title+description; this is what
            // makes GitHub agree with it.
            q.push_str(" in:title,body ");
            q.push_str(&terms);
        }
    }
    q
}

pub async fn list_issues(
    conn_auth: &ResolvedAuth,
    req: &ListIssuesRequest,
) -> Result<ForgeIssueList, ForgeError> {
    let repo = super::normalize_repo(&req.owner_repo)
        .ok_or_else(|| ForgeError::Invalid(format!("bad repository path: {}", req.owner_repo)))?;
    validate_state_filter(&req.state)?;
    let (page, per_page) = req.clamped();

    let assignee = if req.assigned_me {
        Some(current_login(conn_auth).await?)
    } else {
        None
    };
    let q = search_query(
        &repo,
        req.tab,
        &req.state,
        assignee.as_deref(),
        &req.labels,
        req.search.as_deref(),
    );
    let url = format!(
        "{}/search/issues?q={}&advanced_search=true&sort={}&order={}&page={page}&per_page={per_page}",
        conn_auth.api_base,
        urlencode_query(&q),
        req.sort.field(),
        req.sort.direction(),
    );

    let page_data: SearchPage = api_get(conn_auth, &url)
        .await?
        .json()
        .await
        .map_err(|e| ForgeError::Network(format!("bad search payload: {e}")))?;

    let is_pr = req.tab == ForgeTab::Prs;
    let rows = page_data
        .items
        .into_iter()
        .map(|r| r.into_row(is_pr))
        .collect();

    // Reachable rows, not matching rows: paging past the cap is an error at the
    // API, so a "next" that leads there would be a button into a failure — and
    // so would the numbered page that `total_count` alone implies, which is why
    // the ceiling travels with the page instead of staying local to `has_next`.
    let reachable = page_data.total_count.min(SEARCH_RESULT_CAP);
    Ok(ForgeIssueList {
        rows,
        page,
        per_page,
        total_count: Some(page_data.total_count),
        reachable_count: (page_data.total_count > SEARCH_RESULT_CAP).then_some(SEARCH_RESULT_CAP),
        has_next: i64::from(page) * i64::from(per_page) < reachable,
        incomplete: page_data.incomplete_results,
    })
}

/// The repository's labels, for the workbench's label filter.
///
/// `/repos/{o}/{r}/labels`, NOT a search endpoint: this runs on the core quota
/// (5000/hour), so opening the filter costs nothing against the 30-per-minute
/// budget the list itself lives on.
pub async fn list_labels(
    auth: &ResolvedAuth,
    owner_repo: &str,
) -> Result<ForgeLabelList, ForgeError> {
    let repo = super::normalize_repo(owner_repo)
        .ok_or_else(|| ForgeError::Invalid(format!("bad repository path: {owner_repo}")))?;
    let url = format!(
        "{}/repos/{repo}/labels?per_page={LABEL_PAGE_SIZE}",
        auth.api_base
    );
    let raw: Vec<RawLabel> = api_get(auth, &url)
        .await?
        .json()
        .await
        .map_err(|e| ForgeError::Network(format!("bad labels payload: {e}")))?;
    // A full page means there may be more. Said out loud rather than silently
    // truncated: a filter list that stops at 100 otherwise reads as complete.
    let truncated = raw.len() >= LABEL_PAGE_SIZE;
    Ok(ForgeLabelList {
        labels: raw
            .into_iter()
            .filter_map(|l| ForgeLabel::parse(l.name, l.color.as_deref()))
            .collect(),
        truncated,
    })
}

/// One page of an item's conversation.
///
/// `/repos/{o}/{r}/issues/{n}/comments` serves ISSUES AND PULL REQUESTS alike
/// — a pull request is an issue here — which is why this takes no item kind at
/// all, and why it is also the endpoint the write-back posts to
/// (`deliver::create_issue_comment`). `/pulls/{n}/comments` is a different
/// collection: review comments anchored to a diff line, which are not what the
/// item's `comments` count counts and not what a reader of the thread expects
/// to find in it.
///
/// On the CORE quota (5000/hour), not search's 30-per-minute one — which is
/// what makes opening the detail panel cheap even though the list it sits over
/// is paid for out of the smaller budget.
pub async fn list_comments(
    auth: &ResolvedAuth,
    owner_repo: &str,
    number: i64,
    page: u32,
    per_page: u32,
) -> Result<ForgeCommentList, ForgeError> {
    let repo = super::normalize_repo(owner_repo)
        .ok_or_else(|| ForgeError::Invalid(format!("bad repository path: {owner_repo}")))?;
    if number <= 0 {
        return Err(ForgeError::Invalid(format!(
            "bad work item number: {number}"
        )));
    }
    // Oldest first, which is this endpoint's own default and the only order a
    // conversation reads in. It takes no `sort` parameter, so there is nothing
    // to state — the ascending order is why "load more" appends.
    let url = format!(
        "{}/repos/{repo}/issues/{number}/comments?page={page}&per_page={per_page}",
        auth.api_base
    );
    let response = api_get(auth, &url).await?;
    // The pagination header, never the row count: a full page is not proof of
    // a next one, and GitHub says so explicitly.
    let has_next = has_next_link(response.headers());
    let raw: Vec<RawComment> = response
        .json()
        .await
        .map_err(|e| ForgeError::Network(format!("bad comments payload: {e}")))?;
    Ok(ForgeCommentList {
        comments: raw.into_iter().map(RawComment::into_comment).collect(),
        page,
        per_page,
        has_next,
    })
}

#[derive(Debug, Deserialize)]
struct RawComment {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    user: Option<RawCommentUser>,
}

#[derive(Debug, Deserialize)]
struct RawCommentUser {
    #[serde(default)]
    login: String,
    #[serde(default)]
    avatar_url: Option<String>,
}

impl RawComment {
    fn into_comment(self) -> ForgeComment {
        let (author, author_avatar) = match self.user {
            Some(user) => (
                ForgeComment::author_name(Some(user.login)),
                user.avatar_url.as_deref().and_then(sanitize_web_url),
            ),
            None => (None, None),
        };
        ForgeComment {
            id: self.id.to_string(),
            author,
            author_avatar,
            body: truncate_chars(self.body.as_deref().unwrap_or_default(), BODY_CAP),
            updated_at: ForgeComment::edited_at(self.created_at.as_deref(), self.updated_at),
            created_at: self.created_at,
            html_url: self.html_url.as_deref().and_then(sanitize_web_url),
        }
    }
}

// ── writes ──────────────────────────────────────────────────────────────────

/// `POST /repos/{o}/{r}/issues/{n}/comments` — the SAME collection
/// [`list_comments`] reads, which is what makes an optimistic append honest: a
/// comment posted here is one the next page of the thread will contain.
///
/// A pull request is an issue at GitHub, so this takes no kind. Note it is not
/// `/pulls/{n}/comments`, which is the review-comment collection anchored to a
/// diff line and is neither what the item's `comments` count counts nor what a
/// reader of the thread expects to find in it.
pub async fn create_comment(
    auth: &ResolvedAuth,
    owner_repo: &str,
    number: i64,
    body: &str,
) -> Result<ForgeComment, ForgeError> {
    let repo = super::normalize_repo(owner_repo)
        .ok_or_else(|| ForgeError::Invalid(format!("bad repository path: {owner_repo}")))?;
    if number <= 0 {
        return Err(ForgeError::Invalid(format!(
            "bad work item number: {number}"
        )));
    }
    let url = format!("{}/repos/{repo}/issues/{number}/comments", auth.api_base);
    let raw: RawComment = api_post(auth, &url, &serde_json::json!({ "body": body }))
        .await?
        .json()
        .await
        .map_err(|e| ForgeError::Network(format!("bad comment payload: {e}")))?;
    Ok(raw.into_comment())
}

/// Close or reopen one item, and hand back the row as the forge now sees it.
///
/// Two endpoints, picked by kind. `/issues/{n}` would very likely serve both —
/// a pull request is an issue here — but `/pulls/{n}` is the documented way to
/// change a pull request's state and its response carries `merged_at` at the
/// top level, which is the one field that tells a merged change from a closed
/// one. Guessing wrong on a merged pull request would paint it "closed".
///
/// The returned row is what the panel and the list both adopt, rather than a
/// local flip of `state`: reopening an issue somebody else closed can fail, be
/// silently refused by a lock, or land alongside edits made in the browser —
/// and the row the forge answers with is the only one of those that is true.
pub async fn set_item_state(
    auth: &ResolvedAuth,
    owner_repo: &str,
    kind: ForgeItemKind,
    number: i64,
    action: ForgeStateAction,
) -> Result<ForgeIssueRow, ForgeError> {
    let repo = super::normalize_repo(owner_repo)
        .ok_or_else(|| ForgeError::Invalid(format!("bad repository path: {owner_repo}")))?;
    if number <= 0 {
        return Err(ForgeError::Invalid(format!(
            "bad work item number: {number}"
        )));
    }
    let collection = match kind {
        ForgeItemKind::Issue => "issues",
        ForgeItemKind::Change => "pulls",
    };
    let url = format!("{}/repos/{repo}/{collection}/{number}", auth.api_base);
    let raw: RawIssue = api_patch(
        auth,
        &url,
        &serde_json::json!({ "state": action.github_state() }),
    )
    .await?
    .json()
    .await
    .map_err(|e| ForgeError::Network(format!("bad item payload: {e}")))?;
    Ok(raw.into_row(kind == ForgeItemKind::Change))
}

/// `POST /repos/{o}/{r}/issues` — open an issue, and hand back the row for it.
///
/// The row is what lets the list show the new issue without a refetch, and it
/// is the forge's copy rather than the draft echoed back: labels the repository
/// does not have are dropped by GitHub, not applied, and the number and URL
/// only exist once it has been written.
pub async fn create_issue(
    auth: &ResolvedAuth,
    owner_repo: &str,
    draft: &ResolvedNewIssue,
) -> Result<ForgeIssueRow, ForgeError> {
    let repo = super::normalize_repo(owner_repo)
        .ok_or_else(|| ForgeError::Invalid(format!("bad repository path: {owner_repo}")))?;
    let url = format!("{}/repos/{repo}/issues", auth.api_base);
    let mut payload = serde_json::json!({ "title": draft.title });
    if let Some(body) = draft.body.as_deref() {
        payload["body"] = serde_json::Value::String(body.to_string());
    }
    if !draft.labels.is_empty() {
        payload["labels"] = serde_json::Value::from(draft.labels.clone());
    }
    let raw: RawIssue = api_post(auth, &url, &payload)
        .await?
        .json()
        .await
        .map_err(|e| ForgeError::Network(format!("bad issue payload: {e}")))?;
    Ok(raw.into_row(false))
}

// ── proposed changes ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct RawPull {
    #[serde(default)]
    number: i64,
    #[serde(default)]
    state: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    merged: bool,
    /// Tri-state on purpose — see [`ForgeChangeDetail::mergeable`].
    #[serde(default)]
    mergeable: Option<bool>,
    #[serde(default)]
    mergeable_state: Option<String>,
    #[serde(default)]
    additions: Option<i64>,
    #[serde(default)]
    deletions: Option<i64>,
    #[serde(default)]
    changed_files: Option<i64>,
    #[serde(default)]
    commits: Option<i64>,
    #[serde(default)]
    head: Option<RawPullRef>,
    #[serde(default)]
    base: Option<RawPullRef>,
}

#[derive(Debug, Deserialize)]
struct RawPullRef {
    #[serde(default, rename = "ref")]
    ref_name: String,
    #[serde(default)]
    sha: Option<String>,
    #[serde(default)]
    repo: Option<RawPullRepo>,
}

#[derive(Debug, Deserialize)]
struct RawPullRepo {
    #[serde(default)]
    full_name: Option<String>,
}

/// One pull request's branches, size and CI — everything the panel shows that
/// a list row does not carry.
///
/// Three requests, all on the CORE quota (5000/hour) rather than search's
/// thirty-a-minute one: the pull itself, then its head commit's check runs and
/// its legacy commit statuses. Both check collections are asked for because
/// repositories genuinely use one or the other — GitHub Actions writes check
/// runs, while external CI (and anything still on the older integration API)
/// writes commit statuses, and a panel that read only one would show "no
/// checks" over a red build.
pub async fn change_detail(
    auth: &ResolvedAuth,
    owner_repo: &str,
    number: i64,
) -> Result<ForgeChangeDetail, ForgeError> {
    let repo = super::normalize_repo(owner_repo)
        .ok_or_else(|| ForgeError::Invalid(format!("bad repository path: {owner_repo}")))?;
    if number <= 0 {
        return Err(ForgeError::Invalid(format!(
            "bad work item number: {number}"
        )));
    }
    let url = format!("{}/repos/{repo}/pulls/{number}", auth.api_base);
    let raw: RawPull = api_get(auth, &url)
        .await?
        .json()
        .await
        .map_err(|e| ForgeError::Network(format!("bad pull payload: {e}")))?;

    let head_sha = raw.head.as_ref().and_then(|h| h.sha.clone());
    let checks = match head_sha.as_deref() {
        Some(sha) => head_checks(auth, &repo, sha).await,
        // No head commit to ask about (a pull whose branch was deleted after a
        // merge can answer this way): nothing was asked, so nothing is claimed.
        None => ForgeCheckList::unavailable(),
    };

    // The head repository is shown only when it is somebody ELSE's — a fork is
    // the fact worth a line; "acme/app → acme/app" is noise on every other
    // change. `same_repo` rather than `==`: GitHub answers in canonical casing.
    let head_repo = raw
        .head
        .as_ref()
        .and_then(|h| h.repo.as_ref())
        .and_then(|r| r.full_name.clone())
        .filter(|full| !super::same_repo(full, &repo));

    Ok(ForgeChangeDetail {
        number: if raw.number > 0 { raw.number } else { number },
        base_ref: raw
            .base
            .as_ref()
            .map(|b| b.ref_name.clone())
            .unwrap_or_default(),
        head_ref: raw
            .head
            .as_ref()
            .map(|h| h.ref_name.clone())
            .unwrap_or_default(),
        head_repo,
        head_sha,
        draft: raw.draft,
        // Same normalization the row gets: GitHub has no `merged` state, so a
        // merged pull reports `closed` and only `merged` tells them apart.
        state: if raw.merged {
            "merged".to_string()
        } else {
            raw.state
        },
        mergeable: raw.mergeable,
        merge_state: raw.mergeable_state.filter(|s| !s.is_empty()),
        additions: raw.additions,
        deletions: raw.deletions,
        changed_files: raw.changed_files,
        commits: raw.commits,
        checks,
    })
}

#[derive(Debug, Deserialize)]
struct RawCheckRunPage {
    #[serde(default)]
    check_runs: Vec<RawCheckRun>,
}

#[derive(Debug, Deserialize)]
struct RawCheckRun {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    name: String,
    /// queued | in_progress | completed | waiting | requested | pending
    #[serde(default)]
    status: String,
    /// Only once `status` is `completed`.
    #[serde(default)]
    conclusion: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    output: Option<RawCheckOutput>,
}

#[derive(Debug, Deserialize)]
struct RawCheckOutput {
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawCombinedStatus {
    #[serde(default)]
    statuses: Vec<RawCommitStatus>,
}

#[derive(Debug, Deserialize)]
struct RawCommitStatus {
    #[serde(default)]
    id: i64,
    /// The status' name — `context`, not `name`.
    #[serde(default)]
    context: String,
    /// success | failure | error | pending
    #[serde(default)]
    state: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    target_url: Option<String>,
}

/// The head commit's checks, from BOTH of GitHub's collections.
///
/// Failure is not propagated: a token without `checks:read` still lists the
/// pull request perfectly well, and losing the panel over a section of it would
/// be a worse answer than the section saying it could not look. Which is
/// exactly what [`ForgeCheckList::unavailable`] means — and why it is not the
/// same value as "there are no checks".
///
/// The two collections sit behind two SEPARATE fine-grained permissions
/// ("Checks" and "Commit statuses"), so half an answer is a real outcome: a
/// token granted only the latter is refused by `/check-runs` and handed an
/// empty `200 []` by `/status`. Reporting that as a complete empty list is the
/// original bug in disguise — GitHub Actions is red and the panel says nothing
/// ran — so a half answer comes back as [`ForgeCheckList::partial`].
async fn head_checks(auth: &ResolvedAuth, repo: &str, sha: &str) -> ForgeCheckList {
    // The sha comes from GitHub's own response, but it lands in a URL path, so
    // it is checked like anything else that does.
    if sha.is_empty() || !sha.chars().all(|c| c.is_ascii_alphanumeric()) {
        return ForgeCheckList::unavailable();
    }
    let runs: Option<Vec<ForgeCheck>> = async {
        let url = format!(
            "{}/repos/{repo}/commits/{sha}/check-runs?per_page={LABEL_PAGE_SIZE}",
            auth.api_base
        );
        let page: RawCheckRunPage = api_get(auth, &url).await.ok()?.json().await.ok()?;
        Some(
            page.check_runs
                .into_iter()
                .map(|run| ForgeCheck {
                    id: format!("run-{}", run.id),
                    state: check_run_state(&run.status, run.conclusion.as_deref()),
                    summary: run
                        .output
                        .and_then(|o| o.title)
                        .filter(|title| !title.trim().is_empty()),
                    url: run.html_url.as_deref().and_then(sanitize_web_url),
                    name: run.name,
                    // GitHub has no per-check "allowed to fail" flag; that is a
                    // branch-protection property of the repository, not of the run.
                    allow_failure: false,
                })
                .collect(),
        )
    }
    .await;

    let statuses: Option<Vec<ForgeCheck>> = async {
        let url = format!(
            "{}/repos/{repo}/commits/{sha}/status?per_page={LABEL_PAGE_SIZE}",
            auth.api_base
        );
        let combined: RawCombinedStatus = api_get(auth, &url).await.ok()?.json().await.ok()?;
        Some(
            combined
                .statuses
                .into_iter()
                .map(|status| ForgeCheck {
                    id: format!("status-{}", status.id),
                    state: commit_status_state(&status.state),
                    summary: status
                        .description
                        .filter(|description| !description.trim().is_empty()),
                    url: status.target_url.as_deref().and_then(sanitize_web_url),
                    name: status.context,
                    allow_failure: false,
                })
                .collect(),
        )
    }
    .await;

    if runs.is_none() && statuses.is_none() {
        return ForgeCheckList::unavailable();
    }
    let partial = runs.is_none() || statuses.is_none();
    let mut checks = runs.unwrap_or_default();
    // A repository can have both, and an integration that writes each way
    // writes the same NAME to both — showing it twice reads as two checks that
    // could disagree.
    for status in statuses.unwrap_or_default() {
        if !checks.iter().any(|check| check.name == status.name) {
            checks.push(status);
        }
    }
    if partial {
        ForgeCheckList::partial(checks)
    } else {
        ForgeCheckList::available(checks)
    }
}

/// A check run's `status` × `conclusion`, folded into the one vocabulary.
fn check_run_state(status: &str, conclusion: Option<&str>) -> ForgeCheckState {
    // `conclusion` is only meaningful once the run has completed; a queued run
    // carries `null` there and reading it as "no verdict" would grey out every
    // check that has not started.
    if status != "completed" {
        return match status {
            "in_progress" => ForgeCheckState::Running,
            _ => ForgeCheckState::Queued,
        };
    }
    match conclusion.unwrap_or_default() {
        "success" => ForgeCheckState::Success,
        // `action_required` is a failure that names its own remedy: the change
        // does not go in until somebody does something.
        "failure" | "timed_out" | "startup_failure" | "action_required" => ForgeCheckState::Failure,
        // Neutral, skipped, cancelled, stale — ran (or deliberately did not)
        // and produced no verdict. NOT success: a skipped required check is
        // not a pass.
        _ => ForgeCheckState::Neutral,
    }
}

/// The legacy commit-status vocabulary. `error` and `failure` are different
/// words for the same outcome as far as a strip of indicators is concerned.
fn commit_status_state(state: &str) -> ForgeCheckState {
    match state {
        "success" => ForgeCheckState::Success,
        "failure" | "error" => ForgeCheckState::Failure,
        "pending" => ForgeCheckState::Running,
        _ => ForgeCheckState::Neutral,
    }
}

#[derive(Debug, Deserialize)]
struct RawPullFile {
    #[serde(default)]
    filename: String,
    #[serde(default)]
    previous_filename: Option<String>,
    /// added | removed | modified | renamed | copied | changed | unchanged
    #[serde(default)]
    status: String,
    #[serde(default)]
    additions: i64,
    #[serde(default)]
    deletions: i64,
    /// Absent for a binary file — and also for a diff GitHub considers too
    /// large to inline, which is why its absence alone does not mean binary.
    #[serde(default)]
    patch: Option<String>,
}

/// One page of the files a pull request touches.
///
/// `/pulls/{n}/files`, on the core quota. GitHub serves at most 3000 files
/// across all pages, and stops offering a `rel="next"` there — so the footer
/// simply runs out, which is the same thing github.com's own "Files changed"
/// tab does.
pub async fn list_change_files(
    auth: &ResolvedAuth,
    owner_repo: &str,
    number: i64,
    page: u32,
    per_page: u32,
) -> Result<ForgeChangedFileList, ForgeError> {
    let repo = super::normalize_repo(owner_repo)
        .ok_or_else(|| ForgeError::Invalid(format!("bad repository path: {owner_repo}")))?;
    if number <= 0 {
        return Err(ForgeError::Invalid(format!(
            "bad work item number: {number}"
        )));
    }
    let url = format!(
        "{}/repos/{repo}/pulls/{number}/files?page={page}&per_page={per_page}",
        auth.api_base
    );
    let response = api_get(auth, &url).await?;
    let has_next = has_next_link(response.headers());
    let raw: Vec<RawPullFile> = response
        .json()
        .await
        .map_err(|e| ForgeError::Network(format!("bad pull files payload: {e}")))?;
    Ok(ForgeChangedFileList {
        files: raw
            .into_iter()
            .map(|file| {
                // No patch AND nothing counted on either side: a text file with
                // a diff too large to inline still reports its line counts, so
                // this is the pair that says "there is nothing textual here".
                let binary = file.patch.is_none() && file.additions == 0 && file.deletions == 0;
                ForgeChangedFile {
                    status: file_status(&file.status),
                    previous_path: file
                        .previous_filename
                        .filter(|previous| !previous.is_empty()),
                    path: file.filename,
                    additions: (!binary).then_some(file.additions),
                    deletions: (!binary).then_some(file.deletions),
                    binary,
                    // An empty patch is dropped rather than passed on: a file
                    // GitHub sent `""` for has nothing to open onto, and a row
                    // that offered the reveal anyway would open onto nothing.
                    patch: file.patch.filter(|patch| !patch.is_empty()),
                }
            })
            .collect(),
        page,
        per_page,
        has_next,
    })
}

/// GitHub's seven file statuses, mapped onto the four a reader distinguishes.
/// `copied` reads as an addition (the path is new); `changed` and `unchanged`
/// are modifications with and without content, and neither warrants its own
/// glyph.
fn file_status(raw: &str) -> ForgeFileStatus {
    match raw {
        "added" | "copied" => ForgeFileStatus::Added,
        "removed" => ForgeFileStatus::Removed,
        "renamed" => ForgeFileStatus::Renamed,
        _ => ForgeFileStatus::Modified,
    }
}

// ── merging ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct RawRepoSettings {
    /// All three default to TRUE on a repository, and `#[serde(default)]` would
    /// default them to false — which would answer "no methods permitted" for
    /// any payload that surprised us. `Option` keeps "GitHub did not say"
    /// distinguishable from "GitHub said no".
    #[serde(default)]
    allow_merge_commit: Option<bool>,
    #[serde(default)]
    allow_squash_merge: Option<bool>,
    #[serde(default)]
    allow_rebase_merge: Option<bool>,
}

/// Which merge methods this repository permits.
///
/// `GET /repos/{o}/{r}` — one core-quota request, asked only when the panel is
/// about to offer the button. A repository can forbid any of the three, and
/// GitHub answers a forbidden one with `405 Method Not Allowed` at merge time;
/// a menu built without this would offer entries that can only fail.
///
/// A token that reads the pull request but not the repository's settings is not
/// an error here — [`ForgeMergeOptions::unknown`] is a menu with one safe entry,
/// which is a better answer than no button at all.
pub async fn merge_options(
    auth: &ResolvedAuth,
    owner_repo: &str,
) -> Result<ForgeMergeOptions, ForgeError> {
    let repo = super::normalize_repo(owner_repo)
        .ok_or_else(|| ForgeError::Invalid(format!("bad repository path: {owner_repo}")))?;
    let url = format!("{}/repos/{repo}", auth.api_base);
    let raw: RawRepoSettings = api_get(auth, &url)
        .await?
        .json()
        .await
        .map_err(|e| ForgeError::Network(format!("bad repository payload: {e}")))?;

    // Absent reads as PERMITTED: every one of these defaults to true on a new
    // repository, and an old GitHub Enterprise that omits the key entirely
    // permits all three. Dropping a method GitHub would have accepted is the
    // failure that has no recovery in the UI.
    let allowed = |flag: Option<bool>| flag.unwrap_or(true);
    let methods: Vec<ForgeMergeMethod> = [
        (ForgeMergeMethod::Merge, raw.allow_merge_commit),
        (ForgeMergeMethod::Squash, raw.allow_squash_merge),
        (ForgeMergeMethod::Rebase, raw.allow_rebase_merge),
    ]
    .into_iter()
    .filter(|(_, flag)| allowed(*flag))
    .map(|(method, _)| method)
    .collect();

    Ok(ForgeMergeOptions {
        // A repository CAN have all three turned off (it is then merged only
        // through a merge queue or the command line), and an empty list is the
        // honest answer — the panel reads it as "offer the safe default and let
        // GitHub refuse", which is what would happen anyway.
        default_method: methods.first().copied().unwrap_or(ForgeMergeMethod::Merge),
        methods,
        // Not a question here: GitHub's `merge` writes a merge commit, and the
        // other two shapes are their own methods rather than a repository
        // setting that reinterprets this one (which is GitLab's model).
        merge_strategy: ForgeMergeStrategy::MergeCommit,
    })
}

/// Merge one pull request, and hand back the row the forge now serves.
///
/// Two requests, and the second is why the return is an `Option`. `PUT
/// /pulls/{n}/merge` answers with `{sha, merged, message}` — not with the pull
/// request — so the row the panel and the list adopt has to be re-read. The
/// re-read goes through `RawIssue` exactly as [`set_item_state`] does, because
/// the pull object carries `merged_at` at the TOP level and that timestamp is
/// the only thing that tells a merged pull request from a closed one (GitHub
/// has no merged state; see [`RawIssue::into_row`]).
///
/// `Ok(None)` means IT MERGED AND THE RE-READ FAILED. That is not an error, and
/// making it one was a real bug: the merge is irreversible and already done, so
/// a network blip on the second request would have reported a completed merge
/// as a failure and invited the user to do it again.
///
/// `head_sha`, when given, is the commit the caller was looking at. GitHub
/// refuses with a 409 ("Head branch was modified. Review and try the merge
/// again.") if the branch has moved since — which is the entire point, and a
/// sentence good enough to show as-is.
pub async fn merge_change(
    auth: &ResolvedAuth,
    owner_repo: &str,
    number: i64,
    method: ForgeMergeMethod,
    head_sha: Option<&str>,
) -> Result<Option<ForgeIssueRow>, ForgeError> {
    let repo = super::normalize_repo(owner_repo)
        .ok_or_else(|| ForgeError::Invalid(format!("bad repository path: {owner_repo}")))?;
    if number <= 0 {
        return Err(ForgeError::Invalid(format!(
            "bad work item number: {number}"
        )));
    }
    let url = format!("{}/repos/{repo}/pulls/{number}/merge", auth.api_base);
    let mut payload = serde_json::json!({ "merge_method": method.github_method() });
    if let Some(sha) = head_sha {
        payload["sha"] = serde_json::Value::String(sha.to_string());
    }
    // The response body is read and dropped on purpose: `merged` in it is
    // always true for a 2xx, and everything the panel shows comes from the row
    // below. A 405 (not mergeable, or this method is forbidden here) and a 409
    // (the branch moved) have already become `ForgeError::Api` with GitHub's
    // own sentence in them.
    api_put(auth, &url, &payload).await?;

    // Past this line the change HAS landed, so nothing below may return `Err`.
    let url = format!("{}/repos/{repo}/pulls/{number}", auth.api_base);
    let raw: Option<RawIssue> = async { api_get(auth, &url).await.ok()?.json().await.ok() }.await;
    Ok(raw.map(|raw| raw.into_row(true)))
}

/// Whether GitHub's `Link` header offers a `rel="next"`.
///
/// The alternative — "the page came back full, so there is probably more" —
/// promises a next page that is empty whenever the total is an exact multiple
/// of the page size, which for a page size of 20 is every twentieth thread.
fn has_next_link(headers: &reqwest::header::HeaderMap) -> bool {
    headers
        .get_all("link")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|header| {
            header.split(',').any(|part| {
                // `<url>; rel="next"` — the quotes are what keep this from
                // matching `rel="next-something"` if one ever appeared.
                part.split(';')
                    .skip(1)
                    .any(|param| param.trim().eq_ignore_ascii_case("rel=\"next\""))
            })
        })
}

/// `GET {api_base}/user` → login, cached per `(api_base, account)` — resolving
/// it on every "assigned to me" page would waste a rate-limit point per click.
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
        login: String,
    }
    let user: User = api_get(auth, &format!("{}/user", auth.api_base))
        .await?
        .json()
        .await
        .map_err(|e| ForgeError::Network(format!("bad /user payload: {e}")))?;
    if let Ok(mut cache) = LOGIN_CACHE.write() {
        cache.insert(cache_key, user.login.clone());
    }
    Ok(user.login)
}

/// Authenticated GET with the standard GitHub headers; non-2xx classified into
/// `ForgeError` (rate limits distinguished from plain auth failures).
pub(crate) async fn api_get(
    auth: &ResolvedAuth,
    url: &str,
) -> Result<reqwest::Response, ForgeError> {
    let response = super::http_client()?
        .get(url)
        .header("Authorization", format!("Bearer {}", auth.token))
        .header("User-Agent", "coaxi")
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|e| ForgeError::Network(e.to_string()))?;
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    Err(classify_failure(status.as_u16(), response).await)
}

/// Authenticated POST with the same headers and the same failure taxonomy as
/// [`api_get`]. Writes are never retried here: a retried create could open a
/// duplicate pull request, so the caller decides.
pub(crate) async fn api_post(
    auth: &ResolvedAuth,
    url: &str,
    body: &serde_json::Value,
) -> Result<reqwest::Response, ForgeError> {
    send(super::http_client()?.post(url), auth, body).await
}

/// Authenticated PATCH — how GitHub spells "edit an existing thing", and the
/// only method that changes an item's state.
pub(crate) async fn api_patch(
    auth: &ResolvedAuth,
    url: &str,
    body: &serde_json::Value,
) -> Result<reqwest::Response, ForgeError> {
    send(super::http_client()?.patch(url), auth, body).await
}

/// Authenticated PUT. GitHub reserves this for the handful of endpoints that
/// REPLACE rather than edit — merging a pull request is one — and answers PATCH
/// on them with a 404, so it is not interchangeable with [`api_patch`].
pub(crate) async fn api_put(
    auth: &ResolvedAuth,
    url: &str,
    body: &serde_json::Value,
) -> Result<reqwest::Response, ForgeError> {
    send(super::http_client()?.put(url), auth, body).await
}

/// The headers and failure taxonomy every write shares. Writes are never
/// retried here: a retried create could open a duplicate pull request or post
/// a comment twice, so the caller decides.
async fn send(
    request: reqwest::RequestBuilder,
    auth: &ResolvedAuth,
    body: &serde_json::Value,
) -> Result<reqwest::Response, ForgeError> {
    let response = request
        .header("Authorization", format!("Bearer {}", auth.token))
        .header("User-Agent", "coaxi")
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(body)
        .send()
        .await
        .map_err(|e| ForgeError::Network(e.to_string()))?;
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    Err(classify_failure(status.as_u16(), response).await)
}

async fn classify_failure(status: u16, response: reqwest::Response) -> ForgeError {
    let retry_after = response
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    let primary_exhausted = response
        .headers()
        .get("x-ratelimit-remaining")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.trim() == "0");
    match status {
        429 => ForgeError::RateLimited { retry_after },
        403 if retry_after.is_some() || primary_exhausted => {
            ForgeError::RateLimited { retry_after }
        }
        401 | 403 => ForgeError::Auth(format!("GitHub returned {status}")),
        404 => ForgeError::NotFound,
        _ => {
            let host = response
                .url()
                .host_str()
                .unwrap_or_default()
                .to_ascii_lowercase();
            let message: String = response
                .text()
                .await
                .unwrap_or_default()
                .chars()
                .take(300)
                .collect();
            // A GitLab asked for GitHub Enterprise's `/api/v3` says so in
            // as many words. That is not an API error to report — it is the
            // instance identifying itself, and the only sane response is to
            // believe it and stop calling this host a GitHub.
            if status == 410 && message.contains("API V3 is no longer supported") {
                // Provider identity remains pinned to the Core-resolved account.
                return ForgeError::WrongForge {
                    host,
                    detected: ForgeProvider::GitLab,
                };
            }
            ForgeError::Api { status, message }
        }
    }
}
