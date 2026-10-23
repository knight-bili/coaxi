import SwiftUI

private let conversationFeatures = [("question", "向你提问"), ("sessionInfo", "参考本次选中的旧讨论"), ("automations", "安排后续工作"), ("workTasks", "创建未启动使命")]
private var emptyConversationFlags: JSONValue { .object(Dictionary(uniqueKeysWithValues: conversationFeatures.map {($0.0, .bool(false))})) }

/// 沿原移动队员设置表单；本地只保存原编号，不复制第二套权限。
struct RovaiConversationToolSettings: View {
    let model: RovaiWorkspaceModel; let agentId: String
    @State private var policy: JSONValue?
    @State private var pending: JSONValue?
    @State private var issue: String?
    @State private var generation: UInt64?
    @State private var busy = false
    private var owner: String {"conversation-tools-policy:" + agentId}
    private var global: JSONValue {policy?["globalFlags"] ?? emptyConversationFlags}
    private var flags: JSONValue {(policy?["members"]?.rvArray ?? []).first {$0.text("agentId") == agentId}?["flags"] ?? emptyConversationFlags}
    private var blocked: Bool {busy || policy == nil || pending != nil || model.busy || model.pending != nil || generation != model.connectionGeneration}
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            DisclosureGroup("全局工具开关") {
                ForEach(conversationFeatures, id: \.0) { key, label in Toggle("允许分配：" + label, isOn: Binding(get: {global[key] == .bool(true)}, set: {checked in var value = global.rvObject; value[key] = .bool(checked); Task {await save(global: .object(value))}})).disabled(blocked) }
                Picker("文字与卡片反馈总开关", selection: Binding(get: {policy?["feedbackGate"] == .null ? "inherit" : policy?["feedbackGate"] == .bool(true) ? "true" : "false"}, set: {value in Task {await save(feedback: value == "inherit" ? .null : .bool(value == "true"))}})) {Text("沿用原反馈设置").tag("inherit"); Text("新执行关闭反馈").tag("false"); Text("允许原已开启队员").tag("true")}.disabled(blocked)
            }
            ForEach(conversationFeatures, id: \.0) { key, label in
                Toggle(isOn: Binding(get: {flags[key] == .bool(true)}, set: {checked in var value = flags.rvObject; value[key] = .bool(checked); Task {await save(memberFlags: .object(value))}})) {VStack(alignment: .leading) {Text(label); if global[key] != .bool(true) {Text("总开关当前关闭").font(.caption).foregroundStyle(Theme.textSecondary)}}}.disabled(blocked)
            }
            Text("总开关与此队员分配同时开启才生效，仍需程序支持。只影响新任务；原Rovai内置工具继续使用自己的授权。").font(.caption).foregroundStyle(Theme.textSecondary)
            if pending != nil {Button("核对原保存请求") {Task {await reconcile()}}.disabled(busy || generation != model.connectionGeneration)}
            if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning)}
        }.task(id: agentId) {
            generation = model.connectionGeneration;busy = true;defer {busy = false}
            do {let raw = try model.readBody(owner);if !raw.isEmpty {guard raw.utf8.count <= 1024 else {throw RovaiHostFailure(code: "原请求记录过大")};let value = try JSONDecoder().decode(JSONValue.self, from: Data(raw.utf8));guard Set(value.rvObject.keys) == ["operation", "commandId"], value.text("operation") == "conversationTools.policy.set", UUID(uuidString: value.text("commandId")) != nil else {throw RovaiHostFailure(code: "原请求记录损坏")};pending = value};policy = try await nativeUserCall(model, .conversationToolsPolicyGet, .object([:]))} catch {issue = error.localizedDescription}
        }
    }
    private func save(global: JSONValue? = nil, memberFlags: JSONValue? = nil, feedback: JSONValue? = nil) async {
        guard !blocked, let base = policy, let generation else {return};busy = true;issue = nil;defer {busy = false}
        let command = UUID().uuidString.lowercased(), original = JSONValue.object(["operation": .string("conversationTools.policy.set"), "commandId": .string(command)])
        let members = memberFlags.map {flags in (base["members"]?.rvArray ?? []).filter {$0.text("agentId") != agentId} + [.object(["agentId": .string(agentId), "flags": flags])]} ?? (base["members"]?.rvArray ?? [])
        let input = JSONValue.object(["commandId": .string(command), "expectedRevision": base["revision"] ?? .number(0), "globalFlags": global ?? base["globalFlags"] ?? emptyConversationFlags, "feedbackGate": feedback ?? base["feedbackGate"] ?? .null, "members": .array(members)])
        do {let raw = String(decoding: try JSONEncoder().encode(original), as: UTF8.self);try model.saveBody(owner, body: raw);guard generation == model.connectionGeneration else {throw CancellationError()};pending = original;let value = try await nativeUserCall(model, .conversationToolsPolicySet, input);guard try model.clearBodyIfMatches(owner, expectedBody: raw, expectedGeneration: generation) else {throw RovaiHostFailure(code: "原请求记录已变化")};pending = nil;policy = value} catch {if generation == model.connectionGeneration {issue = error.localizedDescription}}
    }
    private func reconcile() async {
        guard !busy, let pending, let generation, generation == model.connectionGeneration else {return};busy = true;issue = nil;defer {busy = false}
        do {let value = try await nativeUserCall(model, .commandsReconcile, .object(["operation": .string("conversationTools.policy.set"), "params": .object(["commandId": pending["commandId"] ?? .null]) ]));guard value.text("state") == "recorded", let result = value["result"], result.text("commandId") == pending.text("commandId"), result.text("commandType") == "conversationTools.policy.set" else {throw RovaiHostFailure(code: "原设置还没有确定回执，没有重复保存")};let raw = String(decoding: try JSONEncoder().encode(pending), as: UTF8.self);guard try model.clearBodyIfMatches(owner, expectedBody: raw, expectedGeneration: generation) else {throw RovaiHostFailure(code: "原请求记录已变化")};self.pending = nil;if result.text("status") == "rejected" {policy = try await nativeUserCall(model, .conversationToolsPolicyGet, .object([:]));throw RovaiHostFailure(code: result.text("code"))};policy = result["payload"]} catch {if generation == model.connectionGeneration {issue = error.localizedDescription}}
    }
}
