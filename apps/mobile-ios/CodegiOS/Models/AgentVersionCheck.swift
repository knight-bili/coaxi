import Foundation

/// A client-only install/upgrade/uninstall action (mirrors the web's UI-only
/// `UiFixAction` kinds — distinct from the server's ``FixActionKind``). The raw
/// value matches the web action string; the `binary`/`npx` split tells the model
/// layer which endpoint to call.
enum AgentInstallAction: String, Hashable, Sendable {
    case downloadBinary = "download_binary"
    case upgradeBinary = "upgrade_binary"
    case installNpx = "install_npx"
    case upgradeNpx = "upgrade_npx"
    case uninstallBinary = "uninstall_binary"
    case uninstallNpx = "uninstall_npx"
    case customInstall = "custom_install"

    var isUninstall: Bool { self == .uninstallBinary || self == .uninstallNpx }
    /// Upgrades clear the binary cache before (re)installing (web `runBinaryAction`
    /// `clearCache = mode === "upgrade"`); custom installs do too, handled by the
    /// caller via the explicit version override.
    var isUpgrade: Bool { self == .upgradeBinary || self == .upgradeNpx }
    var isInstall: Bool { self == .downloadBinary || self == .installNpx }
}

/// One synthesized version-row button.
struct AgentVersionAction: Hashable, Sendable, Identifiable {
    let label: String
    let action: AgentInstallAction
    var disabled: Bool = false
    var id: String { "\(action.rawValue)|\(label)" }
}

/// The synthesized "Version Status" row: a pass/warn/fail status, a
/// `Remote: … · Local: …` message, and the install/upgrade/uninstall/custom
/// buttons appropriate to the current state.
struct AgentVersionCheck: Hashable, Sendable {
    enum Status: String, Sendable { case pass, warn, fail }
    let status: Status
    let message: String
    let actions: [AgentVersionAction]
}

/// Pure version helpers + the synthetic version-status row, ported 1:1 from the
/// web `acp-agent-settings.tsx` (`compareVersion`, `hasComparableVersion`,
/// `isValidCustomVersion`, `buildVersionCheck`). Kept free of UI/networking so it
/// can be exercised directly in checks.
enum AgentVersion {

    private static func isDigit(_ c: Character) -> Bool { ("0"..."9").contains(c) }

    /// `< 0` if `a` is older than `b`. Strips leading non-digits, splits on ".",
    /// numeric-compares part by part (missing parts = 0). Only meaningful when
    /// both inputs pass ``hasComparable(_:)``.
    static func compare(_ a: String, _ b: String) -> Int {
        func toParts(_ value: String) -> [Int] {
            let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
            let normalized: Substring
            if let firstDigit = trimmed.firstIndex(where: isDigit) {
                normalized = trimmed[firstDigit...]
            } else {
                normalized = ""
            }
            return normalized
                .split(separator: ".", omittingEmptySubsequences: false)
                .map { part in Int(part.prefix(while: isDigit)) ?? 0 }
        }
        let left = toParts(a), right = toParts(b)
        let len = max(left.count, right.count)
        for i in 0..<len {
            let lv = i < left.count ? left[i] : 0
            let rv = i < right.count ? right[i] : 0
            if lv != rv { return lv > rv ? 1 : -1 }
        }
        return 0
    }

    /// Web `Boolean(value && /\d/.test(value) && value.includes("."))`.
    static func hasComparable(_ value: String?) -> Bool {
        guard let value, !value.isEmpty else { return false }
        return value.contains(where: isDigit) && value.contains(".")
    }

    /// Web `sanitize_custom_version` mirror: tolerate a leading `v`, must start
    /// with a digit, must be dotted, only `[0-9A-Za-z.\-+]`. Rejects `latest`,
    /// bare majors, anything with spaces/`@`.
    static func isValidCustom(_ value: String) -> Bool {
        var normalized = Substring(value.trimmingCharacters(in: .whitespacesAndNewlines))
        if let f = normalized.first, f == "v" || f == "V" { normalized = normalized.dropFirst() }
        guard let first = normalized.first, isDigit(first) else { return false }
        let ok = normalized.allSatisfy { c in
            isDigit(c) || ("A"..."Z").contains(c) || ("a"..."z").contains(c)
                || c == "." || c == "-" || c == "+"
        }
        return ok && normalized.contains(".")
    }

