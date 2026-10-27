import SwiftUI

/// 复用原 NativeRuntimeEditor 的 Form/Picker；原生安装和队员版本均由 Core 返回。
struct CoaxiPortableMemberConnection: View {
    let model: CoaxiWorkspaceModel; let agentId: String; let disabled: Bool
    @State private var binding: JSONValue = .object([:]); @State private var providers: [JSONValue] = []
    @State private var next: JSONValue = .null; @State private var selected = "default"; @State private var identity = ""
    @State private var pending: JSONValue?; @State private var busy = false; @State private var issue: String?
    private var owner: String { "portable-member/\(agentId)/\(identity)" }
    private var original: JSONValue { binding["binding"]?["connection"] ?? .null }
    private var kind: String {
        switch binding["runtime"]?.text("adapterKind") { case "claude-code-cli": "claude_code"; case "codex-cli": "codex"; case "gemini-cli": "gemini"; default: "" }
    }
    private var available: [JSONValue] { providers.filter { $0.text("agentType") == kind && $0["enabled"] == .bool(true) } }
    private var connection: JSONValue {
        if selected == "default" { return .null }; if selected == "original" { return original }
        guard let value = available.first(where: { $0.text("id") == selected }) else { return .null }
        return .object(["id": value["id"] ?? .null, "revision": value["revision"] ?? .null, "digest": value["digest"] ?? .null])
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("明确选择后只用于之后的新任务；原任务保留原服务商版本。").font(.caption).foregroundStyle(Theme.textSecondary)
            if binding["runtime"] == .null {
                Text("原运行程序在当前主机尚不可用。请先保存上面的本机程序设置，再刷新连接；旧任务保留原连接。").font(.caption).foregroundStyle(Theme.textSecondary)
            }
            Picker("服务商", selection: $selected) {
                Text("使用程序原有配置").tag("default")
                if original != .null { Text("保留原连接版本").tag("original") }
                ForEach(available.map { CoaxiRow(value: $0) }) { row in Text(row.value.text("name")).tag(row.value.text("id")) }
            }.disabled(disabled || busy || pending != nil || kind.isEmpty)
            HStack {
                Button("刷新") { work { try await load() } }.disabled(busy || disabled || pending != nil)
                if next != .null { Button("加载更多") { work { try await load(more: true) } }.disabled(busy || disabled || pending != nil) }
                Button("保存连接") { work { try await save() } }.disabled(busy || disabled || pending != nil || connection == original || kind.isEmpty)
            }
            if pending != nil { Text("原请求待核对，未重复发送。").font(.caption); Button("核对原请求") { work { try await reconcile() } }.disabled(busy) }
            if disabled { Text("请先保存或取消上面的运行设置。").font(.caption).foregroundStyle(Theme.textSecondary) }
            if let issue { Text(issue).foregroundStyle(.red) }; if busy { ProgressView() }
        }.task(id: agentId) {
            busy = true
            do {
                identity = try await model.presentationIdentity()
                let raw = try model.readBody(owner)
                if !raw.isEmpty {
                    guard let value = JSONValue.parse(raw), case .object(let fields) = value, Set(fields.keys) == Set(["operation", "commandId", "targetId"]), value.text("operation") == "configuration.memberConnection.set", value.text("targetId") == agentId, UUID(uuidString: value.text("commandId")) != nil else { throw CoaxiHostFailure(code: "原请求恢复记录无效") }
                    pending = value
                }
                try await load()
            } catch is CancellationError {} catch { issue = error.localizedDescription }; busy = false
        }
    }
    private func current() async throws { guard !identity.isEmpty, try await model.presentationIdentity() == identity else { throw CancellationError() } }
    private func work(_ action: @escaping () async throws -> Void) {
        guard !busy else { return }; busy = true; issue = nil
        Task { do { try await action() } catch is CancellationError {} catch { issue = error.localizedDescription }; busy = false }
    }
    private func load(more: Bool = false) async throws {
        try await current()
        if !more { let value = try await model.query(.configurationMemberConnectionGet, params: .object(["schemaVersion": .number(1), "agentId": .string(agentId)])); try await current(); binding = value; selected = original == .null ? "default" : "original" }
        let page = try await model.query(.configurationProvidersList, params: .object(["schemaVersion": .number(1), "after": more ? next : .null, "limit": .number(100)]))
        try await current(); providers = more ? providers + (page["items"]?.rvArray ?? []) : page["items"]?.rvArray ?? []; next = page["next"] ?? .null
    }
    private func remember(_ value: JSONValue?) async throws { try await current(); try model.saveBody(owner, body: value?.prettyString ?? ""); pending = value }
    private func accept(_ result: JSONValue) async throws {
        guard let pending, result.text("commandId") == pending.text("commandId"), result.text("commandType") == "configuration.memberConnection.set", ["applied", "rejected"].contains(result.text("status")) else { throw CoaxiHostFailure(code: "原请求尚无匹配的确定回执") }
        if let target = result["payload"]?["agentId"], target != .null, target != .string(agentId) { throw CoaxiHostFailure(code: "原请求队员不同") }
        let rejected = result.text("status") == "rejected"; try await remember(nil)
        if rejected { throw CoaxiHostFailure(code: "设置被拒绝，请刷新后再选择") }
        try await load(); await model.refresh()
    }
    private func save() async throws {
        try await current(); guard pending == nil, !disabled, !kind.isEmpty else { throw CoaxiHostFailure(code: "请先处理原请求或运行设置") }
        let command = UUID().uuidString.lowercased(), selectedConnection = connection
        try await remember(.object(["operation": .string("configuration.memberConnection.set"), "commandId": .string(command), "targetId": .string(agentId)]))
        let result = try await model.query(.configurationMemberConnectionSet, params: .object(["schemaVersion": .number(1), "commandId": .string(command), "agentId": .string(agentId), "expectedVersion": binding["agentVersion"] ?? .null, "connection": selectedConnection]))
        try await current(); try await accept(result)
    }
    private func reconcile() async throws {
        guard let pending else { return }
        let value = try await model.query(.commandsReconcile, params: .object(["operation": .string("configuration.memberConnection.set"), "params": .object(["commandId": pending["commandId"] ?? .null])]))
        try await current(); guard value.text("state") == "recorded", let result = value["result"] else { throw CoaxiHostFailure(code: "原请求尚无确定回执，请稍后继续核对") }; try await accept(result)
    }
}
