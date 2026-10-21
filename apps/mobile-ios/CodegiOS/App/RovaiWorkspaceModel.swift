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
    var lastOperation: RovaiOperation?
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
        generation &+= 1; events?.cancel(); client = nil; self.profile = profile; connected = false; thread = nil; threadId = nil; navigation = .object([:]); members = []; missions = []; boards = []; issue = nil; pending = nil
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
    func open(_ threadId: String?) async { self.threadId = threadId; thread = nil; if threadId != nil { await refresh() } }
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
        let data = Data(body.utf8); guard data.count <= 64 * 1024 else { throw RovaiHostFailure(code: "草稿超过64KB，请拆分后发送。") }
        guard RovaiSecureSession.save(data.isEmpty ? nil : data, profile: profile.id, account: bodyAccount(owner)) else { throw RovaiHostFailure(code: "草稿未能保存，输入仍在当前页面。") }
    }
    func execute(_ operation: RovaiOperation, params: JSONValue) async {
        guard pending == nil else { return }
        await submit(.object(["operation": .string(operation.rawValue), "params": params]))
    }
    private func submit(_ request: JSONValue) async {
        guard !writing, let client, let profile, let operation = RovaiOperation(rawValue: request.text("operation")), let params = request["params"] else { return }
        writing = true; busy = true; issue = nil; let lifetime = generation; let account = self.account
        defer { writing = false; if lifetime == generation { busy = false } }
        do {
            let bytes = try JSONEncoder().encode(request)
            guard RovaiSecureSession.save(bytes, profile: profile.id, account: account) else { throw RovaiHostFailure(code: "无法保存原请求，尚未向工作区发送。") }
            pending = request
            let result = try await client.request(operation, params: params)
            RovaiSecureSession.save(nil, profile: profile.id, account: account)
            guard lifetime == generation else { return }
            pending = nil; lastResult = result; lastOperation = operation
            let receipt = result["commandResult"] == .null ? result : result["commandResult"] ?? result
            if receipt.text("status") == "rejected" { issue = receipt.text("code") }
            await refresh()
        } catch is CancellationError {} catch { if lifetime == generation { failure(error) } }
    }
    func retry() async { if let pending { await submit(pending) } }
    func reconcile() async {
        guard !writing, let request = pending, let profile, let client, let operation = RovaiOperation(rawValue: request.text("operation")), let params = request["params"] else { return }
        let lifetime = generation
        do { let result = try await client.reconcile(operation, params: params); guard lifetime == generation else { return }; if result.text("state") == "recorded" { RovaiSecureSession.save(nil, profile: profile.id, account: account); pending = nil; lastResult = result["result"]; lastOperation = operation; await refresh() } else { issue = "原操作结果尚不明确，请稍后核对或重试原操作。" } } catch { if lifetime == generation { failure(error) } }
    }
    func command(_ operation: RovaiOperation, value: JSONValue) async { await execute(operation, params: .object(["commandId": .string(UUID().uuidString.lowercased()), "command": value])) }
    func send(_ text: String, recipients: Set<String>) async {
        guard let threadId, !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        var segments: [JSONValue] = recipients.sorted().map { .object(["kind": .string("atom"), "atom": .object(["type": .string("member"), "agentId": .string($0)])]) }
        segments.append(.object(["kind": .string("text"), "text": .string(text)]))
        await execute(.threadMessagesSend, params: .object(["commandId": .string(UUID().uuidString.lowercased()), "threadId": .string(threadId), "content": .object(["version": .number(2), "segments": .array(segments)]), "sourceAttachments": .array([]), "quotes": .array([]), "replyToThreadMessageId": .null, "execution": .object(["taskId": .null, "purpose": .string(text.trimmingCharacters(in: .whitespacesAndNewlines)), "completionRole": .string("required")])]))
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
