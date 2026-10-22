import SwiftUI

private struct NativeTaskTemplateDraft: Equatable {
    var id: String
    var revision: JSONValue
    var name: String
    var title: String
    var body: String
    var config: JSONValue
}

struct RovaiTaskTemplateSettings: View {
    let model: RovaiWorkspaceModel
    @Environment(\.dismiss) private var dismiss
    @State private var identity = ""
    @State private var rows: [JSONValue] = []
    @State private var next: JSONValue = .null
    @State private var draft: NativeTaskTemplateDraft?, baseline: NativeTaskTemplateDraft?
    @State private var pending: JSONValue?, receipt: JSONValue?, issue: String?
    @State private var busy = false, ready = false, leaving = false, removing = false
    private var owner: String { "portable-task-templates/" + identity }
    private var dirty: Bool { draft != nil && draft != baseline }
    var body: some View {
        Form {
            Section { Text("保存模板不会启动任务。原程序设置和材料随模板保留，使用时在本次使命里核对。").foregroundStyle(Theme.textSecondary) }
            Section {
                Button("新建模板") { draft = NativeTaskTemplateDraft(id: UUID().uuidString.lowercased(), revision: .null, name: "", title: "", body: "", config: .object(["config_values": .object([:])])) ; baseline = nil }.disabled(!ready || pending != nil || dirty)
                Button("刷新模板") { work { try await list() } }.disabled(dirty)
                ForEach(rows, id: \.self) { row in Button(row.text("name")) { work { try await open(row.text("id")) } }.disabled(pending != nil || dirty) }
                if next != .null { Button("更多模板") { work { try await list(more: true) } } }
                if ready && rows.isEmpty { Text("还没有模板，可以新建或从原应用配置搬入。").foregroundStyle(Theme.textSecondary) }
            }
            if let draft {
                Section("编辑模板") {
                    TextField("模板名称", text: field(\.name))
                    TextField("任务标题", text: field(\.title))
                    TextField("任务说明", text: field(\.body), axis: .vertical).lineLimit(5...12)
                    Text("只修改上述内容，其余原高级设置和非文字材料完整保留。").font(.caption).foregroundStyle(Theme.textSecondary)
                    Button("保存模板") { work { try await mutate("put") } }.disabled(!dirty)
                    Button("放弃编辑") { self.draft = nil; baseline = nil }
                    if draft.revision != .null { Button("删除模板", role: .destructive) { removing = true } }
                }.disabled(pending != nil)
            }
            if pending != nil {
                Section("原模板操作") {
                    Text(receipt?.text("state") == "unknown" ? "尚未查到原结果，未自动重发。" : "原操作和文件结果分别保留。").font(.caption)
                    Button("核对原模板操作") { work { try await reconcile("receipt") } }
                    if receipt?.text("state") == "recorded", (receipt?["files"]?.rvArray ?? []).contains(where: { $0.text("state") == "pending" }) { Button("继续保存原模板文件") { work { try await reconcile("finish") } } }
                    if receipt?.text("state") == "recorded", (receipt?["files"]?.rvArray ?? []).contains(where: { ["conflict", "failed"].contains($0.text("state")) }) { Button("保留原结果并重新编辑") { work { try await remember(nil) } } }
                }
            }
            if let issue { Section { Text(issue).foregroundStyle(.red) } }; if busy { ProgressView() }
        }.disabled(busy).navigationTitle("任务模板").navigationBarBackButtonHidden(true).interactiveDismissDisabled(dirty || busy)
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("返回") { if dirty { leaving = true } else { dismiss() } }.disabled(busy) } }
        .confirmationDialog("放弃未保存的模板修改？", isPresented: $leaving) { Button("放弃并返回", role: .destructive) { dismiss() }; Button("继续编辑", role: .cancel) {} }
        .confirmationDialog("删除这份模板？已创建的任务、历史消息和原导入归档继续保留。", isPresented: $removing) { Button("确认删除模板", role: .destructive) { work { try await mutate("remove") } }; Button("取消", role: .cancel) {} }
        .task(id: model.connected) { await start() }
    }
    private func field(_ key: WritableKeyPath<NativeTaskTemplateDraft, String>) -> Binding<String> { Binding(get: { draft?[keyPath: key] ?? "" }, set: { value in if var current = draft { current[keyPath: key] = value; draft = current } }) }
    private func current() async throws { guard !identity.isEmpty, try await model.presentationIdentity() == identity else { throw CancellationError() } }
    private func call(_ input: JSONValue) async throws -> JSONValue { let generation = model.connectionGeneration, scope = identity; let value = try await model.privateConfiguration("templates", params: input); guard generation == model.connectionGeneration, identity == scope else { throw CancellationError() }; try await current(); return value }
    private func remember(_ value: JSONValue?) async throws { try model.saveBody(owner, body: value?.prettyString ?? ""); try await current(); pending = value }
    private func list(more: Bool = false) async throws {
        var input: [String: JSONValue] = ["operation": .string("list"), "limit": .number(100)]; if more { input["after"] = next }
        let value = try await call(.object(input)); let loaded = value["items"]?.rvArray ?? []
        rows = more ? rows + loaded.filter { item in !rows.contains { $0["id"] == item["id"] } } : loaded; next = value["next"] ?? .null
    }
    private func open(_ id: String) async throws {
        let value = try await call(.object(["operation": .string("get"), "id": .string(id)]))
        guard value != .null, let config = JSONValue.parse(value.text("config")), case .object = config else { throw RovaiHostFailure(code: "原模板需要先补齐配置，完整内容仍保留") }
        let result = NativeTaskTemplateDraft(id: value.text("id"), revision: value["revision"] ?? .null, name: value.text("name"), title: value.text("title"), body: config.text("display_text"), config: config)
        draft = result; baseline = result
    }
    private func settle(_ value: JSONValue) async throws {
        guard value["commandId"] == pending?["commandId"] else { throw RovaiHostFailure(code: "原模板回执编号不同") }; receipt = value
        guard value.text("state") == "recorded" else { return }
        if value["result"]?.text("status") == "rejected" { try await remember(nil); throw RovaiHostFailure(code: "模板已在其他位置变化，当前编辑仍保留，请重新读取") }
        let files = value["files"]?.rvArray ?? []
        if value["result"]?.text("status") == "applied", !files.isEmpty, files.allSatisfy({ $0.text("state") == "applied" }) { try await remember(nil); draft = nil; baseline = nil; try await list() }
    }
    private func reconcile(_ operation: String) async throws { guard let pending else { return }; try await settle(call(.object(["operation": .string(operation), "commandId": pending["commandId"] ?? .null]))) }
    private func mutate(_ operation: String) async throws {
        guard let draft, pending == nil, !draft.name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, draft.name.utf8.count <= 512, draft.title.utf8.count <= 4096 else { throw RovaiHostFailure(code: "请填写有效的名称和标题") }
        var written = false
        var blocks = (draft.config["prompt_blocks"]?.rvArray ?? []).compactMap { block -> JSONValue? in
            guard block.text("type") == "text" else { return block }; if written { return nil }; written = true; return .object(["type": .string("text"), "text": .string(draft.body)])
        }
        if !written { blocks.insert(.object(["type": .string("text"), "text": .string(draft.body)]), at: 0) }
        var config = draft.config.rvObject; config["display_text"] = .string(draft.body); config["prompt_blocks"] = .array(blocks)
        let content = JSONValue.object(config).prettyString; guard content.utf8.count <= 1024 * 1024 else { throw RovaiHostFailure(code: "模板超过大小限制，请缩短说明或拆分材料") }
        let command = UUID().uuidString.lowercased(); try await remember(.object(["commandId": .string(command), "targetId": .string(draft.id)]))
        var request: [String: JSONValue] = ["operation": .string(operation), "commandId": .string(command), "id": .string(draft.id), "expectedRevision": draft.revision]
        if operation == "put" { request["value"] = .object(["name": .string(draft.name), "title": .string(draft.title), "config": .string(content)]) }
        try await settle(call(.object(request)))
    }
    private func work(_ action: @escaping () async throws -> Void) { guard !busy else { return }; let generation = model.connectionGeneration; busy = true; issue = nil; Task { do { try await current(); try await action() } catch is CancellationError {} catch { if generation == model.connectionGeneration { issue = error.localizedDescription } }; if generation == model.connectionGeneration { busy = false } } }
    private func start() async {
        identity = ""; ready = false; draft = nil; baseline = nil; pending = nil; receipt = nil; rows = []; guard model.connected else { return }; busy = true
        do {
            identity = try await model.presentationIdentity(); let raw = try model.readBody(owner)
            if !raw.isEmpty { guard let value = JSONValue.parse(raw), case .object(let fields) = value, Set(fields.keys) == Set(["commandId", "targetId"]), UUID(uuidString: value.text("commandId")) != nil, UUID(uuidString: value.text("targetId")) != nil else { throw RovaiHostFailure(code: "原模板恢复记录无效，已保留") }; pending = value }
            try await list(); if pending != nil { try await reconcile("receipt") }; ready = true
        } catch is CancellationError {} catch { issue = error.localizedDescription }; busy = false
    }
}
