import SwiftUI
import Observation

private let portableProviderTypes = [("claude_code", "Claude Code"), ("codex", "Codex"), ("gemini", "Gemini")]
private let portableClaudeFields = [("main", "主模型"), ("reasoning", "推理模型"), ("haiku", "Haiku"), ("sonnet", "Sonnet"), ("opus", "Opus"), ("fable", "Fable"), ("customOption", "自定义选项"), ("customOptionName", "选项名称"), ("customOptionDescription", "选项说明")]
private func portableField(_ value: JSONValue, _ key: String, _ field: JSONValue) -> JSONValue {
    guard case .object(var object) = value else { return value }; object[key] = field; return .object(object)
}
private func portableProviderModel(_ type: String) -> JSONValue {
    if type == "claude_code" { return .object(Dictionary(uniqueKeysWithValues: [("kind", JSONValue.string("claude"))] + portableClaudeFields.map { ($0.0, .string("")) })) }
    if type == "codex" { return .object(["kind": .string(type), "customs": .array([]), "excludedOfficials": .array([]), "default": .null]) }
    return .object(["kind": .string("gemini"), "model": .null])
}

@MainActor @Observable
private final class PortableProvidersController {
    let workspace: CoaxiWorkspaceModel
    var rows: [JSONValue] = []; var next: JSONValue = .null
    var filter = ""; var busy = false; var issue: String?; var pending: JSONValue?
    var editing: JSONValue?; var removal: JSONValue?; var identity = ""
    private var active = true
    init(_ model: CoaxiWorkspaceModel) { workspace = model }
    private var owner: String { "portable-providers/" + identity }
    private func current() async throws { guard active, !identity.isEmpty, try await workspace.presentationIdentity() == identity else { throw CancellationError() } }
    func stop() { active = false; editing = nil }
    func start() async {
        active = true; rows = []; next = .null; editing = nil; removal = nil; pending = nil; identity = ""
        guard workspace.connected else { return }
        await work {
            self.identity = try await self.workspace.presentationIdentity()
            let raw = try self.workspace.readBody(self.owner)
            if !raw.isEmpty {
                guard let value = JSONValue.parse(raw), case .object(let fields) = value,
                      Set(fields.keys) == Set(["operation", "commandId", "targetId"]),
                      ["configuration.providers.upsert", "configuration.providers.remove"].contains(value.text("operation")),
                      UUID(uuidString: value.text("commandId")) != nil, UUID(uuidString: value.text("targetId")) != nil else { throw CoaxiHostFailure(code: "原请求恢复记录无效") }
                self.pending = value
            }
            try await self.load()
        }
    }
    func work(_ action: () async throws -> Void) async {
        guard !busy else { return }; busy = true; issue = nil
        do { try await action() } catch is CancellationError {} catch { issue = error.localizedDescription }
        busy = false
    }
    func load(more: Bool = false) async throws {
        try await current()
        let page = try await workspace.query(.configurationProvidersList, params: .object(["schemaVersion": .number(1), "after": more ? next : .null, "limit": .number(100)]))
        try await current(); rows = more ? rows + (page["items"]?.rvArray ?? []) : page["items"]?.rvArray ?? []; next = page["next"] ?? .null
    }
    private func remember(_ value: JSONValue?) async throws {
        try await current(); try workspace.saveBody(owner, body: value?.prettyString ?? ""); pending = value
    }
    private func accept(_ result: JSONValue) async throws {
        guard let pending, result.text("commandId") == pending.text("commandId"), result.text("commandType") == pending.text("operation"), ["applied", "rejected"].contains(result.text("status")) else { throw CoaxiHostFailure(code: "原请求尚无匹配的确定回执，未重复发送") }
        if let id = result["payload"]?["id"], id != .null, id != pending["targetId"] { throw CoaxiHostFailure(code: "原请求目标不同") }
        let rejected = result.text("status") == "rejected"; try await remember(nil)
        if rejected { throw CoaxiHostFailure(code: "保存被拒绝，请刷新后核对设置") }
        editing = nil; removal = nil; try await load()
    }
    func reconcile() async throws {
        guard let pending else { return }
        let value = try await workspace.query(.commandsReconcile, params: .object(["operation": pending["operation"] ?? .null, "params": .object(["commandId": pending["commandId"] ?? .null])]))
        try await current(); guard value.text("state") == "recorded", let result = value["result"] else { throw CoaxiHostFailure(code: "原请求尚无确定回执，请稍后继续核对") }; try await accept(result)
    }
    func edit(_ value: JSONValue) async throws {
        let result = try await workspace.privateConfiguration("provider", params: .object(["id": value["id"] ?? .null, "revision": value["revision"] ?? .null, "digest": value["digest"] ?? .null]))
        try await current(); editing = result
    }
    func add() { editing = .object(["schemaVersion": .number(1), "id": .string(UUID().uuidString.lowercased()), "revision": .number(0), "name": .string(""), "agentType": .string("claude_code"), "apiUrl": .string(""), "credential": .null, "rawAgentTypesJson": .string(""), "rawModel": .null, "model": portableProviderModel("claude_code"), "enabled": .bool(true)]) }
    func save(_ source: JSONValue, original: JSONValue, keyMode: String, key: String, operation: String) async throws {
        try await current(); guard pending == nil else { throw CoaxiHostFailure(code: "请先核对原请求") }
        guard case .object(var object) = source else { throw CoaxiHostFailure(code: "设置无效") }
        for field in ["revision", "digest", "createdAt", "updatedAt"] { object.removeValue(forKey: field) }
        if keyMode == "replace" {
            let secret = try await workspace.privateConfiguration("secrets", params: .object(["schemaVersion": .number(1), "operationId": .string(operation), "purpose": .string("provider_key"), "value": .string(key)]))
            try await current(); guard let reference = secret["reference"] else { throw CoaxiHostFailure(code: "密钥引用缺失") }; object["credential"] = reference
        } else if keyMode == "clear" { object["credential"] = .null }
        let command = UUID().uuidString.lowercased()
        try await remember(.object(["operation": .string("configuration.providers.upsert"), "commandId": .string(command), "targetId": object["id"] ?? .null]))
        let result = try await workspace.query(.configurationProvidersUpsert, params: .object(["schemaVersion": .number(1), "commandId": .string(command), "expectedRevision": original["revision"] ?? .number(0), "definition": .object(object)]))
        try await current(); try await accept(result)
    }
    func remove(_ value: JSONValue) async throws {
        try await current(); guard pending == nil else { throw CoaxiHostFailure(code: "请先核对原请求") }
        let command = UUID().uuidString.lowercased()
        try await remember(.object(["operation": .string("configuration.providers.remove"), "commandId": .string(command), "targetId": value["id"] ?? .null]))
        let result = try await workspace.query(.configurationProvidersRemove, params: .object(["schemaVersion": .number(1), "commandId": .string(command), "id": value["id"] ?? .null, "expectedRevision": value["revision"] ?? .null]))
        try await current(); try await accept(result)
    }
}

