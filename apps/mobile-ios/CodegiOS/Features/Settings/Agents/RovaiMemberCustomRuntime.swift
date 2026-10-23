import SwiftUI

/// 沿原模型设置的默认/指定切换；保存具体定义引用，原请求只读核对。
@MainActor struct RovaiMemberCustomRuntime: View {
    let model: RovaiWorkspaceModel, member: JSONValue
    @State private var rows: [JSONValue] = []
    @State private var next: String?
    @State private var selected = ""
    @State private var modelId = ""
    @State private var options = "{}"
    @State private var explicit = false
    @State private var busy = false
    @State private var issue: String?
    @State private var generation: UInt64?
    @State private var original: JSONValue?
    private var id: String {member.text("agentId")}
    private var owner: String {"custom-member-runtime:" + id}
    private var saved: JSONValue {member["runtimeConfiguration"]?["customRuntimeRef"] ?? .object([:])}
    private var current: JSONValue? {rows.first {$0.text("id") == selected}}
    private var disabled: Bool {busy || model.busy || model.pending != nil || generation != model.connectionGeneration}
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("从已添加的具体版本中选择。先在智能体设置中安装此版本。").font(.caption).foregroundStyle(Theme.textSecondary)
            Picker("程序", selection: $selected) {
                Text("选择自定义智能体").tag("")
                ForEach(rows, id: \.self) {definition in Text(definition.text("name") + " · " + definition.text("version") + " · r" + String(definition["revision"]?.rvInt ?? 0) + (definition["enabled"]?.rvBool == false ? " · 已停用" : "")).tag(definition.text("id"))}
                if !selected.isEmpty && !rows.contains(where: {$0.text("id") == selected}) {Text(selected + " · 原保存版本").tag(selected)}
            }.disabled(disabled || original != nil)
            if next != nil {Button("更多程序") {work {try await load(more: true)}}.disabled(disabled)}
            if !saved.text("id").isEmpty {Text("当前保存：" + saved.text("id") + " · r" + String(saved["definitionRevision"]?.rvInt ?? 0)).font(.caption)}
            if current != nil {
                Toggle("指定模型", isOn: $explicit).disabled(disabled || original != nil)
                if explicit {TextField("原程序模型标识", text: $modelId).textInputAutocapitalization(.never).autocorrectionDisabled().disabled(disabled || original != nil);TextEditor(text: $options).font(.body.monospaced()).frame(minHeight: 100).accessibilityLabel("模型参数 JSON").disabled(disabled || original != nil)}
                Text("保存只影响之后的任务。模型和参数由真实智能体会话核对，失败会保留原因，不自动换模型。").font(.caption).foregroundStyle(Theme.textSecondary)
            }
            if let issue {Text(issue).foregroundStyle(Theme.warning)}
            if let original {Text(original.text("commandId")).font(.caption).textSelection(.enabled);Button("核对原配置请求") {work {try await apply(reconcile: true)}}.disabled(disabled)}
            else {Button("使用所选程序") {work {try await apply()}}.disabled(disabled || current?["enabled"]?.rvBool != true)}
        }
        .task(id: id) {
            generation = model.connectionGeneration;selected = saved.text("id");let selection = member["runtimeConfiguration"]?["model"];explicit = selection?.text("mode") == "explicit";modelId = selection?.text("modelId") ?? "";options = selection?["options"]?.prettyString ?? "{}"
            busy = true;defer {busy = false};do {let raw = try model.readBody(owner);if !raw.isEmpty {guard let pending = JSONValue.parse(raw), pending["command"]?.text("agentId") == id else {throw RovaiHostFailure(code: "原请求队员归属不同")};original = pending};try await load()} catch {issue = error.localizedDescription}
        }
    }
    private func work(_ block: @escaping () async throws -> Void) {guard !disabled else {return};busy = true;Task {defer {busy = false};do {try await block();issue = nil} catch is CancellationError {} catch {if generation == model.connectionGeneration {issue = error.localizedDescription}}}}
    private func load(more: Bool = false) async throws {
        let result = try await model.query(.runtimeCustomList, params: .object(["after": more ? next.map(JSONValue.string) ?? .null : .null, "limit": .number(100)]));guard generation == model.connectionGeneration else {throw CancellationError()}
        let incoming = result["items"]?.rvArray ?? [];rows = more ? rows + incoming.filter {item in !rows.contains {$0.text("id") == item.text("id")}} : incoming;next = result["next"]?.stringValue
    }
    private func apply(reconcile: Bool = false) async throws {
        guard generation == model.connectionGeneration else {throw CancellationError()}
        let request: JSONValue
        if let original {request = original}
        else {
            guard let current, current["enabled"]?.rvBool == true else {throw RovaiHostFailure(code: "请选择已启用的自定义智能体")}
            let selection: JSONValue
            if explicit {
                guard !modelId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, options.utf8.count <= 65536, let parsed = JSONValue.parse(options), case .object = parsed else {throw RovaiHostFailure(code: "请填写模型标识，并使用64KB内的参数 JSON 对象")}
                selection = .object(["mode": .string("explicit"), "modelId": .string(modelId), "options": parsed])
            } else {selection = .object(["mode": .string("runtime_default")])}
            request = .object(["commandId": .string(UUID().uuidString.lowercased()), "command": .object(["agentId": .string(id), "expectedVersion": member["version"] ?? .number(0), "adapterKind": .string("custom-acp"), "model": selection, "permissions": .object(["adapterKind": .string("custom-acp"), "schemaVersion": .number(1), "values": .object([:])]), "customRuntimeRef": customRuntimeRef(current)])])
        }
        try model.saveBody(owner, body: request.prettyString);original = request
        let receipt: JSONValue
        if reconcile {let result = try await model.query(.commandsReconcile, params: .object(["operation": .string(RovaiOperation.membersRuntimeSet.rawValue), "params": request]));guard result.text("state") == "recorded", let recorded = result["result"] else {throw RovaiHostFailure(code: "原配置尚无确定回执，没有重复保存")};receipt = recorded}
        else {receipt = try await model.query(.membersRuntimeSet, params: request)}
        guard let generation, generation == model.connectionGeneration else {throw CancellationError()};guard ["applied", "rejected"].contains(receipt.text("status")) else {throw RovaiHostFailure(code: "原配置结果尚未确定")}
        _ = try model.clearBodyIfMatches(owner, expectedBody: request.prettyString, expectedGeneration: generation);original = nil
        guard receipt.text("status") == "applied" else {throw RovaiHostFailure(code: receipt.text("code"))};await model.refresh();try await load()
    }
}
