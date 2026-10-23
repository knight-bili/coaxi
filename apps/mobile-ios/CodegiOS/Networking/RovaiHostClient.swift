import Foundation
import CryptoKit

struct RovaiHostSession: Codable, Sendable {
    let token: String
    let clientId: String
    let editorProof: String
    let ownerId: String
    let epoch: String
    var expiresAt: Double
}
struct RovaiHostFailure: LocalizedError, Sendable {
    let code: String
    var unknownResult = false
    var errorDescription: String? { code }
}
/// Actor isolation protects a single Host/editor lifetime, never a second execution engine.
actor RovaiHostClient {
    let baseURL: URL
    private let administratorToken: String
    private let session: URLSession
    private let load: @Sendable () async -> Data?
    private let save: @Sendable (Data?) async -> Void
    private var credential: RovaiHostSession?
    private var loaded = false
    private var denied = false
    private var deadline: TimeInterval = 0
    private var generation: UInt64 = 0
    private var login: Task<RovaiHostSession, Error>?
    init(baseURL: URL, administratorToken: String, load: @escaping @Sendable () async -> Data? = { nil }, save: @escaping @Sendable (Data?) async -> Void = { _ in }) throws {
        guard let parts = URLComponents(url: baseURL, resolvingAgainstBaseURL: false), ["http", "https"].contains(parts.scheme ?? ""), parts.host != nil, parts.user == nil, parts.password == nil, parts.query == nil, parts.fragment == nil else { throw RovaiHostFailure(code: "host.invalid_url") }
        self.baseURL = baseURL; self.administratorToken = administratorToken; self.load = load; self.save = save
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 120; configuration.timeoutIntervalForResource = 180
        configuration.waitsForConnectivity = true
        self.session = URLSession(configuration: configuration, delegate: RovaiNoRedirect(), delegateQueue: nil)
    }
    private func endpoint(_ path: String) -> URL { baseURL.appendingPathComponent("api/v1/" + path) }
    private func post(_ path: String, body: JSONValue, bearer: String? = nil) async throws -> JSONValue {
        var request = URLRequest(url: endpoint(path)); request.httpMethod = "POST"; request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        if let bearer { request.setValue("Bearer " + bearer, forHTTPHeaderField: "Authorization") }
        request.httpBody = try JSONEncoder().encode(body)
        let (data, response) = try await session.data(for: request)
        guard let response = response as? HTTPURLResponse else { throw RovaiHostFailure(code: "host.invalid_response") }
        guard (200...299).contains(response.statusCode) else { throw RovaiHostFailure(code: response.statusCode == 401 ? "host.unauthorized" : "host.http_\(response.statusCode)") }
        guard data.count <= 32 * 1024 * 1024 else { throw RovaiHostFailure(code: "host.response_limit") }
        return try JSONDecoder().decode(JSONValue.self, from: data)
    }
    func connect(authenticate: Bool = false) async throws -> RovaiHostSession {
        if !authenticate, !denied, let credential, deadline > ProcessInfo.processInfo.systemUptime + 60 { return credential }
        if let login { return try await login.value }
        let lifetime = generation
        let task = Task<RovaiHostSession, Error> {
            if !self.loaded { self.loaded = true; if let data = await self.load() { self.credential = try? JSONDecoder().decode(RovaiHostSession.self, from: data) } }
            if !authenticate {
                guard !self.denied, let previous = self.credential else { throw RovaiHostFailure(code: "host.login_required") }
                let resumed = try await self.post("session", body: .object(["editor": .object(["clientId": .string(previous.clientId), "proof": .string(previous.editorProof)])]), bearer: previous.token)
                self.deadline = try self.monotonicDeadline(resumed)
                if self.deadline <= ProcessInfo.processInfo.systemUptime + 60 { try await self.renew() }
                return previous
            }
            var fields: [String: JSONValue] = ["protocolVersion": .number(4), "administratorToken": .string(self.administratorToken)]
            if let previous = self.credential { fields["editor"] = .object(["clientId": .string(previous.clientId), "proof": .string(previous.editorProof)]) }
            let value = try await self.post("login", body: .object(fields))
            guard value["protocolVersion"] == .number(4) else { throw RovaiHostFailure(code: "host.protocol_incompatible") }
            let next = try JSONDecoder().decode(RovaiHostSession.self, from: JSONEncoder().encode(value))
            guard lifetime == self.generation else { _ = try? await self.post("logout", body: .object([:]), bearer: next.token); throw CancellationError() }
            self.credential = next; self.deadline = try self.monotonicDeadline(value); self.denied = false; await self.save(try JSONEncoder().encode(next)); return next
        }
        login = task
        do { let result = try await task.value; login = nil; return result } catch { login = nil; throw error }
    }
    func request(_ operation: RovaiOperation, params: JSONValue = .object([:])) async throws -> JSONValue {
        let current = try await connect()
        let envelope: JSONValue
        do { envelope = try await post("request", body: .object(["operation": .string(operation.rawValue), "params": params]), bearer: current.token) }
        catch is CancellationError { throw CancellationError() }
        catch { if (error as? RovaiHostFailure)?.code == "host.unauthorized" { denied = true; deadline = 0 }; throw RovaiHostFailure(code: error.localizedDescription, unknownResult: true) }
        if let error = envelope["error"], error != .null { throw RovaiHostFailure(code: error.prettyString) }
        return envelope["result"] ?? .null
    }
    func files(_ action: String, request: JSONValue) async throws -> JSONValue {
        guard ["open", "restore", "reopen", "readText", "readPage", "readHtml", "resolveLine", "reload", "release"].contains(action) else { throw RovaiHostFailure(code: "预览操作无效") }
        let current = try await connect(); let lifetime = generation
        let value = try await post("files", body: .object(["action": .string(action), "request": request]), bearer: current.token)
        guard lifetime == generation else { throw CancellationError() }
        guard value["ok"] == .bool(true) else { throw RovaiHostFailure(code: value["error"]?.text("message") ?? "预览失败") }
        return value["value"] ?? .null
    }

    /// 私有内容不进入普通命令恢复正文；失败只显示固定原因，未知结果核对原编号。
    func privateConfiguration(_ action: String, input: JSONValue) async throws -> JSONValue {
        guard ["provider", "secrets", "receipt", "export", "export-receipt", "sync", "logging", "templates", "files-finish"].contains(action), try JSONEncoder().encode(input).count <= 2 * 1024 * 1024 else { throw RovaiHostFailure(code: "配置私有操作无效") }
        let current = try await connect(); let lifetime = generation
        let envelope: JSONValue
        do { envelope = try await post("configuration/" + action, body: input, bearer: current.token) }
        catch is CancellationError { throw CancellationError() }
        catch { if (error as? RovaiHostFailure)?.code == "host.unauthorized" { denied = true; deadline = 0 }; throw RovaiHostFailure(code: "configuration.private_result_unknown", unknownResult: true) }
        guard lifetime == generation else { throw CancellationError() }
        if let error = envelope["error"], error != .null { throw RovaiHostFailure(code: "configuration.private_input_rejected") }
        guard let result = envelope["result"] else { throw RovaiHostFailure(code: "configuration.private_response_invalid") }; return result
    }

    func configurationArchive(metadata: JSONValue, data: Data) async throws -> JSONValue {
        guard data.count <= 64 * 1024 * 1024, metadata["byteSize"]?.rvInt == data.count else { throw RovaiHostFailure(code: "配置文件超过64MB或大小已变化") }
        let digest = "sha256:" + SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
        guard metadata.text("sourceDigest") == digest else { throw RovaiHostFailure(code: "配置文件已变化") }
        let raw = try JSONEncoder().encode(metadata)
        guard raw.count <= 128 * 1024 else { throw RovaiHostFailure(code: "配置说明过大") }
        let current = try await connect(); let lifetime = generation; let boundary = "RovaiConfig-" + UUID().uuidString
        var bytes = Data("--\(boundary)\r\nContent-Disposition: form-data; name=\"metadata\"\r\nContent-Type: application/json\r\n\r\n".utf8)
        bytes.append(raw); bytes.append(Data("\r\n--\(boundary)\r\nContent-Disposition: form-data; name=\"file\"; filename=\"configuration\"\r\nContent-Type: application/octet-stream\r\n\r\n".utf8)); bytes.append(data); bytes.append(Data("\r\n--\(boundary)--\r\n".utf8))
        var request = URLRequest(url: endpoint("configuration/archive")); request.httpMethod = "POST"; request.setValue("Bearer " + current.token, forHTTPHeaderField: "Authorization"); request.setValue("multipart/form-data; boundary=" + boundary, forHTTPHeaderField: "Content-Type"); request.setValue("no-store", forHTTPHeaderField: "Cache-Control"); request.httpBody = bytes
        let reply: Data; let response: URLResponse
        do { (reply, response) = try await session.data(for: request) }
        catch is CancellationError { throw CancellationError() }
        catch { throw RovaiHostFailure(code: "configuration.archive_result_unknown", unknownResult: true) }
        guard let http = response as? HTTPURLResponse, (200...299).contains(http.statusCode) else { if (response as? HTTPURLResponse)?.statusCode == 401 { denied = true; deadline = 0 }; throw RovaiHostFailure(code: "configuration.archive_result_unknown", unknownResult: true) }
        guard lifetime == generation else { throw CancellationError() }
        guard reply.count <= 2 * 1024 * 1024 else { throw RovaiHostFailure(code: "configuration.archive_response_limit", unknownResult: true) }
        let envelope = try JSONDecoder().decode(JSONValue.self, from: reply)
        if let error = envelope["error"], error != .null { throw RovaiHostFailure(code: "configuration.archive_rejected") }
        return envelope["result"] ?? .null
    }

    func downloadConfigurationExport(_ receipt: JSONValue) async throws -> Data {
        guard receipt.text("state") == "ready", let size = receipt["byteSize"]?.rvInt, (0...64 * 1024 * 1024).contains(size) else { throw RovaiHostFailure(code: "配置副本尚未准备好") }
        let current = try await connect(); let lifetime = generation
        var request = URLRequest(url: endpoint("configuration/export-download")); request.httpMethod = "POST"; request.setValue("Bearer " + current.token, forHTTPHeaderField: "Authorization"); request.setValue("application/json", forHTTPHeaderField: "Content-Type"); request.setValue("no-store", forHTTPHeaderField: "Cache-Control"); request.httpBody = try JSONEncoder().encode(JSONValue.object(["operationId": receipt["operationId"] ?? .null]))
        let (url, response) = try await session.download(for: request); defer { try? FileManager.default.removeItem(at: url) }
        guard let http = response as? HTTPURLResponse, (200...299).contains(http.statusCode) else { if (response as? HTTPURLResponse)?.statusCode == 401 { denied = true; deadline = 0 }; throw RovaiHostFailure(code: "configuration.download_unavailable") }
        guard lifetime == generation else { throw CancellationError() }
        let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
        guard (attributes[.size] as? NSNumber)?.intValue == size else { throw RovaiHostFailure(code: "配置副本大小已变化") }
        let data = try Data(contentsOf: url, options: .mappedIfSafe)
        let digest = "sha256:" + SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
        guard data.count == size, digest == receipt.text("encryptedDigest") else { throw RovaiHostFailure(code: "配置副本摘要不同") }
        guard lifetime == generation else { throw CancellationError() }; return data
    }
    func officePreviewURL(port: Int, cap: String) throws -> URL { guard (1...65535).contains(port), (32...256).contains(cap.utf8.count), cap.utf8.allSatisfy({ ($0 >= 48 && $0 <= 57) || ($0 >= 65 && $0 <= 90) || ($0 >= 97 && $0 <= 122) || $0 == 95 || $0 == 45 }), var parts = URLComponents(url: baseURL.appendingPathComponent("api/office-watch-proxy/\(port)/"), resolvingAgainstBaseURL: false) else { throw RovaiHostFailure(code: "预览授权无效") }; parts.queryItems = [URLQueryItem(name: "cap", value: cap)]; guard let url = parts.url else { throw RovaiHostFailure(code: "预览地址无效") }; return url }
    func fileBytes(_ action: String, request value: JSONValue) async throws -> RovaiFileBytes {
        guard ["readBinary", "readChildImage", "download"].contains(action) else { throw RovaiHostFailure(code: "文件操作无效") }
        let current = try await connect(); let lifetime = generation
        var request = URLRequest(url: endpoint("files/bytes")); request.httpMethod = "POST"; request.setValue("application/json", forHTTPHeaderField: "Content-Type"); request.setValue("Bearer " + current.token, forHTTPHeaderField: "Authorization"); request.httpBody = try JSONEncoder().encode(JSONValue.object(["action": .string(action), "request": value]))
        let (stream, reply) = try await session.bytes(for: request)
        guard let reply = reply as? HTTPURLResponse, (200...299).contains(reply.statusCode) else { if (reply as? HTTPURLResponse)?.statusCode == 401 { denied = true; deadline = 0 }; throw RovaiHostFailure(code: "文件读取被拒绝") }
        let mime = reply.mimeType ?? "application/octet-stream"
        guard mime != "application/json", let contentGeneration = reply.value(forHTTPHeaderField: "x-rovai-content-generation"), contentGeneration == value.text("expectedGeneration") else { throw RovaiHostFailure(code: "文件已变化或来源失效，请重新打开") }
        var data = Data(); for try await byte in stream { if data.count % 65536 == 0 { try Task.checkCancellation(); guard lifetime == generation else { throw CancellationError() } }; guard data.count < 20 * 1024 * 1024 else { throw RovaiHostFailure(code: "文件超过手机预览的20MB限制") }; data.append(byte) }
        guard lifetime == generation else { throw CancellationError() }; return RovaiFileBytes(data: data, mime: mime, contentGeneration: contentGeneration)
    }
    func upload(intent: JSONValue, data: Data) async throws -> JSONValue {
        guard data.count <= 20 * 1024 * 1024, intent["byteSize"]?.rvInt == data.count else { throw RovaiHostFailure(code: "文件超过20MB或大小已变化") }
        let digest = SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
        guard intent.text("sha256") == digest else { throw RovaiHostFailure(code: "上传文件已变化") }
        let current = try await connect(); let boundary = "Rovai-" + UUID().uuidString
        var bytes = Data(); bytes.append(Data("--\(boundary)\r\nContent-Disposition: form-data; name=\"intent\"\r\nContent-Type: application/json\r\n\r\n".utf8)); bytes.append(try JSONEncoder().encode(intent))
        bytes.append(Data("\r\n--\(boundary)\r\nContent-Disposition: form-data; name=\"file\"; filename=\"source\"\r\nContent-Type: application/octet-stream\r\n\r\n".utf8)); bytes.append(data); bytes.append(Data("\r\n--\(boundary)--\r\n".utf8))
        var request = URLRequest(url: endpoint("uploads")); request.httpMethod = "POST"; request.setValue("Bearer " + current.token, forHTTPHeaderField: "Authorization"); request.setValue("multipart/form-data; boundary=" + boundary, forHTTPHeaderField: "Content-Type"); request.httpBody = bytes
        let (reply, response) = try await session.data(for: request)
        guard let response = response as? HTTPURLResponse, (200...299).contains(response.statusCode) else { if (response as? HTTPURLResponse)?.statusCode == 401 { denied = true; deadline = 0 }; throw RovaiHostFailure(code: "host.upload_result_unknown", unknownResult: true) }
        guard reply.count <= 32 * 1024 * 1024 else { throw RovaiHostFailure(code: "host.upload_response_limit", unknownResult: true) }; return try JSONDecoder().decode(JSONValue.self, from: reply)
    }
    func uploadMissionBatch(intent: JSONValue, sources: [JSONValue]) async throws -> JSONValue {
        let current = try await connect();let lifetime = generation
        let (file, boundary) = try await RovaiMissionBatch.spool(intent: intent, sources: sources)
        defer {try? FileManager.default.removeItem(at: file)}
        guard lifetime == generation else {throw CancellationError()}
        var request = URLRequest(url: endpoint("mission-files"));request.httpMethod = "POST";request.setValue("Bearer " + current.token, forHTTPHeaderField: "Authorization");request.setValue("multipart/form-data; boundary=" + boundary, forHTTPHeaderField: "Content-Type")
        let (bytes, response) = try await session.upload(for: request, fromFile: file)
        guard lifetime == generation, let response = response as? HTTPURLResponse, (200...299).contains(response.statusCode) else {if (response as? HTTPURLResponse)?.statusCode == 401 {denied = true;deadline = 0};throw RovaiHostFailure(code: "host.mission_upload_result_unknown", unknownResult: true)}
        guard bytes.count <= 1024 * 1024 else {throw RovaiHostFailure(code: "host.mission_response_limit", unknownResult: true)}
        return try JSONDecoder().decode(JSONValue.self, from: bytes)
    }
    func uploadStatus(intent: JSONValue, action: String = "reconcile") async throws -> JSONValue {
        guard ["reconcile", "receipt", "retry", "finish"].contains(action) else { throw RovaiHostFailure(code: "上传操作无效") }; let current = try await connect(); return try await post("uploads/" + action, body: intent, bearer: current.token)
    }
    func command(_ operation: RovaiOperation, command: JSONValue, id: UUID) async throws -> JSONValue {
        try await request(operation, params: .object(["commandId": .string(id.uuidString.lowercased()), "command": command]))
    }
    func reconcile(_ operation: RovaiOperation, params: JSONValue) async throws -> JSONValue {
        try await request(.commandsReconcile, params: .object(["operation": .string(operation.rawValue), "params": params]))
    }
    func invalidations() async throws -> AsyncThrowingStream<JSONValue, Error> {
        let current = try await connect(); var request = URLRequest(url: endpoint("events")); request.setValue("Bearer " + current.token, forHTTPHeaderField: "Authorization"); request.timeoutInterval = 3600
        let streamSession = self.session
        return AsyncThrowingStream { continuation in
            let reader = Task {
                do {
                    let (bytes, response) = try await streamSession.bytes(for: request)
                    guard let response = response as? HTTPURLResponse else { throw RovaiHostFailure(code: "host.invalid_response") }
                    if response.statusCode == 401 || response.statusCode == 403 { throw RovaiHostFailure(code: "host.unauthorized") }
                    guard response.statusCode == 200 else { throw RovaiHostFailure(code: "host.http_" + String(response.statusCode)) }
                    continuation.yield(.object(["event": .string("resync"), "data": .null]))
                    var event = "", data = ""
                    for try await line in bytes.lines {
                        try Task.checkCancellation()
                        if line.isEmpty { if ["resync", "invalidate"].contains(event), let value = JSONValue.parse(data) { continuation.yield(.object(["event": .string(event), "data": value])) }; event = ""; data = "" }
                        else if line.hasPrefix("event:") { event = String(line.dropFirst(6)).trimmingCharacters(in: .whitespaces) }
                        else if line.hasPrefix("data:") { if !data.isEmpty { data += "\n" }; data += String(line.dropFirst(5)).trimmingCharacters(in: .whitespaces); guard data.utf8.count <= 1024 * 1024 else { throw RovaiHostFailure(code: "host.event_limit") } }
                    }
                    continuation.finish()
                } catch { continuation.finish(throwing: error) }
            }
            continuation.onTermination = { _ in reader.cancel() }
        }
    }
    private func monotonicDeadline(_ value: JSONValue) throws -> TimeInterval { guard case .number(let ttl) = value["expiresInSeconds"] else { throw RovaiHostFailure(code: "host.invalid_timing") }; return ProcessInfo.processInfo.systemUptime + min(max(ttl, 0), 86_400) }
    func renew() async throws { guard var current = credential else { return }; let value = try await post("session/renew", body: .object([:]), bearer: current.token); guard case .number(let expiry) = value["expiresAt"] else { throw RovaiHostFailure(code: "host.invalid_renewal") }; current.expiresAt = expiry; deadline = try monotonicDeadline(value); credential = current; await save(try JSONEncoder().encode(current)) }
    func logout() async throws { generation &+= 1; login?.cancel(); login = nil; let previous = credential; credential = nil; denied = true; deadline = 0; await save(nil); if let previous { _ = try await post("logout", body: .object([:]), bearer: previous.token) } }
    func close() { generation &+= 1; login?.cancel(); login = nil; credential = nil; session.invalidateAndCancel() }
}
private final class RovaiNoRedirect: NSObject, URLSessionTaskDelegate, @unchecked Sendable {
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) { completionHandler(nil) }
}

struct RovaiFileBytes: Sendable { let data: Data; let mime: String; let contentGeneration: String }