/// 沿原 ModelProvidersSettingsView 与 ModelProviderEditorSheet 的原生列表、表单和 sheet。
struct CoaxiPortableProviderSettings: View {
    let model: CoaxiWorkspaceModel
    @State private var controller: PortableProvidersController
    init(model: CoaxiWorkspaceModel) { self.model = model; _controller = State(initialValue: PortableProvidersController(model)) }
    var body: some View {
        List {
            Section {
                Picker("类型", selection: $controller.filter) { Text("全部").tag(""); ForEach(portableProviderTypes, id: \.0) { Text($0.1).tag($0.0) } }
                if controller.pending != nil { Text("上次请求尚待核对，未重复发送。"); Button("核对原请求") { Task { await controller.work { try await controller.reconcile() } } } }
                if let issue = controller.issue { Text(issue).foregroundStyle(.red) }
                if controller.busy { ProgressView() }
            }
            if controller.rows.isEmpty && !controller.busy { Section { Text("还没有服务商。新增后，到队员的运行设置中明确选择。").foregroundStyle(Theme.textSecondary) } }
            ForEach(portableProviderTypes, id: \.0) { type in
                if controller.filter.isEmpty || controller.filter == type.0 {
                    Section(type.1) {
                        ForEach(controller.rows.filter { $0.text("agentType") == type.0 }.map { CoaxiRow(value: $0) }) { row in
                            Button { Task { await controller.work { try await controller.edit(row.value) } } } label: {
                                HStack {
                                    VStack(alignment: .leading, spacing: 6) { Text(row.value.text("name")).font(.headline).foregroundStyle(Theme.textPrimary); Text(row.value.text("apiUrl")).font(.caption).foregroundStyle(Theme.textSecondary).lineLimit(2) }
                                    Spacer(); Image(systemName: "chevron.right").foregroundStyle(Theme.textTertiary)
                                }
                            }.contextMenu { Button("编辑") { Task { await controller.work { try await controller.edit(row.value) } } }; Button("移除", role: .destructive) { controller.removal = row.value } }
                        }
                    }
                }
            }
            if controller.next != .null { Button("加载更多") { Task { await controller.work { try await controller.load(more: true) } } } }
        }
        .scrollContentBackground(.hidden).navigationTitle("模型服务商")
        .toolbar { ToolbarItem(placement: .topBarTrailing) { Button { controller.add() } label: { Image(systemName: "plus") }.accessibilityLabel("新增服务商").disabled(controller.busy || controller.pending != nil || controller.identity.isEmpty) } }
        .disabled(controller.busy)
        .task(id: model.connected) { await controller.start() }
        .onDisappear { controller.stop() }
        .refreshable { await controller.work { try await controller.load() } }
        .sheet(isPresented: Binding(get: { controller.editing != nil }, set: { if !$0 { controller.editing = nil } })) {
            if let original = controller.editing { PortableProviderEditor(original: original, blocked: controller.pending != nil, save: { source, mode, key, operation in try await controller.save(source, original: original, keyMode: mode, key: key, operation: operation) }) }
        }
        .confirmationDialog("移除服务商", isPresented: Binding(get: { controller.removal != nil }, set: { if !$0 { controller.removal = nil } }), titleVisibility: .visible) {
            if let provider = controller.removal { Button("移除 " + provider.text("name"), role: .destructive) { Task { await controller.work { try await controller.remove(provider) } } } }
        } message: { Text("移除后不再用于新的选择，历史任务仍保留原版本。") }
    }
}

