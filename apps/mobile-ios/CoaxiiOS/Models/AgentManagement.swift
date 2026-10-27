import Foundation

/// Per-agent preflight diagnostics (`acp_preflight`). `status` is kept as a raw
/// string (lenient) so an unknown future check status can't fail the decode; the
/// UI colors known values (ok/pass = accent, warn = amber, error/fail = danger).
struct PreflightResult: Decodable, Sendable {
    let agentType: AgentType
    let agentName: String
    let passed: Bool
    let checks: [PreflightCheck]
}

struct PreflightCheck: Decodable, Hashable, Sendable, Identifiable {
    let checkId: String
    let label: String
    let status: String
    let message: String
    /// Server-suggested remediations (`fixes[]`). Optional/lenient so an older
    /// server that omits the field — or an unknown future fix kind — can't fail
    /// the decode. Rendered as tappable buttons under the check.
    let fixes: [FixAction]?

    var id: String { checkId }

    var isOK: Bool { ["ok", "pass", "passed", "success"].contains(status.lowercased()) }
    var isWarning: Bool { ["warn", "warning"].contains(status.lowercased()) }

    enum CodingKeys: String, CodingKey { case checkId, label, status, message, fixes }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        checkId = (try? c.decode(String.self, forKey: .checkId)) ?? ""
        label = (try? c.decode(String.self, forKey: .label)) ?? ""
        status = (try? c.decode(String.self, forKey: .status)) ?? ""
        message = (try? c.decode(String.self, forKey: .message)) ?? ""
        fixes = try? c.decodeIfPresent([FixAction].self, forKey: .fixes)
    }
}

/// The kinds of server-emitted preflight remediation. Closed set on the wire
/// (`FixActionKind`), but decoded leniently to `.unknown` so a new server kind
/// can't fail the list. Client-only install/upgrade/uninstall actions are a
/// separate concept — see ``AgentInstallAction``.
enum FixActionKind: String, Sendable, Hashable {
    case openURL = "open_url"
    case redownloadBinary = "redownload_binary"
    case retryConnection = "retry_connection"
    case openAgentsSettings = "open_agents_settings"
    case installOpenCodePlugins = "install_opencode_plugins"
    case installUv = "install_uv"
    case unknown

    init(from decoder: Decoder) throws {
        let raw = (try? decoder.singleValueContainer().decode(String.self)) ?? ""
        self = FixActionKind(rawValue: raw) ?? .unknown
    }
}

extension FixActionKind: Decodable {}

/// One remediation button on a preflight check (`{label, kind, payload}`). Decode
/// is defensive (missing parts → empty / `.unknown`); a memberwise init is also
/// provided so the UI can synthesize fixes if ever needed.
struct FixAction: Decodable, Hashable, Sendable, Identifiable {
    let label: String
    let kind: FixActionKind
    let payload: String

    var id: String { "\(kind.rawValue)|\(payload)|\(label)" }

    enum CodingKeys: String, CodingKey { case label, kind, payload }

    init(label: String, kind: FixActionKind, payload: String) {
        self.label = label; self.kind = kind; self.payload = payload
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        label = (try? c.decode(String.self, forKey: .label)) ?? ""
        kind = (try? c.decode(FixActionKind.self, forKey: .kind)) ?? .unknown
        payload = (try? c.decode(String.self, forKey: .payload)) ?? ""
    }
}
