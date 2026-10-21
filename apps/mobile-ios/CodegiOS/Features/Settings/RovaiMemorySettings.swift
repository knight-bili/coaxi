import SwiftUI
import UniformTypeIdentifiers

private let nativeMemoryScopes = [("hearth", "共同记忆"), ("companion", "队员记忆"), ("relationship", "队员间记忆")]
private let nativeMemoryKinds = [("preference", "偏好"), ("agreement", "约定"), ("lesson", "经验")]
private func strings(_ value: JSONValue?, _ key: String) -> [String] { (value?[key]?.rvArray ?? []).compactMap(\.stringValue) }
struct RovaiMemorySettings: View {
    let model: RovaiWorkspaceModel
    @State private var library: JSONValue = .object([:])
    @State private var reviews: [JSONValue] = []
    @State private var selected: JSONValue?
    @State private var scope = "hearth"
    @State private var filter = "all"
    @State private var search = ""
    @State private var reviewMode = false
    @State private var busy = false
    @State private var issue: String?
    @State private var editor: Editor?
    @State private var action: Action?
    @State private var exporting = false
    @State private var exportDocument = RovaiMemoryExportDocument(data: Data())
    private struct Editor: Identifiable { let id = UUID(); let mode: String; let value: JSONValue? }
    private struct Action: Identifiable { let id = UUID(); let operation: RovaiOperation; let command: JSONValue; let title: String }
    private var blocked: Bool { model.pending != nil || model.busy }
    private var memories: [JSONValue] { library["memories"]?.rvArray ?? [] }
    private var visible: [JSONValue] { memories.filter { value in value.text("scope") == scope && (filter == "all" || filter == "agent" && value.text("creationOrigin") == "agent" || filter == "review" && value["reviewDue"]?.rvBool == true || filter == "stopped" && value.text("lifecycle") != "active") && (search.isEmpty || (value.text("currentBody") + strings(value, "currentRetrievalKeys").joined(separator: " ") + value.text("companionAgentId")).localizedCaseInsensitiveContains(search)) } }
    var body: some View {
        List {
            if let issue { Section { Text(issue).font(.caption).foregroundStyle(Theme.warning) } }
            Section { HStack { Button(reviewMode ? "返回记忆库" : "待审核 · \(reviews.filter { $0.text("status") == "pending" }.count)") { reviewMode.toggle(); selected = nil }; Spacer(); Button("导出") { Task { do { let value = try await model.query(.memoryExport); exportDocument = RovaiMemoryExportDocument(data: Data(value.prettyString.utf8)); exporting = true } catch { issue = error.localizedDescription } } }.disabled(busy) } }
            if reviewMode { ForEach(reviews, id: \.self) { value in GlassRow { VStack(alignment: .leading, spacing: 8) { Text(value.text("requestedAction") == "revise" ? "修订建议" : "新增建议").font(.headline); Text(value.text("status")).font(.caption).foregroundStyle(Theme.textSecondary); if value.text("status") == "pending" { MarkdownContent(raw: value.text("candidateBody")); Text("候选内容尚未成为共同记忆。").font(.caption); if value["stale"]?.rvBool == true { Text("目标版本已经变化，只能拒绝此建议。").font(.caption).foregroundStyle(Theme.textSecondary) }; HStack { if value["stale"]?.rvBool != true { Button("核对内容后接受") { editor = Editor(mode: "review", value: value) } }; Button("拒绝", role: .destructive) { action = Action(operation: .memoryHearthReviewItemsReject, command: .object(["reviewItemId": value["reviewItemId"] ?? .null, "expectedReviewItemVersion": value["version"] ?? .null]), title: "拒绝这条建议？") } }.disabled(blocked) } else { Text("候选正文已清除。").font(.caption).foregroundStyle(Theme.textSecondary) } } } } }
            else if let selected { detail(selected) }
            else {
                Section { Picker("记忆范围", selection: $scope) { ForEach(nativeMemoryScopes, id: \.0) { Text($0.1).tag($0.0) } }; Picker("筛选", selection: $filter) { Text("全部").tag("all"); Text("队员形成").tag("agent"); Text("待复核").tag("review"); Text("已停止沿用").tag("stopped") }; TextField("搜索正文、检索词或队员", text: $search) }
                Section("容量") { ForEach(library["capacities"]?.rvArray.filter { $0.text("scope") == scope } ?? [], id: \.self) { value in Text("沿用 \(value["activeCount"]?.rvInt ?? 0) / \(value["maxCount"]?.rvInt ?? 0) 条 · \(value["activeBodyBytes"]?.rvInt ?? 0) 字节").font(.caption).foregroundStyle(Theme.textSecondary) } }
                if visible.isEmpty { Text("当前筛选下没有记忆。").foregroundStyle(Theme.textSecondary) }
                ForEach(visible.map { RovaiRow(value: $0) }) { row in Button { selected = row.value } label: { GlassRow { VStack(alignment: .leading, spacing: 6) { Text(nativeMemoryKinds.first { $0.0 == row.value.text("kind") }?.1 ?? "记忆").font(.caption).foregroundStyle(Theme.textSecondary); Text(row.value.text("currentBody").isEmpty ? "正文已清除" : row.value.text("currentBody")).lineLimit(4).foregroundStyle(Theme.textPrimary) } } }.buttonStyle(.plain) }
            }
            if busy { ProgressView().frame(maxWidth: .infinity) }
        }.scrollContentBackground(.hidden).navigationTitle("记忆").toolbar { Button { editor = Editor(mode: "create", value: nil) } label: { Image(systemName: "plus") }.disabled(blocked); Button { Task { await load() } } label: { Image(systemName: "arrow.clockwise") }.disabled(busy) }
        .task(id: model.resourceVersion) { try? await Task.sleep(for: .milliseconds(250)); if !Task.isCancelled { await load() } }.onChange(of: model.lastResult) { _, _ in Task { await load() } }
        .sheet(item: $editor) { item in RovaiMemoryEditor(model: model, mode: item.mode, original: item.value) { editor = nil; Task { await load() } } }
        .confirmationDialog(action?.title ?? "确认操作", isPresented: Binding(get: { action != nil }, set: { if !$0 { action = nil } }), titleVisibility: .visible) { if let action { Button("确认", role: action.operation == .memoryForget || action.operation == .memoryHearthReviewItemsReject ? .destructive : nil) { Task { await model.command(action.operation, value: action.command); self.action = nil } }.disabled(blocked) }; Button("取消", role: .cancel) { action = nil } } message: { Text(action?.operation == .memoryForget ? "正文、历史版本和关联候选会按Core规则清除，无法通过重新沿用恢复。" : "按所选内容和当前版本执行。") }
        .fileExporter(isPresented: $exporting, document: exportDocument, contentType: .json, defaultFilename: "Rovai-memory") { result in if case .failure(let error) = result { issue = error.localizedDescription } }
    }
    @ViewBuilder private func detail(_ value: JSONValue) -> some View {
        Section { Button("返回记忆列表") { selected = nil }; Text(nativeMemoryScopes.first { $0.0 == value.text("scope") }?.1 ?? "记忆").font(.caption); MarkdownContent(raw: value.text("currentBody").isEmpty ? "正文已清除" : value.text("currentBody")); Text("检索词 · " + strings(value, "currentRetrievalKeys").joined(separator: "、")).font(.caption).foregroundStyle(Theme.textSecondary); Text("更新 · " + value.text("updatedAt")).font(.caption) }
        if value.text("lifecycle") != "forgotten" { Section { Button("修订") { editor = Editor(mode: "revise", value: value) }; Button("复核提醒") { editor = Editor(mode: "schedule", value: value) }; Button(value.text("lifecycle") == "active" ? "停止沿用" : "重新沿用") { action = Action(operation: value.text("lifecycle") == "active" ? .memoryRetire : .memoryReactivate, command: version(value), title: "修改这条记忆的沿用状态？") }; Button("彻底忘记", role: .destructive) { action = Action(operation: .memoryForget, command: version(value), title: "永久清除正文和版本内容？") } }.disabled(blocked) }
        Section("版本记录") { ForEach(value["revisions"]?.rvArray ?? [], id: \.self) { revision in GlassRow { VStack(alignment: .leading, spacing: 8) { Text(revision.text("createdAt")).font(.caption).foregroundStyle(Theme.textSecondary); MarkdownContent(raw: revision.text("body").isEmpty ? "正文已清除" : revision.text("body")) } } } }
    }
    private func version(_ value: JSONValue) -> JSONValue { .object(["memoryId": value["id"] ?? .null, "expectedVersion": value["version"] ?? .null]) }
    private func load() async { guard !busy else { return }; busy = true; defer { busy = false }; do { async let library = model.query(.memoryList); async let reviews = model.query(.memoryHearthReviewItemsList); let (value, candidates) = try await (library, reviews); self.library = value; self.reviews = candidates.rvArray; if let selected { self.selected = (value["memories"]?.rvArray ?? []).first { $0.text("id") == selected.text("id") } }; issue = nil } catch is CancellationError {} catch { issue = error.localizedDescription } }
}
private struct RovaiMemoryEditor: View {
    let model: RovaiWorkspaceModel
    let mode: String
    let original: JSONValue?
    let onSaved: () -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var authority: JSONValue?
    @State private var scope = "hearth"
    @State private var kind = "preference"
    @State private var text = ""
    @State private var keys = ""
    @State private var first = ""
    @State private var second = ""
    @State private var direction = "mutual"
    @State private var actor = ""
    @State private var reviewEnabled = false
    @State private var reviewDate = Date()
    @State private var issue: String?
    @State private var sent: String?
    @State private var leaving = false
    @State private var baseline = ""
    private var fingerprint: String { JSONValue.object(["scope": .string(scope), "kind": .string(kind), "text": .string(text), "keys": .string(keys), "first": .string(first), "second": .string(second), "direction": .string(direction), "actor": .string(actor), "reviewEnabled": .bool(reviewEnabled), "reviewDate": .number(reviewDate.timeIntervalSince1970)]).prettyString }
    private var blocked: Bool { model.pending != nil || model.busy }
    var body: some View {
        NavigationStack { Form {
            if let issue { Section { Text(issue).font(.caption).foregroundStyle(Theme.warning) } }
            if mode == "create" { Section("作用范围") { Picker("范围", selection: $scope) { ForEach(nativeMemoryScopes, id: \.0) { Text($0.1).tag($0.0) } }; Picker("类型", selection: $kind) { ForEach(nativeMemoryKinds, id: \.0) { Text($0.1).tag($0.0) } }; if scope != "hearth" { memberPicker("队员", value: $first) }; if scope == "relationship" { memberPicker("另一位队员", value: $second); Picker("方向", selection: $direction) { Text("相互约定").tag("mutual"); Text("定向约定").tag("directed") }; if direction == "directed" { Picker("履行队员", selection: $actor) { Text("请选择").tag(""); ForEach(model.members.filter { [first, second].contains($0.text("agentId")) }, id: \.self) { Text($0.text("displayName")).tag($0.text("agentId")) } } } } } }
            if mode != "schedule" { Section("完整正文") { TextEditor(text: $text).frame(minHeight: 160); TextField("检索词，每行一项", text: $keys, axis: .vertical).lineLimit(2...8) } }
            if mode != "review" { Section("复核提醒") { Toggle("设置下次复核", isOn: $reviewEnabled); if reviewEnabled { DatePicker("时间", selection: $reviewDate, displayedComponents: [.date, .hourAndMinute]) } } }
            if ["revise", "schedule"].contains(mode) { Section("最新版本") { Button("核对最新版本，保留当前编辑") { Task { do { authority = try await model.query(.memoryGet, params: .object(["memoryId": original?["id"] ?? .null])); issue = "已读取最新版本，请对照最新正文后再保存。" } catch { issue = error.localizedDescription } } }; if let authority { MarkdownContent(raw: authority.text("currentBody")) } } }
            if mode == "review" { Section { Text("接受后，最终内容会成为正式共同记忆。").font(.caption); if authority?["stale"]?.rvBool == true || authority?.text("status") != "pending" { Text("审核项已变化，请关闭后刷新。").foregroundStyle(Theme.warning) } } }
        }.disabled(blocked).navigationTitle(mode == "create" ? "新增记忆" : mode == "review" ? "审核共同记忆建议" : mode == "schedule" ? "复核提醒" : "修订记忆")
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("返回") { if fingerprint != baseline { leaving = true } else { dismiss() } } }; ToolbarItem(placement: .confirmationAction) { Button(mode == "review" ? "接受" : "保存") { Task { await save() } }.disabled(blocked || mode != "schedule" && text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || mode == "review" && (authority?["stale"]?.rvBool == true || authority?.text("status") != "pending")) } } }
        .task { authority = original; text = original?.text(mode == "review" ? "candidateBody" : "currentBody") ?? ""; keys = strings(original, mode == "review" ? "candidateRetrievalKeys" : "currentRetrievalKeys").joined(separator: "\n"); if let original { scope = original.text("scope"); kind = original.text("kind"); first = original.text("companionAgentId"); if let date = ISO8601DateFormatter().date(from: original.text("reviewAfter")) { reviewDate = date; reviewEnabled = true } }; baseline = fingerprint }
        .interactiveDismissDisabled(true)
        .confirmationDialog("放弃当前编辑？", isPresented: $leaving, titleVisibility: .visible) { Button("放弃编辑", role: .destructive) { dismiss() }; Button("继续编辑", role: .cancel) {} } message: { Text("尚未保存的内容会被丢弃，正式记忆不会改变。") }
        .task(id: model.resourceVersion) { guard mode == "review", let original else { return }; try? await Task.sleep(for: .milliseconds(250)); guard !Task.isCancelled else { return }; do { let values = try await model.query(.memoryHearthReviewItemsList); let latest = values.rvArray.first { $0.text("reviewItemId") == original.text("reviewItemId") }; authority = latest; if latest == nil || latest?.text("status") != "pending" { text = ""; keys = ""; issue = "审核项已经结束，候选正文已清除。" } else if latest?["stale"]?.rvBool == true { issue = "目标已经变化，请关闭后刷新，只能拒绝此建议。" } } catch { issue = error.localizedDescription } }
        .onChange(of: model.lastResult) { _, value in let receipt = value?["commandResult"] ?? value; if sent != nil, receipt?.text("commandId") == sent, receipt?.text("status") == "applied" { onSaved() } }
    }
    private func memberPicker(_ title: String, value: Binding<String>) -> some View { Picker(title, selection: value) { Text("请选择").tag(""); ForEach(model.members, id: \.self) { Text($0.text("displayName")).tag($0.text("agentId")) } } }
    private func save() async {
        var fields: [String: JSONValue] = [:]; let operation: RovaiOperation
        switch mode { case "create": operation = .memoryCreate; fields = ["scope": .string(scope), "kind": .string(kind), "companionAgentId": scope == "companion" ? .string(first) : .null, "relationshipAgentIds": .array(scope == "relationship" ? [first, second].map { .string($0) } : []), "direction": scope == "relationship" ? .string(direction) : .null, "directedActorAgentId": scope == "relationship" && direction == "directed" ? .string(actor) : .null]
        case "review": operation = .memoryHearthReviewItemsAccept; fields = ["reviewItemId": authority?["reviewItemId"] ?? .null, "expectedReviewItemVersion": authority?["version"] ?? .null]
        default: operation = mode == "schedule" ? .memoryReviewSchedule : .memoryRevise; fields = ["memoryId": authority?["id"] ?? .null, "expectedVersion": authority?["version"] ?? .null]; if mode == "revise" { fields["baseRevisionId"] = authority?["currentRevisionId"] ?? .null } }
        if mode != "schedule" { fields[mode == "review" ? "finalBody" : "body"] = .string(text.trimmingCharacters(in: .whitespacesAndNewlines)); fields[mode == "review" ? "finalRetrievalKeys" : "retrievalKeys"] = .array(keys.components(separatedBy: .newlines).map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }.map { .string($0) }) }
        if mode != "review" { fields["reviewAfter"] = reviewEnabled ? .string(ISO8601DateFormatter().string(from: reviewDate)) : .null }
        let id = UUID().uuidString.lowercased(); sent = id; await model.execute(operation, params: .object(["commandId": .string(id), "command": .object(fields)]))
    }
}
private struct RovaiMemoryExportDocument: FileDocument {
    static var readableContentTypes: [UTType] { [.json] }
    var data: Data
    init(data: Data) { self.data = data }
    init(configuration: ReadConfiguration) throws { data = configuration.file.regularFileContents ?? Data() }
    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper { FileWrapper(regularFileWithContents: data) }
}
