import Foundation

/// Cursor (`cursor-agent` CLI) settings logic, ported from the web
/// `src/components/settings/cursor-config-panel.tsx`. Kept out of the view so the
/// env rules — which decide whether a credential is written or DELETED — are
/// readable on their own.
///
/// Unlike Kimi/Pi, Cursor has no dedicated save command: its credential/model
/// knobs are plain env vars and its permission rules are a structured patch on
/// `~/.cursor/cli-config.json`, both of which the shared draft + host Save
/// already persist in one `acp_update_agent_env` → `acp_update_agent_config` pass.
enum CursorConfig {
    // MARK: Env keys

    /// The Cursor Dashboard account key (headless / server machines).
    static let apiKeyEnv = "CURSOR_API_KEY"
    /// Written by older builds; `cursor-agent` has NO bring-your-own-endpoint
    /// support, so codeg always scrubs this rather than surfacing a field for it.
    static let apiBaseUrlEnv = "CURSOR_API_BASE_URL"
    /// The `--model` id passed at launch.
    static let modelEnv = "CURSOR_MODEL"
    /// codeg-side launch knob: `"1"` inserts the CLI's root `--force` flag (Run
    /// Everything) before the `acp` subcommand. The CLI reads no such env var.
    static let forceEnv = "CURSOR_FORCE"
    /// codeg-side knob recording the chosen authentication method. The launch
    /// path clears an inherited API key in `subscription` mode so the browser
    /// login is used. The CLI ignores this var.
    static let authModeEnv = "CURSOR_AUTH_MODE"

    // MARK: Authentication method

    /// Cursor's two real authentication methods. `custom` is a Cursor *account*
    /// API key — NOT a third-party endpoint (the CLI has none). The wire token
    /// stays `"custom"` for rows saved before the web's rename.
    enum AuthMethod: String, CaseIterable, Sendable {
        case subscription
        case custom
    }

    /// The persisted method, tolerant of legacy rows: an explicit
    /// `CURSOR_AUTH_MODE` wins, otherwise a saved API key implies `custom`.
    static func inferMode(_ env: [String: String]) -> AuthMethod {
        let explicit = (env[authModeEnv] ?? "").trimmingCharacters(in: .whitespaces)
        if let mode = AuthMethod(rawValue: explicit) { return mode }
        return (env[apiKeyEnv] ?? "").trimmingCharacters(in: .whitespaces).isEmpty ? .subscription : .custom
    }

    /// The saved Run Everything knob, tolerant of hand-edited values.
    static func isForceEnabled(_ env: [String: String]) -> Bool {
        let value = (env[forceEnv] ?? "").trimmingCharacters(in: .whitespaces).lowercased()
        return value == "1" || value == "true"
    }

    /// Whether the knob was ever written. A fresh agent (key absent) defaults to
    /// Run Everything ON; an explicit `"0"` — the user chose "ask before running"
    /// — is respected.
    static func hasForceKnob(_ env: [String: String]) -> Bool { env[forceEnv] != nil }

    /// The copy-pasteable login command. codeg's managed `cursor-agent` lives in
    /// its binary cache and is NOT on the user's PATH, so a bare
    /// `cursor-agent login` fails — use the resolved absolute path, quoted when
    /// it contains whitespace.
    static func loginCommand(binaryPath: String?) -> String {
        let path = (binaryPath ?? "").trimmingCharacters(in: .whitespaces)
        guard !path.isEmpty else { return "cursor-agent login" }
        let program = path.contains(where: { $0 == " " || $0 == "\t" }) ? "\"\(path)\"" : path
        return "\(program) login"
    }

    /// Bake the panel's credential/model/launch knobs into `envText`, mirroring
    /// the web `buildCursorEnv`. Unrelated keys are preserved (``EnvText/patch``
    /// merges), and an empty value deletes its key:
    ///
    /// - `subscription` — the API key is DELETED so a launch (and the probes)
    ///   fall back to the Cursor account.
    /// - `custom` — the key from the form is written.
    ///
    /// `CURSOR_API_BASE_URL` is always removed (dead weight from a legacy row),
    /// and the method itself is always recorded.
    static func applyEnv(_ envText: String,
                         mode: AuthMethod,
                         apiKey: String,
                         model: String,
                         force: Bool) -> String {
        EnvText.patch(envText, [
            authModeEnv: mode.rawValue,
            apiBaseUrlEnv: "",
            apiKeyEnv: mode == .custom ? apiKey : "",
            modelEnv: model,
            forceEnv: force ? "1" : "",
        ])
    }
}

// MARK: - Probe responses

/// `acp_cursor_auth_status` — the result of probing `cursor-agent status`.
/// Parsed defensively server-side, so an unknown shape still yields something
/// showable via `rawStatus` / `error`.
struct CursorAuthStatus: Decodable, Sendable, Equatable {
    /// A launchable cursor-agent binary was found (cache or system install).
    let installed: Bool
    let isAuthenticated: Bool
    /// The CLI's own status string (e.g. `"unauthenticated"`).
    let rawStatus: String?
    /// Account email when signed in.
    let email: String?
    /// Membership/plan label when the CLI reports one (usually absent).
    let membership: String?
    /// Probe failure detail (spawn error / timeout / non-JSON output).
    let error: String?
    /// Absolute path of the binary codeg would launch — the source for
    /// ``CursorConfig/loginCommand(binaryPath:)``. Nil when nothing is installed.
    let binaryPath: String?

    private enum CodingKeys: String, CodingKey {
        case installed, isAuthenticated, rawStatus, email, membership, error, binaryPath
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        installed = try c.decodeIfPresent(Bool.self, forKey: .installed) ?? false
        isAuthenticated = try c.decodeIfPresent(Bool.self, forKey: .isAuthenticated) ?? false
        rawStatus = try c.decodeIfPresent(String.self, forKey: .rawStatus)
        email = try c.decodeIfPresent(String.self, forKey: .email)
        membership = try c.decodeIfPresent(String.self, forKey: .membership)
        error = try c.decodeIfPresent(String.self, forKey: .error)
        binaryPath = try c.decodeIfPresent(String.self, forKey: .binaryPath)
    }
}

/// One entry from `cursor-agent models`. `label` is the human name (empty when
/// the CLI emitted a bare id); `id` is what goes to `--model`.
struct CursorModelInfo: Decodable, Sendable, Hashable, Identifiable {
    let id: String
    let label: String
    let isDefault: Bool

    private enum CodingKeys: String, CodingKey { case id, label, isDefault }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decodeIfPresent(String.self, forKey: .id) ?? ""
        label = try c.decodeIfPresent(String.self, forKey: .label) ?? ""
        isDefault = try c.decodeIfPresent(Bool.self, forKey: .isDefault) ?? false
    }

    /// What the picker shows — the CLI's label, falling back to the raw id.
    var displayLabel: String { label.isEmpty ? id : label }
}

/// `acp_cursor_list_models` — best-effort parsed CLI output. `error` carries the
/// reason the probe couldn't run (e.g. not signed in) while `models` stays empty.
struct CursorModelsResult: Decodable, Sendable {
    let models: [CursorModelInfo]
    let defaultModel: String?
    let error: String?

    private enum CodingKeys: String, CodingKey { case models, defaultModel, error }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        models = try c.decodeIfPresent([CursorModelInfo].self, forKey: .models) ?? []
        defaultModel = try c.decodeIfPresent(String.self, forKey: .defaultModel)
        error = try c.decodeIfPresent(String.self, forKey: .error)
    }
}
