import SwiftUI

/// 复用Codeg原生模板编辑器，Core保存全局和工程内容。
struct RovaiMessageLibrary: View {
    let model: RovaiWorkspaceModel
    var threadId: String? = nil
    var onUse: ((String) -> Bool)? = nil
    @State private var level = "global"
    @State private var value: JSONValue?
    @State private var digest = ""
    @State private var busy = false
    @State private var issue: String?
    @State private var editor: Editor?
    @State private var tag = ""
    @State private var aliasEditor: AliasEditor?
    private struct AliasEditor: Identifiable {let id = UUID();let index: Int?;let value: JSONValue?}
    private var aliases: [JSONValue] {value?["directoryAliases"]?.rvArray ?? []}
    private struct Editor: Identifiable { let id = UUID(); let value: JSONValue? }
    private var messages: [JSONValue] { value?["quickMessages"]?.rvArray ?? [] }
    private var tags: [String] { (value?["tags"]?.rvArray ?? []).compactMap(\.stringValue) }
    private func args(_ next: JSONValue? = nil) -> JSONValue {
        var result: [String: JSONValue] = ["level": .string(level)]
        if let threadId { result["scope"] = .object(["threadId": .string(threadId)]) }
        if let next { result["expectedDigest"] = .string(digest); result["value"] = next }
        return .object(result)
    }
    var body: some View {
        List {
            if let issue { Section { Text(issue).font(.caption).foregroundStyle(Theme.warning) } }
            if threadId != nil { Section { Picker("适用范围", selection: $level) { Text("全局").tag("global"); Text("当前工程").tag("project"); Text("当前对话").tag("thread") }.pickerStyle(.segmented).disabled(busy || editor != nil || aliasEditor != nil) } }
            Section { ForEach(messages.map { RovaiRow(value: $0) }) { row in
                GlassRow { VStack(alignment: .leading, spacing: 8) {
                    Text(row.value.text("title")).font(.headline); Text(row.value.text("text")).foregroundStyle(Theme.textSecondary).lineLimit(3)
                    HStack { if let onUse { Button("填入") { if !onUse(row.value.text("text")) { issue = "当前输入暂时不能插入，请先核对原操作。" } } }; Button("编辑") { editor = Editor(value: row.value) }; Spacer(); Button(role: .destructive) { updateMessages(messages.filter { $0.text("id") != row.id }) } label: { Image(systemName: "trash") } }.disabled(busy)
                } }.listRowBackground(Color.clear).listRowSeparator(.hidden)
            }.onMove { source, destination in var next = messages; next.move(fromOffsets: source, toOffset: destination); updateMessages(next) }
            Button { editor = Editor(value: nil) } label: { Label("新建快捷消息", systemImage: "plus") }.disabled(value == nil || busy)
            } header: { Text("快捷消息") } footer: { Text("选择后只填入输入框，由你发送。") }
            Section {
                ForEach(Array(aliases.enumerated()), id: \.offset) { index, row in GlassRow { VStack(alignment: .leading, spacing: 8) {Text(row.text("name").isEmpty ? row.text("path") : row.text("name")).font(.headline);Text(row.text("path")).font(.caption).foregroundStyle(Theme.textSecondary);if !row.text("group").isEmpty {Text(row.text("group")).font(.caption).foregroundStyle(Theme.textSecondary)};HStack {Button("编辑") {aliasEditor = AliasEditor(index: index, value: row)};Spacer();Button(role: .destructive) {updateAliases(aliases.enumerated().filter {$0.offset != index}.map(\.element))} label: {Image(systemName: "trash")}}.disabled(busy)} }.listRowBackground(Color.clear).listRowSeparator(.hidden)}.onMove {source, destination in var next = aliases;next.move(fromOffsets: source, toOffset: destination);updateAliases(next)}
                Button {aliasEditor = AliasEditor(index: nil, value: nil)} label: {Label("添加关联目录", systemImage: "folder.badge.plus")}.disabled(busy || value == nil)
            } header: {Text("关联目录、别名与分组")} footer: {Text("填写主机上的目录。别名只整理显示，不改变文件访问权限。")}
            Section("标签") {
                ForEach(tags, id: \.self) { label in HStack { Text(label); Spacer(); Button { updateTags(tags.filter { $0 != label }) } label: { Image(systemName: "xmark.circle") }.disabled(busy) } }
                HStack { TextField("新标签", text: $tag); Button("添加") { let next = tags + [tag.trimmingCharacters(in: .whitespacesAndNewlines)]; Task { do { var changed = value?.rvObject ?? [:]; changed["tags"] = .array(next.map(JSONValue.string)); try await save(.object(changed)); tag = "" } catch { issue = error.localizedDescription } } }.disabled(value == nil || busy || tag.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || tags.contains(tag.trimmingCharacters(in: .whitespacesAndNewlines))) }
            }
        }.scrollContentBackground(.hidden).navigationTitle("快捷消息和标签")
        .toolbar { EditButton().disabled(busy) }
        .task(id: "\(level):\(model.connectionGeneration)") { await load() }
        .sheet(item: $aliasEditor) {target in RovaiDirectoryAliasSheet(value: target.value) {path, name, group in
            let row = JSONValue.object(["path": .string(path), "name": .string(name), "group": .string(group)])
            let next = target.index == nil ? aliases + [row] : aliases.enumerated().map {$0.offset == target.index ? row : $0.element}
            var changed = value?.rvObject ?? [:];changed["directoryAliases"] = .array(next);try await save(.object(changed))
        }}
        .sheet(item: $editor) { target in
            QuickMessageEditorSheet(editing: target.value.map { QuickMessage(id: 0, title: $0.text("title"), content: $0.text("text"), sortOrder: 0) }) { title, content in
                let item = JSONValue.object(["id": .string(target.value?.text("id") ?? UUID().uuidString.lowercased()), "title": .string(title), "text": .string(content)])
                let next = target.value == nil ? messages + [item] : messages.map { $0.text("id") == target.value?.text("id") ? item : $0 }
                var changed = value?.rvObject ?? [:]; changed["quickMessages"] = .array(next); try await save(.object(changed))
            }
        }
    }
    private func updateAliases(_ rows: [JSONValue]) {var changed = value?.rvObject ?? [:];changed["directoryAliases"] = .array(rows);Task {do {try await save(.object(changed))} catch {issue = error.localizedDescription}}}
    private func load() async {
        guard !busy else { return }; busy = true; defer { busy = false }; let generation = model.connectionGeneration; value = nil
        do { let result = try await model.query(.workbenchOrganizerGet, params: args()); guard generation == model.connectionGeneration, !Task.isCancelled else { return }; value = result["value"]; digest = result.text("digest"); issue = nil } catch is CancellationError {} catch { issue = error.localizedDescription }
    }
    private func save(_ next: JSONValue) async throws {
        guard !busy, !digest.isEmpty, model.pending == nil else { throw RovaiHostFailure(code: "请先完成当前操作") }; let request = args(next); let generation = model.connectionGeneration; busy = true; defer { busy = false }
        let result = try await model.query(.workbenchOrganizerSet, params: request); guard generation == model.connectionGeneration else { throw RovaiHostFailure(code: "工作区已切换，原草稿仍保留") }; value = result["value"]; digest = result.text("digest"); issue = nil
    }
    private func updateMessages(_ rows: [JSONValue]) { var changed = value?.rvObject ?? [:]; changed["quickMessages"] = .array(rows); Task { do { try await save(.object(changed)) } catch { issue = error.localizedDescription } } }
    private func updateTags(_ labels: [String]) { var changed = value?.rvObject ?? [:]; changed["tags"] = .array(labels.map(JSONValue.string)); Task { do { try await save(.object(changed)) } catch { issue = error.localizedDescription } } }
}

