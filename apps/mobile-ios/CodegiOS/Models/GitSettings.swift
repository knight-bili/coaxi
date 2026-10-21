import Foundation

/// Result of `detect_git` / `test_git_path` — whether git is available and where.
struct GitDetectResult: Decodable, Sendable {
    let installed: Bool
    let version: String?
    let path: String?
}

/// `get_git_settings` response. Only a custom path override. The update is sent
/// as a raw `{settings:{custom_path}}` object (snake_case inside), so there is no
/// Encodable counterpart here.
struct GitSettings: Decodable, Sendable {
    let customPath: String?
}

/// A configured GitHub (or GH Enterprise) account (`get_github_accounts`). The
/// `id` is a UUID string and doubles as the keyring key for the account's token
/// (saved separately via `save_account_token`; never part of this metadata).
struct GitHubAccount: Decodable, Identifiable, Hashable, Sendable {
    let id: String
    let serverUrl: String
    let username: String
    let scopes: [String]
    let avatarUrl: String?
    let isDefault: Bool
    let createdAt: String

    /// Snake-case dict for the raw `update_github_accounts` body (the server
    /// stores the full accounts array verbatim).
    var snakeDict: [String: Any] {
        [
            "id": id,
            "server_url": serverUrl,
            "username": username,
            "scopes": scopes,
            "avatar_url": avatarUrl ?? NSNull(),
            "is_default": isDefault,
            "created_at": createdAt,
        ]
    }

    /// Host shown in the UI (e.g. "github.com").
    var host: String {
        URL(string: serverUrl)?.host ?? serverUrl
    }
}

extension GitHubAccount {
    /// A copy with `isDefault` changed (for clearing/setting the default in the
    /// full-replace `update_github_accounts` list).
    func with(isDefault: Bool) -> GitHubAccount {
        GitHubAccount(
            id: id, serverUrl: serverUrl, username: username, scopes: scopes,
            avatarUrl: avatarUrl, isDefault: isDefault, createdAt: createdAt
        )
    }
}

/// `get_github_accounts` wrapper.
struct GitHubAccountsSettings: Decodable, Sendable {
    let accounts: [GitHubAccount]
}

/// `validate_github_token` result. Note the success flag is `success` (not
/// `valid`), and `message` carries the error text on failure.
struct GitHubTokenValidation: Decodable, Sendable {
    let success: Bool
    let username: String?
    let scopes: [String]
    let avatarUrl: String?
    let message: String?
}
