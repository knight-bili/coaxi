import SwiftUI

/// Codeg's native grouped list and reading view, backed by the shared Core catalog.
struct RovaiSkillSettingsView: View {
    let model: RovaiWorkspaceModel
    @State private var catalog: JSONValue = .object([:])
    @State private var installed: [JSONValue] = []
    @State private var choices: [JSONValue] = []
    @State private var search = ""
    @State private var family = "all"
    @State private var issue: String?
    @State private var loading = false
    private var disabled: Bool { model.pending != nil || model.busy || loading }
    private var entries: [JSONValue] { (catalog["entries"]?.rvArray ?? []).filter { entry in (family == "all" || entry.text("family") == family) && (search.isEmpty || "\(entry.text("id")) \(label(entry["displayName"])) \(label(entry["description"]))".localizedCaseInsensitiveContains(search)) } }
    var body: some View {
        ScrollView {
            LazyVStack(spacing: 12) {
                NavigationLink("查看原生 Skills") {RovaiNativeSkills(model: model)}
                if let issue { Text(issue).font(.caption).foregroundStyle(Theme.warning) }
                EditorSection(title: "技能类别") {
                    Picker("类别", selection: $family) { Text("全部").tag("all"); Text("专家").tag("experts"); Text("科研").tag("science"); Text("Office").tag("office"); Text("交互卡片").tag("ui"); Text("自定义").tag("custom") }
                }
                ForEach(entries.map { SkillRow(value: $0) }) { row in
                    NavigationLink {
                        RovaiSkillDetailView(model: model, entry: row.value, sourceRevision: catalog.text("commit"))
                    } label: {
                        GlassRow { VStack(alignment: .leading, spacing: 6) { Text(label(row.value["displayName"]).isEmpty ? row.id : label(row.value["displayName"])).font(.headline).foregroundStyle(Theme.textPrimary); Text(label(row.value["description"])).font(.caption).foregroundStyle(Theme.textSecondary).lineLimit(3); Text(installed.first { $0.text("skillId") == row.id }?.text("status") == "installed" ? "已安装" : "未安装").font(.caption2).foregroundStyle(Theme.textSecondary) } }
                    }.buttonStyle(.plain)
                }
            }.padding(.horizontal, Theme.Layout.screenHMargin).padding(.vertical, 12)
        }.navigationTitle("技能与专家库").searchable(text: $search, prompt: "搜索技能")
        .toolbar { Button("刷新") { Task { await load() } }.disabled(disabled) }
        .task { await load() }.onChange(of: model.lastResult) { _, _ in if model.lastOperation?.rawValue.hasPrefix("skillPacks.") == true { Task { await load() } } }
    }
    private func load() async {
        guard !loading else { return }; loading = true; defer { loading = false }
        do { catalog = try await model.query(.skillPacksList); installed = try await model.query(.skillPacksInstallationsList).rvArray; issue = nil } catch is CancellationError {} catch { issue = error.localizedDescription }
    }
}

private struct SkillRow: Identifiable { let value: JSONValue; var id: String { value.text("id").isEmpty ? value.text("path") : value.text("id") } }
private func label(_ value: JSONValue?) -> String { for key in ["zh-CN", "zh", "en"] { if let name = value?[key]?.stringValue, !name.isEmpty { return name } }; return "" }

