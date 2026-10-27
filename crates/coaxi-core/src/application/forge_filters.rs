//! Coaxi aff1e6c list/search recipes for the four supported forge providers.
use super::*;
pub(super) fn authorize(
    repository: &Repository,
    call: reqwest::RequestBuilder,
) -> reqwest::RequestBuilder {
    if repository.token.is_empty() {
        call
    } else if repository.provider == "gitlab" {
        call.header("PRIVATE-TOKEN", &repository.token)
    } else if matches!(repository.provider.as_str(), "gitea" | "forgejo") {
        call.header("Authorization", format!("token {}", repository.token))
    } else {
        call.bearer_auth(&repository.token)
    }
}
async fn current_login(repository: &Repository, client: &reqwest::Client) -> Result<String> {
    ensure!(!repository.token.is_empty(), "forge.account_required");
    let mut response = authorize(
        repository,
        client.get(format!("{}/user", repository.api_base)),
    )
    .send()
    .await?;
    ensure!(response.status().is_success(), "forge.account_unavailable");
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            bytes.len() + chunk.len() <= 1024 * 1024,
            "forge.response_too_large"
        );
        bytes.extend_from_slice(&chunk);
    }
    let value: Value = serde_json::from_slice(&bytes)?;
    let login = if repository.provider == "gitlab" {
        value["username"].as_str()
    } else {
        value["login"].as_str()
    }
    .context("forge.account_unavailable")?;
    ensure!(
        !login.is_empty()
            && login.len() <= 256
            && login
                .chars()
                .all(|character| character.is_ascii_alphanumeric()
                    || matches!(character, '-' | '_' | '.' | '@')),
        "forge.invalid_account_login"
    );
    Ok(login.into())
}
pub(super) async fn list_url(
    repository: &Repository,
    request: &ForgeRequest,
    client: &reqwest::Client,
) -> Result<reqwest::Url> {
    let state = request.state.as_deref().unwrap_or("open");
    ensure!(
        ["open", "closed", "all"].contains(&state),
        "forge.invalid_state"
    );
    ensure!(
        request.labels.len() <= 20
            && request
                .labels
                .iter()
                .all(|label| !label.is_empty() && label.chars().count() <= 200),
        "forge.invalid_labels"
    );
    let search = request
        .search
        .as_deref()
        .map(|value| value.trim().chars().take(512).collect::<String>())
        .filter(|value| !value.is_empty());
    let sort = request.sort.as_deref().unwrap_or("newest");
    ensure!(
        [
            "newest",
            "oldest",
            "recently_updated",
            "least_recently_updated"
        ]
        .contains(&sort),
        "forge.invalid_sort"
    );
    let field = if matches!(sort, "recently_updated" | "least_recently_updated") {
        "updated"
    } else {
        "created"
    };
    let direction = if matches!(sort, "oldest" | "least_recently_updated") {
        "asc"
    } else {
        "desc"
    };
    let login = if request.assigned_me {
        Some(current_login(repository, client).await?)
    } else {
        None
    };
    let pull = request.action == "pulls";
    if repository.provider == "github" {
        ensure!(
            repository.owner_repo.split('/').all(|part| !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))),
            "forge.invalid_repository"
        );
        ensure!(request.page <= 20, "forge.search_reachable_limit");
        let mut query = format!(
            "repo:{} is:{}",
            repository.owner_repo,
            if pull { "pr" } else { "issue" }
        );
        if state != "all" {
            query.push_str(&format!(" state:{state}"));
        }
        if let Some(login) = login {
            query.push_str(&format!(" assignee:{login}"));
        }
        for label in &request.labels {
            let escaped = label.replace('\\', "\\\\").replace('"', "\\\"");
            query.push_str(&format!(" label:\"{escaped}\""));
        }
        if let Some(search) = search {
            let terms = search
                .split_whitespace()
                .map(|word| word.trim_start_matches(['-', '+']).replace([':', '"'], ""))
                .filter(|word| !word.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            if !terms.is_empty() {
                query.push_str(" in:title,body ");
                query.push_str(&terms);
            }
        }
        let mut url = reqwest::Url::parse(&format!("{}/search/issues", repository.api_base))?;
        url.query_pairs_mut()
            .append_pair("q", &query)
            .append_pair("advanced_search", "true")
            .append_pair("sort", field)
            .append_pair("order", direction)
            .append_pair("page", &request.page.to_string())
            .append_pair("per_page", "50");
        Ok(url)
    } else if repository.provider == "gitlab" {
        let mut url = repository.endpoint(if pull { "pulls" } else { "issues" }, None, false)?;
        let mut query = url.query_pairs_mut();
        query
            .append_pair("state", if state == "open" { "opened" } else { state })
            .append_pair(
                "order_by",
                if field == "updated" {
                    "updated_at"
                } else {
                    "created_at"
                },
            )
            .append_pair("sort", direction)
            .append_pair("with_labels_details", "true")
            .append_pair("page", &request.page.to_string())
            .append_pair("per_page", "50");
        if let Some(login) = login {
            query.append_pair("assignee_username", &login);
        }
        if !request.labels.is_empty() {
            query.append_pair("labels", &request.labels.join(","));
        }
        if let Some(search) = search {
            query
                .append_pair("search", &search)
                .append_pair("in", "title,description");
        }
        drop(query);
        Ok(url)
    } else {
        let mut url = repository.endpoint("issues", None, false)?;
        let mut query = url.query_pairs_mut();
        query
            .append_pair("state", state)
            .append_pair("type", if pull { "pulls" } else { "issues" })
            .append_pair("page", &request.page.to_string())
            .append_pair("limit", "50");
        if let Some(login) = login {
            query.append_pair("assigned_by", &login);
        }
        if !request.labels.is_empty() {
            query.append_pair("labels", &request.labels.join(","));
        }
        if let Some(search) = search {
            query.append_pair("q", &search);
        }
        drop(query);
        Ok(url)
    }
}
