import Foundation
import CryptoKit

enum CoaxiUploadSource {
    private static func root() throws -> URL {
        let base = try FileManager.default.url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
        let root = base.appendingPathComponent("CoaxiUploads", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700, .protectionKey: FileProtectionType.complete])
        return root
    }
    static func prepare(url: URL, threadId: String, target: JSONValue, revision: Int) async throws -> JSONValue {
        try await Task.detached(priority: .userInitiated) {
            let access = url.startAccessingSecurityScopedResource(); defer { if access { url.stopAccessingSecurityScopedResource() } }
            guard url.isFileURL, revision >= 0 else { throw CoaxiHostFailure(code: "请选择本机文件") }
            let values = try url.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey, .fileSizeKey])
            guard values.isRegularFile == true, values.isSymbolicLink != true, (values.fileSize ?? 0) <= 20 * 1024 * 1024 else { throw CoaxiHostFailure(code: "请选择20MB以内的普通文件") }
            let root = try root(); let files = try FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: [.fileSizeKey])
            let occupied = try files.reduce(0) { $0 + (try $1.resourceValues(forKeys: [.fileSizeKey]).fileSize ?? 0) }
            guard occupied < 512 * 1024 * 1024 else { throw CoaxiHostFailure(code: "暂存上传较多，请先核对未完成的上传") }
            let id = UUID().uuidString.lowercased(); let file = root.appendingPathComponent(id + ".bin")
            guard FileManager.default.createFile(atPath: file.path, contents: nil, attributes: [.posixPermissions: 0o600, .protectionKey: FileProtectionType.complete]) else { throw CoaxiHostFailure(code: "无法保存上传来源") }
            do {
                let source = try FileHandle(forReadingFrom: url); defer { try? source.close() }
                let output = try FileHandle(forWritingTo: file); defer { try? output.close() }
                var hash = SHA256(); var count = 0
                while let bytes = try source.read(upToCount: 65536), !bytes.isEmpty { try Task.checkCancellation(); count += bytes.count; guard count <= 20 * 1024 * 1024 else { throw CoaxiHostFailure(code: "文件超过20MB") }; hash.update(data: bytes); try output.write(contentsOf: bytes) }; try output.synchronize()
                let name = String(url.lastPathComponent.unicodeScalars.map { CharacterSet.controlCharacters.contains($0) || $0 == "/" || $0 == "\\" ? "_" : String($0) }.joined().prefix(240))
                return .object(["sourceFile": .string(file.path), "attempted": .bool(false), "intent": .object(["commandId": .string(id), "threadId": .string(threadId), "expectedRevision": .number(Double(revision)), "target": target, "displayName": .string(name.isEmpty ? "附件" : name), "byteSize": .number(Double(count)), "sha256": .string(hash.finalize().map { String(format: "%02x", $0) }.joined())])])
            } catch { try? FileManager.default.removeItem(at: file); throw error }
        }.value
    }
    static func frozenFile(_ request: JSONValue) throws -> URL {try file(request)}
    private static func file(_ request: JSONValue) throws -> URL {
        let id = request["intent"]?.text("commandId") ?? ""
        guard UUID(uuidString: id)?.uuidString.lowercased() == id else { throw CoaxiHostFailure(code: "上传来源记录无效") }
        let file = try root().appendingPathComponent(id + ".bin"); let values = try file.resourceValues(forKeys: [.isSymbolicLinkKey, .isRegularFileKey])
        guard file.path == request.text("sourceFile"), values.isSymbolicLink != true, values.isRegularFile == true else { throw CoaxiHostFailure(code: "上传来源记录无效") }; return file
    }
    static func bytes(_ request: JSONValue) async throws -> Data {
        try await Task.detached(priority: .userInitiated) { let file = try file(request); let size = try file.resourceValues(forKeys: [.fileSizeKey]).fileSize ?? 0; guard size <= 20 * 1024 * 1024 else { throw CoaxiHostFailure(code: "文件超过20MB") }; let handle = try FileHandle(forReadingFrom: file); defer { try? handle.close() }; let data = try handle.read(upToCount: 20 * 1024 * 1024 + 1) ?? Data(); guard data.count <= 20 * 1024 * 1024 else { throw CoaxiHostFailure(code: "文件超过20MB") }; return data }.value
    }
    static func remove(_ request: JSONValue) { if let file = try? file(request) { try? FileManager.default.removeItem(at: file) } }
}
