import Foundation

/// A saved codeg server connection. The auth token is NOT stored here — it
/// lives in the Keychain keyed by `id`. Metadata persists in UserDefaults.
struct ServerProfile: Identifiable, Codable, Hashable, Sendable {
    let id: UUID
    var name: String
    var urlString: String
    var createdAt: Date

    init(id: UUID = UUID(), name: String, urlString: String, createdAt: Date = Date()) {
        self.id = id
        self.name = name
        self.urlString = urlString
        self.createdAt = createdAt
    }

    /// Normalized, validated base URL (`http://host:3080`), or nil if it is not a
    /// safe http(s) endpoint. Rejects non-http(s) schemes, a missing host, and any
    /// embedded userinfo (`user:pass@host`) so the bearer token can never be
    /// directed at an unexpected scheme/host. This is the security boundary used by
    /// `ServerStore.client` and `EventStream`.
    var baseURL: URL? {
        let trimmed = urlString.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, var components = URLComponents(string: trimmed) else { return nil }
        if components.scheme == nil {
            components = URLComponents(string: "http://\(trimmed)") ?? components
        }
        guard let scheme = components.scheme?.lowercased(), scheme == "http" || scheme == "https" else { return nil }
        guard let host = components.host, !host.isEmpty else { return nil }
        guard components.user == nil, components.password == nil else { return nil }
        if let port = components.port, !(1...65535).contains(port) { return nil }
        // Drop a trailing slash path so appendingPathComponent("api") is clean.
        if components.path == "/" { components.path = "" }
        return components.url
    }

    /// Host:port shown in the UI.
    var displayHost: String {
        guard let url = baseURL else { return urlString }
        var host = url.host ?? urlString
        if let port = url.port { host += ":\(port)" }
        return host
    }
}
