//! Codeg delivery wire types (aff1e6c, Apache-2.0). Core owns execution.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ForgePr {
    pub number: i64,
    pub html_url: String,
    /// "open" | "closed" (GitHub has no separate "merged" state).
    pub state: String,
    pub merged: bool,
    pub head_sha: String,
    pub head_ref: String,
    /// `owner/repo` of the head — compared with `same_repo`, never `==`.
    pub head_repo: String,
    pub base_ref: String,
}

/// What to open, once we know nothing suitable exists yet.
#[derive(Debug, Clone)]
pub struct NewPullRequest<'a> {
    pub title: &'a str,
    pub head: &'a str,
    pub base: &'a str,
    pub body: &'a str,
    pub draft: bool,
}
