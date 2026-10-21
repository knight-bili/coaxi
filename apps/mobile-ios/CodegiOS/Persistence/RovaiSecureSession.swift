import Foundation
import Security
import CryptoKit

enum RovaiSecureSession {
    static func account(baseURL: URL, administratorToken: String) -> String {
        SHA256.hash(data: Data((baseURL.absoluteString + "\u{0}" + administratorToken).utf8)).map { String(format: "%02x", $0) }.joined()
    }
    private static func query(profile: UUID, account: String? = nil) -> [String: Any] {
        var value: [String: Any] = [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: "app.rovai.workbench.host-session.\(profile.uuidString)"]
        if let account { value[kSecAttrAccount as String] = account }; return value
    }
    static func load(profile: UUID, account: String) -> Data? {
        var value = query(profile: profile, account: account); value[kSecReturnData as String] = true; value[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: CFTypeRef?; guard SecItemCopyMatching(value as CFDictionary, &result) == errSecSuccess, let data = result as? Data, data.count <= (account.hasPrefix("pending:") ? 8 * 1024 * 1024 : 64 * 1024) else { return nil }; return data
    }
    @discardableResult static func save(_ data: Data?, profile: UUID, account: String) -> Bool {
        let value = query(profile: profile, account: account)
        guard let data else { let status = SecItemDelete(value as CFDictionary); return status == errSecSuccess || status == errSecItemNotFound }
        guard data.count <= (account.hasPrefix("pending:") ? 8 * 1024 * 1024 : 64 * 1024) else { return false }
        let attributes: [String: Any] = [kSecValueData as String: data, kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly]
        let status = SecItemUpdate(value as CFDictionary, attributes as CFDictionary)
        if status == errSecItemNotFound { var insert = value; insert.merge(attributes) { _, next in next }; return SecItemAdd(insert as CFDictionary, nil) == errSecSuccess }
        return status == errSecSuccess
    }
    static func remove(profile: UUID) { SecItemDelete(query(profile: profile) as CFDictionary) }
}