private struct PortableProviderEditor: View {
    let original: JSONValue; let blocked: Bool
    let save: (JSONValue, String, String, String) async throws -> Void
    @State private var value: JSONValue; @State private var key = ""; @State private var keyMode = "keep"
    @State private var secretOperation = UUID().uuidString.lowercased(); @State private var busy = false
    @State private var issue: String?; @State private var discard = false
    @Environment(\.dismiss) private var dismiss
    init(original: JSONValue, blocked: Bool, save: @escaping (JSONValue, String, String, String) async throws -> Void) { self.original = original; self.blocked = blocked; self.save = save; _value = State(initialValue: original) }
    private var type: String { value.text("agentType") }
    private var settings: JSONValue { value["model"] ?? .object([:]) }
    private var dirty: Bool { value != original || !key.isEmpty || keyMode != "keep" }
    private func field(_ name: String) -> Binding<String> { Binding(get: { value.text(name) }, set: { value = portableField(value, name, .string($0)) }) }
    private func modelField(_ name: String) -> Binding<String> { Binding(get: { settings.text(name) }, set: { value = portableField(value, "model", portableField(settings, name, $0.isEmpty && ["model", "default"].contains(name) ? .null : .string($0))) }) }
    private func close() { if dirty { discard = true } else { dismiss() } }
    var body: some View {
        NavigationStack {
            Form {
                Section("服务商") {
                    TextField("名称", text: field("name"))
                    if original["revision"]?.rvInt == 0 { Picker("类型", selection: Binding(get: { type }, set: { value = portableField(portableField(value, "agentType", .string($0)), "model", portableProviderModel($0)) })) { ForEach(portableProviderTypes, id: \.0) { Text($0.1).tag($0.0) } } }
                    TextField("接口地址", text: field("apiUrl")).textInputAutocapitalization(.never).autocorrectionDisabled()
                    Toggle("允许新的选择", isOn: Binding(get: { value["enabled"] == .bool(true) }, set: { value = portableField(value, "enabled", .bool($0)) }))
                }
                Section("API密钥") {
                    Picker("处理方式", selection: $keyMode) { Text("保留").tag("keep"); Text("替换").tag("replace"); Text("清空").tag("clear") }.pickerStyle(.segmented)
                    if keyMode == "replace" { SecureField("API密钥", text: $key).textInputAutocapitalization(.never).autocorrectionDisabled().onChange(of: key) { _, _ in secretOperation = UUID().uuidString.lowercased() } }
                }
                if type == "claude_code" { Section("模型") { ForEach(portableClaudeFields, id: \.0) { TextField($0.1, text: modelField($0.0)).textInputAutocapitalization(.never).autocorrectionDisabled() } } }
                else if type == "gemini" { Section("模型") { TextField("模型（可选）", text: modelField("model")) } }
                else { codex }
                if let issue { Section { Text(issue).foregroundStyle(.red) } }
            }.disabled(busy || blocked)
            .navigationTitle(original["revision"]?.rvInt == 0 ? "新增服务商" : "编辑服务商").navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("取消", action: close).disabled(busy) }
                ToolbarItem(placement: .confirmationAction) { Button("保存") { Task {
                    busy = true; issue = nil
                    do { var candidate = value; try await save(candidate, keyMode, key, secretOperation); key = ""; dismiss() }
                    catch is CancellationError {} catch { issue = error.localizedDescription }; busy = false
                } }.disabled(busy || blocked || value.text("name").trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || value.text("apiUrl").isEmpty || (keyMode == "replace" && key.isEmpty)) }
            }
        }.interactiveDismissDisabled(dirty || busy)
        .confirmationDialog("放弃本次编辑？", isPresented: $discard, titleVisibility: .visible) { Button("放弃编辑", role: .destructive) { dismiss() }; Button("继续编辑", role: .cancel) {} } message: { Text("未保存的地址、模型和密钥输入会丢弃。") }
    }
    private var codex: some View {
        Section("模型目录") {
            ForEach(Array((settings["customs"]?.rvArray ?? []).enumerated()), id: \.offset) { index, row in
                DisclosureGroup(row.text("displayName").isEmpty ? "自定义模型 \(index + 1)" : row.text("displayName")) {
                    TextField("模型编号", text: customField(index, "slug")); TextField("显示名称", text: customField(index, "displayName")); TextField("基于官方模型", text: customField(index, "base")); TextField("上下文窗口（可选）", text: customField(index, "contextWindow")).keyboardType(.numberPad)
                    CoaxiPortableCodexOverrides(value: row["overrides"] ?? .object([:]), disabled: blocked || busy) { overrides in
                        var rows = settings["customs"]?.rvArray ?? []; guard rows.indices.contains(index) else { return }; rows[index] = portableField(rows[index], "overrides", overrides); value = portableField(value, "model", portableField(settings, "customs", .array(rows)))
                    }
                    Button("移除此模型", role: .destructive) { var rows = settings["customs"]?.rvArray ?? []; rows.remove(at: index); value = portableField(value, "model", portableField(settings, "customs", .array(rows))) }
                }
            }
            Button("添加自定义模型") { var rows = settings["customs"]?.rvArray ?? []; rows.append(.object(["slug": .string(""), "displayName": .string(""), "contextWindow": .null, "base": .string(""), "overrides": .object([:])])); value = portableField(value, "model", portableField(settings, "customs", .array(rows))) }
            TextField("默认模型（可选）", text: modelField("default"))
            TextField("隐藏的官方模型（每行一个）", text: Binding(get: { (settings["excludedOfficials"]?.rvArray ?? []).map { if case .string(let name) = $0 { name } else { "" } }.joined(separator: "\n") }, set: { value = portableField(value, "model", portableField(settings, "excludedOfficials", .array($0.split(separator: "\n").map { .string($0.trimmingCharacters(in: .whitespaces)) }))) }), axis: .vertical)
        }
    }
    private func customField(_ index: Int, _ field: String) -> Binding<String> {
        Binding(get: { let rows = settings["customs"]?.rvArray ?? []; guard rows.indices.contains(index) else { return "" }; if field == "contextWindow", let count = rows[index][field]?.rvInt { return String(count) }; return rows[index].text(field) }, set: { text in
            var rows = settings["customs"]?.rvArray ?? []; guard rows.indices.contains(index) else { return }
            let next: JSONValue = field == "contextWindow" ? (text.isEmpty ? .null : Double(text).map(JSONValue.number) ?? .string(text)) : .string(text)
            rows[index] = portableField(rows[index], field, next); value = portableField(value, "model", portableField(settings, "customs", .array(rows)))
        })
    }
}
