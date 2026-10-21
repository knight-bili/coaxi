use super::ForgeProvider;
#[derive(Clone)]
pub struct ResolvedAuth {
    /// Which forge this identity talks to — picks the REST dialect, the auth
    /// header and the endpoint shapes downstream.
    pub provider: ForgeProvider,
    /// Server host (`github.com`, `ghe.corp.com`) — the coordinate system
    /// source keys and git remotes share.
    pub server_host: String,
    /// Derived REST base: `https://api.github.com`, `{server_url}/api/v3`
    /// (GitHub Enterprise) or `{server_url}/api/v4` (GitLab).
    pub api_base: String,
    pub account_id: String,
    /// Login of the pinned account — the username half of the git credential
    /// when this identity pushes. Resolving it here (rather than letting git
    /// pick an account by host) is what makes "the branch is pushed by the
    /// account that triggered the task" true.
    pub username: String,
    /// The account's picture, as the forge reported it when the token was
    /// validated. Display only, and never a reason to fail: an account stored
    /// before avatars were recorded simply has none.
    pub avatar_url: Option<String>,
    pub token: String,
    /// Token scopes as recorded when the account was added, for display only.
    /// Deliberately NOT used as a gate: GitHub's fine-grained tokens report no
    /// scopes at all, so refusing on an empty list would lock out perfectly
    /// good credentials. A token that cannot do the job says so at the API,
    /// and the delivery path turns that into a readable failure.
    pub scopes: Vec<String>,
}

// Hand-written so a stray `{auth:?}` in a log line can never print the token.
impl std::fmt::Debug for ResolvedAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedAuth")
            .field("provider", &self.provider)
            .field("server_host", &self.server_host)
            .field("api_base", &self.api_base)
            .field("account_id", &self.account_id)
            .field("username", &self.username)
            .field("token", &"<redacted>")
            .finish()
    }
}

