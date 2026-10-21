import Foundation
import CryptoKit
import Observation

extension JSONValue {
    var rvArray: [JSONValue] { if case .array(let value) = self { return value }; return [] }
    var rvObject: [String: JSONValue] { if case .object(let value) = self { return value }; return [:] }
    var rvInt: Int { if case .number(let value) = self { return Int(value) }; return 0 }
    var rvBool: Bool { self == .bool(true) }
    func text(_ key: String) -> String { self[key]?.stringValue ?? "" }
}
struct RovaiRow: Identifiable { let value: JSONValue; var id: String { value.text("id").isEmpty ? value.text("missionId").isEmpty ? value.text("agentId") : value.text("missionId") : value.text("id") } }

@MainActor @Observable
final class RovaiWorkspaceModel {
    var connected = false
    var busy = false
    var issue: String?
    var navigation: JSONValue = .object([:])
    var members: [JSONValue] = []
    var missions: [JSONValue] = []
    var boards: [JSONValue] = []
    var thread: JSONValue?
    var threadId: String?
    var pending: JSONValue?
    var lastResult: JSONValue?
    var lastInputOwner: String?
    var lastInputBody: String?
    var lastOperation: RovaiOperation?
    var lastUploadResult: JSONValue?
    var publicSources: [JSONValue] = []
    var publicSourcesReady = false
    var connectionGeneration: UInt64 { generation }
    @ObservationIgnored private var client: RovaiHostClient?
    @ObservationIgnored private var profile: ServerProfile?
    @ObservationIgnored private var account = ""
    @ObservationIgnored private var generation: UInt64 = 0
    @ObservationIgnored private var terminals: [String: RovaiTerminalController] = [:]
    @ObservationIgnored private var events: Task<Void, Never>?
    @ObservationIgnored private var active = true
    @ObservationIgnored private var refreshing = false
    @ObservationIgnored private var writing = false
    func bind(_ profile: ServerProfile?, store: ServerStore) async {
        terminals.values.forEach { $0.retire() }; terminals.removeAll()
        generation &+= 1; events?.cancel(); client = nil; self.profile = profile; connected = false; thread = nil; threadId = nil; navigation = .object([:]); members = []; missions = []; boards = []; issue = nil; pending = nil; lastResult = nil; lastInputOwner = nil; lastInputBody = nil; lastOperation = nil; lastUploadResult = nil; publicSources = []; publicSourcesReady = false
        guard let profile, let token = store.token(for: profile), let baseURL = profile.baseURL else { return }
        account = "pending:" + RovaiSecureSession.account(baseURL: baseURL, administratorToken: token)
        if let data = RovaiSecureSession.load(profile: profile.id, account: account) { pending = try? JSONDecoder().decode(JSONValue.self, from: data) }
        let lifetime = generation
        do { client = try store.rovaiClient(for: profile); _ = try await client?.connect(); guard lifetime == generation else { return }; connected = true; await refresh(); listen() }
        catch { guard lifetime == generation else { return }; failure(error) }
    }
    func authenticate() async {
        guard let client, !busy else { return }; busy = true; issue = nil; let lifetime = generation
        defer { if lifetime == generation { busy = false } }
        do { _ = try await client.connect(authenticate: true); guard lifetime == generation else { return }; connected = true; await refresh(); listen() } catch { if lifetime == generation { failure(error) } }
    }
    func foreground(_ value: Bool) {
        active = value; events?.cancel(); if value && connected { Task { await refresh(); listen() } }
    }
    private func listen() {
        events?.cancel(); guard active, let client else { return }; let lifetime = generation
        events = Task { do { let stream = try await client.invalidations(); var last = ContinuousClock.now; for try await _ in stream { try Task.checkCancellation(); guard lifetime == self.generation else { return }; let elapsed = last.duration(to: .now); if elapsed < .milliseconds(250) { try await Task.sleep(for: .milliseconds(250)) }; await self.refresh(); last = .now } } catch is CancellationError {} catch { if lifetime == self.generation { self.failure(error) } } }
    }
    func refresh() async {
        guard let client, !refreshing else { return }; refreshing = true; let lifetime = generation; let selected = threadId
        defer { refreshing = false }
        do {
            async let nav = client.request(.navigationSnapshot)
            async let profiles = client.request(.membersList)
            async let missionList = client.request(.missionsList)
            async let canvasList = client.request(.canvasBoardsList)
            let (navValue, profileValue, missionValue, canvasValue) = try await (nav, profiles, missionList, canvasList)
            let opened = try await selected.asyncMap { try await client.request(.threadsOpen, params: .object(["threadId": .string($0), "traceId": .string(UUID().uuidString.lowercased())])) }
            guard lifetime == generation else { return }
            navigation = navValue; members = profileValue.rvArray; missions = missionValue.rvArray; boards = canvasValue["boards"]?.rvArray ?? []; if selected == threadId { thread = opened }; issue = nil
        } catch is CancellationError {} catch { if lifetime == generation { failure(error) } }
    }
    func open(_ threadId: String?) async { self.threadId = threadId; thread = nil; publicSources = []; publicSourcesReady = false; if let threadId { do { guard let profile else { return }; publicSources = try readPublicSources(threadId, profile: profile, account: account); publicSourcesReady = true; await refresh() } catch { issue = error.localizedDescription } } }
    private func sourceAccount(_ threadId: String, account: String) -> String { "body:" + account + ":" + SHA256.hash(data: Data(("public-attachments:" + threadId).utf8)).map { String(format: "%02x", $0) }.joined() }
    private func readPublicSources(_ threadId: String, profile: ServerProfile, account: String) throws -> [JSONValue] { guard let data = RovaiSecureSession.load(profile: profile.id, account: sourceAccount(threadId, account: account)) else { return [] }; let value = try JSONDecoder().decode(JSONValue.self, from: data); guard case .array(let sources) = value else { throw RovaiHostFailure(code: "附件草稿无法读取") }; return sources }
    private func savePublicSources(_ sources: [JSONValue], threadId: String, profile: ServerProfile, account: String, lifetime: UInt64) throws { let data = try JSONEncoder().encode(JSONValue.array(sources)); guard sources.count <= 32, data.count <= 64 * 1024, RovaiSecureSession.save(sources.isEmpty ? nil : data, profile: profile.id, account: sourceAccount(threadId, account: account)) else { throw RovaiHostFailure(code: "附件草稿未能保存，原请求仍保留") }; if lifetime == generation && self.threadId == threadId { publicSources = sources; publicSourcesReady = true } }
    func removePublicSource(_ id: String) { guard pending == nil, let profile, let threadId else { return }; do { try savePublicSources(readPublicSources(threadId, profile: profile, account: account).filter { $0.text("id") != id }, threadId: threadId, profile: profile, account: account, lifetime: generation) } catch { issue = error.localizedDescription } }
    private func settlePublicSend(_ request: JSONValue, result: JSONValue, profile: ServerProfile, account: String, lifetime: UInt64) throws {
        guard request.text("operation") == RovaiOperation.threadMessagesSend.rawValue else { return }; let receipt = result["commandResult"] == nil || result["commandResult"] == .null ? result : result["commandResult"]!
        if receipt.text("status") == "rejected" { return }; guard ["applied", "accepted"].contains(receipt.text("status")) else { throw RovaiHostFailure(code: "发送结果尚未确定") }
        let params = request["params"] ?? .null; let sent = Set((params["sourceAttachments"]?.rvArray ?? []).map { $0.text("id") }); let thread = params.text("threadId"); let current = try readPublicSources(thread, profile: profile, account: account)
        try savePublicSources(current.filter { !sent.contains($0.text("id")) }, threadId: thread, profile: profile, account: account, lifetime: lifetime)
    }
    func query(_ operation: RovaiOperation, params: JSONValue = .object([:])) async throws -> JSONValue { guard let client else { throw RovaiHostFailure(code: "尚未连接工作区") }; return try await client.request(operation, params: params) }
    private func bodyAccount(_ owner: String) -> String {
        "body:" + account + ":" + SHA256.hash(data: Data(owner.utf8)).map { String(format: "%02x", $0) }.joined()
    }
    func readBody(_ owner: String) throws -> String {
        guard let profile, !account.isEmpty else { throw RovaiHostFailure(code: "尚未连接工作区") }
        guard let data = RovaiSecureSession.load(profile: profile.id, account: bodyAccount(owner)) else { return "" }
        guard let body = String(data: data, encoding: .utf8) else { throw RovaiHostFailure(code: "草稿无法读取") }; return body
    }
    func saveBody(_ owner: String, body: String) throws {
        guard let profile, !account.isEmpty else { throw RovaiHostFailure(code: "尚未连接工作区") }
        let data = Data(body.utf8); guard data.count <= 1024 * 1024 else { throw RovaiHostFailure(code: "草稿超过1MB，请拆分后发送。") }
        guard RovaiSecureSession.save(data.isEmpty ? nil : data, profile: profile.id, account: bodyAccount(owner)) else { throw RovaiHostFailure(code: "草稿未能保存，输入仍在当前页面。") }
    }
    func upload(_ url: URL, threadId: String, target: JSONValue, revision: Int, expectedGeneration: UInt64) async {
        guard expectedGeneration == generation, pending == nil, !busy else { issue = "工作区已切换或仍有待核对操作，请重新选择附件"; return }
        busy = true; let lifetime = generation
        do { let prepared = try await RovaiUploadSource.prepare(url: url, threadId: threadId, target: target, revision: revision); guard lifetime == generation else { RovaiUploadSource.remove(prepared); return }; busy = false; await submit(.object(["operation": .string("__host.upload"), "params": prepared])) } catch { if lifetime == generation { issue = error.localizedDescription; busy = false } }
    }
    private func completeUpload(_ request: JSONValue, result: JSONValue, profile: ServerProfile, account: String, lifetime: UInt64) async throws {
        guard result["draft"] != nil && result["draft"] != .null || result["receipt"] != nil else { throw RovaiHostFailure(code: "上传结果尚未确定，请核对原上传") }
        let intent = request["params"]?["intent"] ?? .null
        if intent["target"]?.text("kind") == "camp", let source = result["draft"] { guard !source.text("id").isEmpty else { throw RovaiHostFailure(code: "上传来源回执不完整") }; var sources = try readPublicSources(intent.text("threadId"), profile: profile, account: account); if !sources.contains(where: { $0.text("id") == source.text("id") }) { sources.append(source) }; try savePublicSources(sources, threadId: intent.text("threadId"), profile: profile, account: account, lifetime: lifetime) }
        guard RovaiSecureSession.save(nil, profile: profile.id, account: account) else { throw RovaiHostFailure(code: "上传结果尚未保存，原请求仍保留") }
        RovaiUploadSource.remove(request["params"] ?? .null); guard lifetime == generation else { return }
        pending = nil; lastUploadResult = result; lastResult = result; lastOperation = nil; if result["receipt"]?.text("status") == "rejected" { issue = result["receipt"]?.text("code") }; await refresh()
    }
    private func dispatchUpload(_ request: JSONValue, client: RovaiHostClient, profile: ServerProfile, account: String, lifetime: UInt64) async throws {
        let params = request["params"] ?? .null; let intent = params["intent"] ?? .null
        if params["attempted"]?.rvBool == true {
            let result = try await client.uploadStatus(intent: intent, action: intent["target"]?.text("kind") == "camp" ? "reconcile" : "retry")
            if result["draft"] != nil || result.text("state") == "recorded" { try await completeUpload(request, result: result, profile: profile, account: account, lifetime: lifetime); return }
            guard ["unknown", "needs_source"].contains(result.text("state")) else { throw RovaiHostFailure(code: "原上传仍待核对") }
        }
        var fields = params.rvObject; fields["attempted"] = .bool(true); var changed = request.rvObject; changed["params"] = .object(fields); let marked = JSONValue.object(changed)
        guard RovaiSecureSession.save(try JSONEncoder().encode(marked), profile: profile.id, account: account) else { throw RovaiHostFailure(code: "无法保存原上传请求，尚未发送") }; if lifetime == generation { pending = marked }
        let data = try await RovaiUploadSource.bytes(params); let result = try await client.upload(intent: intent, data: data)
        try await completeUpload(marked, result: result, profile: profile, account: account, lifetime: lifetime)
    }
    func executePrivate(_ operation: RovaiOperation, command: JSONValue, owner: String, body: String, replace: Bool = false) async {
        guard pending == nil else { return }
        await submit(.object(["operation": .string(operation.rawValue), "params": .object(["commandId": .string(UUID().uuidString.lowercased()), "command": command]), "nativeBody": .object(["owner": .string(owner), "expectedBody": .string(body), "replace": .bool(replace)])]))
    }
    private func settlePrivate(_ request: JSONValue, result: JSONValue, profile: ServerProfile, account: String, client: RovaiHostClient) async throws {
        guard let local = request["nativeBody"], local != .null else { return }
        let receipt = result["commandResult"] == nil || result["commandResult"] == .null ? result : result["commandResult"]!
        if receipt.text("status") == "rejected" { return }
        guard ["applied", "accepted"].contains(receipt.text("status")) else { throw RovaiHostFailure(code: "私聊操作结果尚未确定") }
        let payload = receipt["payload"] ?? .object([:]); let replace = local["replace"]?.rvBool == true
        if replace && payload.text("placement") == "retained" { return }
        var body = ""
        if replace {
            guard let draft = payload["draft"], draft["scopeVersion"]?.rvInt == 1, draft.text("scopeKey").isEmpty, draft["composerScope"] == .null, let replacement = payload["body"]?.stringValue else { throw RovaiHostFailure(code: "私聊材料归属发生变化") }; body = replacement
            let command = request["params"]?["command"] ?? .null
            let snapshot = try await client.request(request.text("operation").hasPrefix("delegations.") ? .delegationsConversationGet : .singleChatGet, params: .object(["conversationId": command["conversationId"] ?? .null, "composerScope": .null]))
            guard snapshot["draft"]?["revision"] == draft["revision"] else { throw RovaiHostFailure(code: "材料版本已变化，请先核对原操作") }
        }
        let key = "body:" + account + ":" + SHA256.hash(data: Data(local.text("owner").utf8)).map { String(format: "%02x", $0) }.joined()
        let saved = RovaiSecureSession.load(profile: profile.id, account: key).flatMap { String(data: $0, encoding: .utf8) } ?? ""
        if !replace && saved != local.text("expectedBody") { return }
        guard saved == local.text("expectedBody") || saved == body else { throw RovaiHostFailure(code: "输入内容已变化，已保留当前草稿和原请求") }
        let data = Data(body.utf8); guard data.count <= 1024 * 1024, RovaiSecureSession.save(data.isEmpty ? nil : data, profile: profile.id, account: key) else { throw RovaiHostFailure(code: "私聊草稿尚未保存，原请求仍保留") }
    }
    func execute(_ operation: RovaiOperation, params: JSONValue) async {
        guard pending == nil else { return }
        await submit(.object(["operation": .string(operation.rawValue), "params": params]))
    }
    private func submit(_ request: JSONValue) async {
        guard !writing, let client, let profile, let params = request["params"] else { return }
        let operation = RovaiOperation(rawValue: request.text("operation")); guard operation != nil || request.text("operation") == "__host.upload" else { return }
        writing = true; busy = true; issue = nil; let lifetime = generation; let account = self.account
        defer { writing = false; if lifetime == generation { busy = false } }
        do {
            let bytes = try JSONEncoder().encode(request)
            guard RovaiSecureSession.save(bytes, profile: profile.id, account: account) else { throw RovaiHostFailure(code: "无法保存原请求，尚未向工作区发送。") }
            pending = request
            if request.text("operation") == "__host.upload" { try await dispatchUpload(request, client: client, profile: profile, account: account, lifetime: lifetime); return }
            let result = try await client.request(operation!, params: params)
            try await settlePrivate(request, result: result, profile: profile, account: account, client: client)
            try settlePublicSend(request, result: result, profile: profile, account: account, lifetime: lifetime)
            RovaiSecureSession.save(nil, profile: profile.id, account: account)
            guard lifetime == generation else { return }
            pending = nil; lastResult = result; lastOperation = operation; lastInputOwner = request["nativeBody"]?["owner"]?.stringValue; lastInputBody = request["nativeBody"]?["expectedBody"]?.stringValue
            let receipt = result["commandResult"] == .null ? result : result["commandResult"] ?? result
            if receipt.text("status") == "rejected" { issue = receipt.text("code") }
            await refresh()
        } catch is CancellationError {} catch { if lifetime == generation { failure(error) } }
    }
    func retry() async { if let pending { await submit(pending) } }
    func reconcile() async {
        guard !writing, let request = pending, let profile, let client, let params = request["params"] else { return }
        let lifetime = generation; let account = self.account
        do {
            if request.text("operation") == "__host.upload" { let result = try await client.uploadStatus(intent: params["intent"] ?? .null); try await completeUpload(request, result: result, profile: profile, account: account, lifetime: lifetime); return }
            guard let operation = RovaiOperation(rawValue: request.text("operation")) else { return }
            let result = try await client.reconcile(operation, params: params); guard lifetime == generation else { return }
            if result.text("state") == "recorded" { try await settlePrivate(request, result: result["result"] ?? .null, profile: profile, account: account, client: client); try settlePublicSend(request, result: result["result"] ?? .null, profile: profile, account: account, lifetime: lifetime); RovaiSecureSession.save(nil, profile: profile.id, account: account); pending = nil; lastResult = result["result"]; lastOperation = operation; lastInputOwner = request["nativeBody"]?["owner"]?.stringValue; lastInputBody = request["nativeBody"]?["expectedBody"]?.stringValue; await refresh() } else { issue = "原操作结果尚不明确，请稍后核对或重试原操作。" }
        } catch { if lifetime == generation { failure(error) } }
    }

