import SwiftUI

private struct AutomationMissionRow: Identifiable {let value: JSONValue;var id: String {value.text("occurrenceId")}}

/// 复用原编辑表单的 Section 和滚动区域，保存时核对定义与附表两个版本。
struct RovaiAutomationConversationDetails: View {
    let model: RovaiWorkspaceModel
    let automationId: String
    let enabled: Bool
    let onVersionChange: (Int) -> Void
    let onOpenThread: (String) -> Void
    @State private var owner: JSONValue?, pending: JSONValue?
    @State private var action = "launch_session", timezone = ""
    @State private var rows: [JSONValue] = []
    @State private var next: String?, issue: String?
    @State private var busy = false
    @State private var generation: UInt64 = 0
    private var storage: String {"conversation-automation:\(automationId)"}
    var body: some View {
        Group {
            if owner != nil || pending != nil || issue != nil {
                Section("执行方式与时区") {
                    if owner != nil {
                        Picker("计划到期后", selection: $action) {Text("启动队员会话").tag("launch_session");Text("生成待启动的使命").tag("enqueue_task")}.disabled(!enabled || busy || pending != nil)
                        TextField("计划时区，如 Asia/Shanghai", text: $timezone).autocorrectionDisabled().textInputAutocapitalization(.never).disabled(!enabled || busy || pending != nil)
                        Text("生成的使命等待你启动。主机按此时区计算，夏令时重复时刻只执行一次，跳过不存在的时刻。").font(.caption).foregroundStyle(Theme.textSecondary)
                        if pending == nil {Button("保存执行方式与时区") {work {try await save()}}.disabled(!enabled || busy || timezone.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)}
                    }
                    if pending != nil {Button("核对原保存请求") {work {try await reconcile()}}.disabled(busy)}
                    Button("重新载入已保存的计划设置") {work {try await load()}}.disabled(busy || pending != nil)
                    if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning)}
                }
                if !rows.isEmpty {Section("按计划生成的使命") {
                    ForEach(rows.map {AutomationMissionRow(value: $0)}) {row in
                        Button {onOpenThread(row.value.text("threadId"))} label: {VStack(alignment: .leading, spacing: 6) {Text(row.value.text("title"));Text(row.value.text("scheduledAtUtc")).font(.caption).foregroundStyle(Theme.textSecondary);Text("打开使命").font(.caption)}}
                    }
                    if next != nil {Button("更早的使命") {work {try await load(more: true)}}.disabled(busy)}
                }}
            }
        }.task(id: automationId) {
            guard UUID(uuidString: automationId) != nil else {return};generation = model.connectionGeneration;busy = true;defer {busy = false}
            do {let raw = try model.readBody(storage);if !raw.isEmpty {guard raw.utf8.count <= 1024, let value = JSONValue.parse(raw), Set(value.rvObject.keys) == ["operation", "commandId"], value.text("operation") == "conversationTools.automationOwner.set", UUID(uuidString: value.text("commandId")) != nil else {throw RovaiHostFailure(code: "原保存请求记录损坏")};pending = value};try await load()} catch {issue = error.localizedDescription}
        }
    }
    @MainActor private func current() throws {guard generation == model.connectionGeneration else {throw CancellationError()}}
    @MainActor private func call(_ operation: RovaiOperation, _ value: JSONValue) async throws -> JSONValue {try current();let result = try await nativeUserCall(model, operation, value);try current();return result}
    @MainActor private func work(_ task: @escaping @MainActor () async throws -> Void) {guard !busy else {return};busy = true;issue = nil;Task {defer {if generation == model.connectionGeneration {busy = false}};do {try await task()} catch is CancellationError {} catch {if generation == model.connectionGeneration {issue = error.localizedDescription}}}}
    @MainActor private func load(more: Bool = false) async throws {
        var input: [String: JSONValue] = ["automationId": .string(automationId)];if more {input["queuedMissionAfter"] = next.map(JSONValue.string) ?? .null}
        let value = try await call(.automationsGet, .object(input))
        if !more {owner = value["conversationToolOwner"];if let owner {action = owner.text("action");timezone = owner.text("timezone");onVersionChange(owner["automationVersion"]?.rvInt ?? 0)}}
        let items = value["queuedMissions"]?["items"]?.rvArray ?? [];rows = more ? rows + items.filter {item in !rows.contains {$0.text("occurrenceId") == item.text("occurrenceId")}} : items;next = value["queuedMissions"]?["next"]?.stringValue
    }
    @MainActor private func clear(_ original: JSONValue) throws {guard try model.clearBodyIfMatches(storage, expectedBody: original.prettyString, expectedGeneration: generation) else {throw RovaiHostFailure(code: "原保存请求记录已变化")};pending = nil}
    @MainActor private func save() async throws {
        guard let owner else {return};let original = JSONValue.object(["operation": .string("conversationTools.automationOwner.set"), "commandId": .string(UUID().uuidString.lowercased())]);try model.saveBody(storage, body: original.prettyString);try current();pending = original
        let value = try await call(.conversationToolsAutomationOwnerSet, .object(["commandId": original["commandId"] ?? .null, "automationId": .string(automationId), "expectedAutomationVersion": owner["automationVersion"] ?? .null, "expectedOwnerRevision": owner["revision"] ?? .null, "action": .string(action), "timezone": .string(timezone)]));try clear(original);onVersionChange(value["automationVersion"]?.rvInt ?? 0);try await load()
    }
    @MainActor private func reconcile() async throws {
        guard let pending else {return};let value = try await call(.commandsReconcile, .object(["operation": pending["operation"] ?? .null, "params": .object(["commandId": pending["commandId"] ?? .null])]))
        guard value.text("state") == "recorded", let result = value["result"], result.text("commandId") == pending.text("commandId"), result.text("commandType") == pending.text("operation") else {throw RovaiHostFailure(code: "原保存请求没有确定回执，没有再次修改计划")}
        try clear(pending);guard result.text("status") != "rejected" else {throw RovaiHostFailure(code: result.text("code"))};try await load()
    }
}