    /// The synthetic version-status row, or `nil` for `system`/unknown
    /// distributions (no managed install). `uvReady` is only consulted for `uvx`
    /// agents — when the uv runtime is missing, install/upgrade are surfaced
    /// disabled and the user is pointed at the separate "Install uv" preflight fix.
    static func check(_ agent: AcpAgentInfo, uvReady: Bool = true) -> AgentVersionCheck? {
        let dist = agent.distributionType ?? ""
        guard dist == "binary" || dist == "npx" || dist == "uvx" else { return nil }

        let remote = agent.registryVersion ?? "unknown"
        let local = agent.installedVersion ?? "Not installed"
        let versionText = "Remote: \(remote) · Local: \(local)"

        let installAction: AgentInstallAction = dist == "binary" ? .downloadBinary : .installNpx
        let upgradeAction: AgentInstallAction = dist == "binary" ? .upgradeBinary : .upgradeNpx
        let uninstallAction: AgentInstallAction = dist == "binary" ? .uninstallBinary : .uninstallNpx

        // uvx runtime missing → blocked, but uninstall stays available.
        if dist == "uvx" && !uvReady {
            var fixes = [AgentVersionAction(label: "Install", action: installAction, disabled: true)]
            if agent.installedVersion != nil {
                fixes.append(AgentVersionAction(label: "Uninstall", action: uninstallAction))
            }
            return AgentVersionCheck(
                status: .warn,
                message: "\(versionText). The uv runtime isn't installed — install it from the uv check below to use this agent.",
                actions: fixes
            )
        }

        // Only binary agents can be genuinely platform-unsupported.
        if !agent.available && dist != "uvx" {
            return AgentVersionCheck(
                status: .fail,
                message: "\(versionText). This platform doesn't support this agent.",
                actions: []
            )
        }

        let supportsCustom = dist == "npx" || (dist == "binary" && agent.registryVersion != nil)
        let customFix = AgentVersionAction(label: "Custom install", action: .customInstall)
        func withCustom(_ fixes: [AgentVersionAction]) -> [AgentVersionAction] {
            supportsCustom ? fixes + [customFix] : fixes
        }

        // Not installed.
        if agent.installedVersion == nil {
            return AgentVersionCheck(
                status: .fail,
                message: "\(versionText). Tap Install to set it up.",
                actions: withCustom([AgentVersionAction(label: "Install", action: installAction)])
            )
        }

        // Remote comparable, local not → suggest upgrade-to-overwrite.
        if let reg = agent.registryVersion, hasComparable(reg), !hasComparable(agent.installedVersion) {
            return AgentVersionCheck(
                status: .warn,
                message: "\(versionText). The local version isn't comparable; Upgrade to overwrite the install.",
                actions: withCustom([
                    AgentVersionAction(label: "Upgrade", action: upgradeAction),
                    AgentVersionAction(label: "Uninstall", action: uninstallAction),
                ])
            )
        }

        // Both comparable and local is behind → upgrade available.
        if hasComparable(agent.registryVersion), hasComparable(agent.installedVersion),
           let reg = agent.registryVersion, let inst = agent.installedVersion,
           compare(inst, reg) < 0 {
            return AgentVersionCheck(
                status: .warn,
                message: "\(versionText). Upgrade available.",
                actions: withCustom([
                    AgentVersionAction(label: "Upgrade", action: upgradeAction),
                    AgentVersionAction(label: "Uninstall", action: uninstallAction),
                ])
            )
        }

        // Remote unknown but something is installed.
        if agent.registryVersion == nil {
            return AgentVersionCheck(
                status: .warn,
                message: "\(versionText). The remote version is currently unavailable.",
                actions: withCustom([AgentVersionAction(label: "Uninstall", action: uninstallAction)])
            )
        }

        // Up to date.
        return AgentVersionCheck(
            status: .pass,
            message: "\(versionText). Already on the latest version.",
            actions: withCustom([AgentVersionAction(label: "Uninstall", action: uninstallAction)])
        )
    }
}