    func command(_ operation: RovaiOperation, value: JSONValue) async { await execute(operation, params: .object(["commandId": .string(UUID().uuidString.lowercased()), "command": value])) }
    func publicBody(_ text: String, recipients: Set<String>, revision: Int) throws -> String {
        let encoder = JSONEncoder(); encoder.outputFormatting = [.sortedKeys]
        let value = JSONValue.object(["version": .number(1), "text": .string(text), "recipients": .array(recipients.sorted().map(JSONValue.string)), "revision": .number(Double(revision))])
        return String(decoding: try encoder.encode(value), as: UTF8.self)
    }
    func send(_ text: String, recipients: Set<String>, document: String, expectedThreadId: String) async {
        guard let threadId, threadId == expectedThreadId, !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, pending == nil, !busy else { return }
        guard let body = JSONValue.parse(document), body.text("text") == text, Set((body["recipients"]?.rvArray ?? []).compactMap(\.stringValue)) == recipients else { issue = "输入草稿发生变化"; return }
        var segments: [JSONValue] = recipients.sorted().map { .object(["kind": .string("atom"), "atom": .object(["type": .string("member"), "agentId": .string($0)])]) }
        segments.append(.object(["kind": .string("text"), "text": .string(text)]))
        let params = JSONValue.object(["commandId": .string(UUID().uuidString.lowercased()), "threadId": .string(threadId), "content": .object(["version": .number(2), "segments": .array(segments)]), "sourceAttachments": .array(publicSources), "quotes": .array([]), "replyToThreadMessageId": .null, "execution": .object(["taskId": .null, "purpose": .string(text.trimmingCharacters(in: .whitespacesAndNewlines)), "completionRole": .string("required")])])
        await submit(.object(["operation": .string(RovaiOperation.threadMessagesSend.rawValue), "params": params, "nativeBody": .object(["owner": .string("public:" + threadId), "expectedBody": .string(document), "replace": .bool(false)])]))
    }

    func terminal(boardId: String, nodeId: String) -> RovaiTerminalController? {
        let key = boardId + ":" + nodeId
        if let controller = terminals[key] { return controller }
        guard let client, terminals.count < 64 else { issue = "请先关闭其他终端后重试。"; return nil }
        let controller = RovaiTerminalController(client: client, boardId: boardId, nodeId: nodeId)
        terminals[key] = controller; return controller
    }
    func logout() async { events?.cancel(); terminals.values.forEach { $0.retire() }; terminals.removeAll(); do { try await client?.logout(); connected = false } catch { failure(error) } }
    private func failure(_ error: Error) { issue = error.localizedDescription; if issue?.contains("unauthorized") == true || issue?.contains("login_required") == true { connected = false; events?.cancel() } }
}
private extension Optional where Wrapped == String {
    func asyncMap<T>(_ transform: (String) async throws -> T) async rethrows -> T? { guard let value = self else { return nil }; return try await transform(value) }
}
