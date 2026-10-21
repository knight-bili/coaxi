import Foundation

/// 只导航到当前工作区的既有对象，不接受凭据、地址或执行指令。
struct RovaiDeepLink: Identifiable, Equatable {
    let id = UUID()
    let kind: String
    let value: String
    init?(url: URL) {
        guard let parts = URLComponents(url: url, resolvingAgainstBaseURL: false),
              ["codeg", "rovai"].contains(parts.scheme?.lowercased() ?? ""),
              parts.query == nil, parts.fragment == nil, parts.user == nil, parts.password == nil, parts.port == nil,
              let kind = parts.host?.lowercased(), ["tab", "settings", "thread", "mission", "canvas", "conversation", "project", "folder"].contains(kind) else { return nil }
        let path = url.pathComponents.filter { $0 != "/" }
        guard path.count == 1, let value = path.first, !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              value.utf8.count <= 128, !value.contains("/"), !value.contains("\\"), !value.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) }) else { return nil }
        self.kind = kind; self.value = value
    }
}