private struct RovaiDirectoryAliasSheet: View {
    let value: JSONValue?;let save: (String, String, String) async throws -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var path = "";@State private var name = "";@State private var group = "";@State private var busy = false;@State private var issue: String?;@State private var discard = false;@State private var loaded = false
    private var dirty: Bool {loaded && (path != (value?.text("path") ?? "") || name != (value?.text("name") ?? "") || group != (value?.text("group") ?? ""))}
    var body: some View {NavigationStack {Form {if let issue {Text(issue).foregroundStyle(Theme.warning)};TextField("主机目录", text: $path).textInputAutocapitalization(.never).autocorrectionDisabled();TextField("别名", text: $name);TextField("分组（可选）", text: $group)}.disabled(busy).navigationTitle(value == nil ? "添加关联目录" : "编辑关联目录").toolbar {ToolbarItem(placement: .cancellationAction) {Button("取消") {if dirty {discard = true} else {dismiss()}}.disabled(busy)};ToolbarItem(placement: .confirmationAction) {Button("保存") {Task {busy = true;defer {busy = false};do {try await save(path.trimmingCharacters(in: .whitespacesAndNewlines), name.trimmingCharacters(in: .whitespacesAndNewlines), group.trimmingCharacters(in: .whitespacesAndNewlines));dismiss()} catch {issue = error.localizedDescription}}}.disabled(busy || path.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)}}}.interactiveDismissDisabled(busy || dirty).task {if !loaded {path = value?.text("path") ?? "";name = value?.text("name") ?? "";group = value?.text("group") ?? "";loaded = true}}.confirmationDialog("放弃未保存的目录信息？", isPresented: $discard, titleVisibility: .visible) {Button("放弃", role: .destructive) {dismiss()};Button("继续编辑", role: .cancel) {}}}
}
