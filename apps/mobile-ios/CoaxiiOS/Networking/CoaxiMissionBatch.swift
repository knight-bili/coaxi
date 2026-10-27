import Foundation
import CryptoKit

enum CoaxiMissionBatch {
    static func prepare(_ url: URL) async throws -> JSONValue {try await CoaxiUploadSource.prepare(url: url, threadId: "", target: .object(["kind": .string("mission_batch_local_only")]), revision: 0)}
    static func intent(operation: CoaxiOperation, commandId: String, command: JSONValue, kept: [String], sources: [JSONValue]) throws -> JSONValue {
        guard [.missionsCreate, .missionsUpdate].contains(operation), kept.count + sources.count <= 10 else {throw CoaxiHostFailure(code: "使命最多保留10份附件")}
        let files = try sources.map {source -> JSONValue in guard let value = source["intent"] else {throw CoaxiHostFailure(code: "原附件记录不完整")};return .object(["id": value["commandId"] ?? .null, "displayName": value["displayName"] ?? .null, "byteSize": value["byteSize"] ?? .null, "sha256": value["sha256"] ?? .null])}
        return .object(["operation": .string(operation.rawValue), "commandId": .string(commandId), "command": command, "keepAttachmentIds": .array(kept.map(JSONValue.string)), "files": .array(files)])
    }
    /// Assemble multipart on disk; ten 20MB sources must not become one 200MB Data value.
    static func spool(intent: JSONValue, sources: [JSONValue]) async throws -> (URL, String) {
        try await Task.detached(priority: .userInitiated) {
            guard sources.count <= 10, sources.count == (intent["files"]?.rvArray.count ?? 0) else {throw CoaxiHostFailure(code: "原使命附件数量已变化")}
            let root = FileManager.default.temporaryDirectory.appendingPathComponent("CoaxiMissionMultipart", isDirectory: true)
            try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700, .protectionKey: FileProtectionType.complete])
            let path = root.appendingPathComponent(UUID().uuidString.lowercased() + ".multipart")
            guard FileManager.default.createFile(atPath: path.path, contents: nil, attributes: [.posixPermissions: 0o600, .protectionKey: FileProtectionType.complete]) else {throw CoaxiHostFailure(code: "无法准备使命上传")}
            do {
                let output = try FileHandle(forWritingTo: path);defer {try? output.close()}
                let boundary = "CoaxiMission-" + UUID().uuidString
                func write(_ text: String) throws {try output.write(contentsOf: Data(text.utf8))}
                try write("--\(boundary)\r\nContent-Disposition: form-data; name=\"intent\"\r\nContent-Type: application/json\r\n\r\n")
                try output.write(contentsOf: JSONEncoder().encode(intent));try write("\r\n")
                var seen = Set<String>()
                for source in sources {
                    try Task.checkCancellation()
                    let value = source["intent"] ?? .null, id = value.text("commandId")
                    guard UUID(uuidString: id)?.uuidString.lowercased() == id, seen.insert(id).inserted,
                          let expected = (intent["files"]?.rvArray ?? []).first(where: {$0.text("id") == id}), expected["sha256"] == value["sha256"], expected["byteSize"] == value["byteSize"] else {throw CoaxiHostFailure(code: "原使命附件身份已变化")}
                    let file = try CoaxiUploadSource.frozenFile(source)
                    try write("--\(boundary)\r\nContent-Disposition: form-data; name=\"file:\(id)\"; filename=\"source\"\r\nContent-Type: application/octet-stream\r\n\r\n")
                    let input = try FileHandle(forReadingFrom: file);defer {try? input.close()}
                    var size = 0, hash = SHA256()
                    while let bytes = try input.read(upToCount: 65536), !bytes.isEmpty {try Task.checkCancellation();size += bytes.count;guard size <= 20 * 1024 * 1024 else {throw CoaxiHostFailure(code: "文件超过20MB")};hash.update(data: bytes);try output.write(contentsOf: bytes)}
                    guard size == expected["byteSize"]?.rvInt, hash.finalize().map({String(format: "%02x", $0)}).joined() == expected.text("sha256") else {throw CoaxiHostFailure(code: "原使命附件内容已变化")}
                    try write("\r\n")
                }
                try write("--\(boundary)--\r\n");try output.synchronize();return (path, boundary)
            } catch {try? FileManager.default.removeItem(at: path);throw error}
        }.value
    }
}