private struct RovaiSkillDetailView: View {
    let model: RovaiWorkspaceModel
    let entry: JSONValue
    let sourceRevision: String
    @State private var installation: JSONValue?
    @State private var members: JSONValue?
    @State private var content: JSONValue = .object([:])
    @State private var path = "SKILL.md"
    @State private var issue: String?
    @State private var loading = false
    @State private var remove = false
    private var id: String { entry.text("id") }
    private var disabled: Bool { model.pending != nil || model.busy || loading }
    var body: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 18) {
                if let issue { Text(issue).font(.caption).foregroundStyle(Theme.warning) }
                EditorSection(title: label(entry["displayName"])) {
                    Text(label(entry["description"])).font(.subheadline).foregroundStyle(Theme.textSecondary)
                    Text("来源：\(entry.text("sourceUrl"))").font(.caption).textSelection(.enabled)
                    Text("版本：\(sourceRevision.prefix(12))").font(.caption)
                    if entry["needsKey"]?.rvBool == true { Text("需要外部服务凭据。") }
                    if entry["needsEnv"]?.rvBool == true { Text("可能需要额外环境；安装不会自动安装依赖。") }
                    HStack {
                        if ["embedded", "custom"].contains(entry.text("contentSource")) { Button(installation?.text("status") == "installed" ? "更新安装" : "安装") { Task { await install() } }.disabled(disabled) }
                        if installation?.text("status") == "installed" { Button("卸载", role: .destructive) { remove = true }.disabled(disabled) }
                    }
                }
                if members?["installed"]?.rvBool == true {
                    EditorSection(title: "谁可以使用") {
                        Text("用于队员的新任务；取消选择不改写旧任务。").font(.caption).foregroundStyle(Theme.textSecondary)
                        ForEach(model.members.filter { $0.text("presence") != "removed" }.map { RovaiRow(value: $0) }) { row in
                            Toggle(row.value.text("displayName"), isOn: Binding(get: { selected.contains(row.id) }, set: { checked in Task { await saveMembers(checked ? selected.union([row.id]) : selected.subtracting([row.id])) } })).disabled(disabled)
                        }
                    }
                }
                EditorSection(title: "附带文件") { ForEach((content["files"]?.rvArray ?? []).map { SkillRow(value: $0) }) { row in Button(row.id) { path = row.id } } }
                if content.text("status") == "text" { MarkdownContent(raw: content.text("content")) } else if !content.text("status").isEmpty { Text(content.text("status") == "binary" ? "该文件不是文本文件。" : "文件较大，暂不支持正文预览。") }
            }.padding(.horizontal, Theme.Layout.screenHMargin).padding(.vertical, 12)
        }.navigationTitle(label(entry["displayName"]).isEmpty ? id : label(entry["displayName"]))
        .toolbar { Button("刷新") { Task { await load(); await read() } }.disabled(disabled) }
        .task { await load() }.task(id: path) { await read() }
        .onChange(of: model.lastResult) { _, _ in if model.lastOperation?.rawValue.hasPrefix("skillPacks.") == true { Task { await load() } } }
        .confirmationDialog("卸载技能包", isPresented: $remove, titleVisibility: .visible) { Button("确认卸载", role: .destructive) { Task { await model.command(.skillPacksUninstall, value: .object(["skillId": .string(id), "expectedVersion": installation?["version"] ?? .string("absent")])) } } } message: { Text("保留文件和历史版本；重新安装不会自动恢复队员选择。") }
    }
    private var selected: Set<String> { Set((members?["memberIds"]?.rvArray ?? []).compactMap(\.stringValue)) }
    private func load() async {
        guard !loading else { return }; loading = true; defer { loading = false }
        do { installation = try await model.query(.skillPacksInstallationsList).rvArray.first { $0.text("skillId") == id }; members = try await model.query(.skillPacksMembersList).rvArray.first { $0.text("skillId") == id }; issue = nil } catch is CancellationError {} catch { issue = error.localizedDescription }
    }
    private func read() async {
        do { content = try await model.query(.skillPacksRead, params: .object(["skillId": .string(id), "path": .string(path)])) } catch is CancellationError {} catch { issue = error.localizedDescription }
    }
    private func install() async {
        let custom = entry.text("contentSource") == "custom"
        await model.command(custom ? .skillPacksCustomInstall : .skillPacksInstall, value: .object(["skillId": .string(id), "expectedVersion": installation?["version"] ?? .string("absent"), "sourceRevision": .string(custom ? entry.text("loadId") : sourceRevision)]))
    }
    private func saveMembers(_ ids: Set<String>) async {
        guard let members, !disabled else { return }
        await model.command(.skillPacksMembersSet, value: .object(["skillId": .string(id), "expectedVersion": members["version"] ?? .null, "memberIds": .array(ids.sorted().map(JSONValue.string))]))
    }
}
