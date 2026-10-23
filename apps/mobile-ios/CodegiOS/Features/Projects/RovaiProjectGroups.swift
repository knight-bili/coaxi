import SwiftUI

/// Original ProjectGroupsDialog, with shared Host identities and retained unknown CAS requests.
struct RovaiProjectGroups: View {
    let model: RovaiWorkspaceModel
    @Environment(\.dismiss) private var dismiss
    @State private var source: JSONValue?
    @State private var editor: JSONValue?
    @State private var pending: JSONValue?
    @State private var removal: JSONValue?
    @State private var folders: [String:RovaiCanvasFolderRelation] = [:]
    @State private var generation: UInt64?
    @State private var issue: String?
    @State private var busy = false
    @State private var ready = false
    @State private var useCurrent = false
    @State private var arranging = false
    @State private var replaceLayout = false
    @State private var layout: RovaiProjectLayout?
    @State private var initialLayout: RovaiProjectLayout?
    @State private var layoutRevision = 0
    private let layoutOwner = "project-groups:layout"
    private let draftOwner = "project-groups:editor",pendingOwner = "project-groups:pending"
    private var active: [String] {var seen = Set<String>();return (model.navigation["projects"]?.rvArray ?? []).map {$0.text("projectPath")}.filter {rovaiProjectPath($0) && seen.insert($0).inserted}}
    private var groups: [JSONValue] {source?["projectGroups"]?.rvArray ?? []}
    private var revision: Int {source?["projectGroupsRevision"]?.rvInt ?? 0}
    private var group: JSONValue {editor?["group"] ?? .null}
    private var selected: [String] {(group["projectPaths"]?.rvArray ?? []).compactMap(\.stringValue)}
    private var previous: JSONValue? {groups.first {$0.text("id") == group.text("id")}}
    private var previousPaths: [String] {(previous?["projectPaths"]?.rvArray ?? []).compactMap(\.stringValue)}
    private var stale: Bool {arranging ? layoutRevision != revision : editor != nil && editor?["revision"]?.rvInt != revision}
    private var missing: Bool {editor?.text("mode") == "update" && previous == nil}
    private var membershipIssue: Bool {selected.contains {!previousPaths.contains($0) && (folders[$0]?.complete != true || folders[$0]?.parentPath != nil)}}
    private var blocked: Bool {busy || !ready || source == nil || pending != nil || model.busy || model.pending != nil || generation != model.connectionGeneration}
    private var paths: [String] {var seen = Set<String>();return (active+selected).filter {seen.insert($0).inserted}}
    var body: some View {
        NavigationStack {Form {
            Section {
                Text("分组由当前工作区共享，文件和对话保持原处。").font(.caption).foregroundStyle(Theme.textSecondary)
                if busy {ProgressView()}
                if let issue {Text(issue).foregroundStyle(Theme.warning)}
                Button("重新读取分组") {Task {await refresh()}}.disabled(busy)
            }
            if let pending {Section("原分组修改待核对") {
                Text("保留完整原请求，不会自动重发。").font(.caption)
                Button("原样重试原修改") {Task {await retry(pending)}}.disabled(busy)
                if source != nil,revision>(pending["params"]?["expectedRevision"]?.rvInt ?? Int.max) {Button("使用当前版本并放弃旧本机计划") {useCurrent = true}.disabled(busy)}
            }}
            if arranging {layoutSections}
            else if editor == nil {
                Section {Button("排列项目与分组") {beginLayout()}.disabled(blocked || active.contains {folders[$0]?.complete != true});Button("新建项目分组") {edit(nil)}.disabled(blocked);if groups.isEmpty {Text("还没有项目分组。").foregroundStyle(Theme.textSecondary)}}
                ForEach(groups.map(RovaiRow.init(value:))) {row in Section {
                    HStack {Circle().fill(rovaiCanvasTint(row.value.text("color")) ?? Theme.textSecondary).frame(width: 10,height: 10);Text(row.value.text("name")).font(.headline);Spacer();Text("\(row.value["projectPaths"]?.rvArray.count ?? 0) 个工程").font(.caption).foregroundStyle(Theme.textSecondary)}
                    Button("编辑") {edit(row.value)}.disabled(blocked)
                    Button("删除",role: .destructive) {removal = .object(["group": row.value,"revision": .number(Double(revision))])}.disabled(blocked)
                }}
            } else {editorSections}
        }.navigationTitle("管理项目分组").toolbar {Button("保留草稿并关闭") {close()}.disabled(busy)}}.interactiveDismissDisabled(true)
        .task {await restore()}
        .confirmationDialog("放弃本机排版，载入最新布局？",isPresented: $replaceLayout,titleVisibility: .visible) {
            Button("载入最新布局",role: .destructive) {if let source {let next = createProjectLayout(source,active: active,folders: folders);initialLayout = next;layout = next;layoutRevision = revision;persistLayout()}}.disabled(blocked)
            Button("保留当前排版",role: .cancel) {}
        } message: {Text("当前未保存的位置和分组调整将被替换；不会向工作区提交更改。")}
        .confirmationDialog("删除项目分组？",isPresented: Binding(get: {removal != nil},set: {if !$0 {removal = nil}}),titleVisibility: .visible) {
            Button("确认删除分组",role: .destructive) {if let removal {let mutation: JSONValue = .object(["type": .string("delete"),"groupId": removal["group"]?["id"] ?? .null]);Task {await apply(mutation)}};removal = nil}.disabled(blocked || removal?["revision"]?.rvInt != revision)
            Button("取消",role: .cancel) {removal = nil}
        } message: {Text("工程将回到顶层，文件、使命和对话都会保留。")}
        .confirmationDialog("使用当前分组版本？",isPresented: $useCurrent,titleVisibility: .visible) {
            Button("使用当前版本") {Task {await abandonSuperseded()}}.disabled(busy)
            Button("保留原计划",role: .cancel) {}
        } message: {Text("当前版本已超过原修改的版本，原旧请求不能再写入。只放弃这次本机未决计划；未保存的分组草稿仍保留。")}
    }
    @ViewBuilder private var layoutSections: some View {
        Section("排列项目与分组") {
            if stale {Text("项目排列已有新修改，当前排版草稿保留。").foregroundStyle(Theme.warning)}
            if let initialLayout,let layout,projectLayoutRelationIssue(initialLayout,next: layout,folders: folders) {Text("工程关系已有变化或尚未核对，请重新读取后使用最新布局。").foregroundStyle(Theme.warning)}
            Button("放弃本机排版并载入最新布局") {replaceLayout = true}.disabled(blocked)
        }
        if let layout {ForEach(layout.top,id: \.key) {entry in Section {
            RovaiProjectLayoutRow(entry: entry,layout: layout,groups: groups,folders: folders,blocked: blocked || stale,onMove: moveLayout)
            if entry.kind == "group" {ForEach(layout.members[entry.target] ?? [],id: \.self) {path in RovaiProjectLayoutRow(entry: .init(kind: "project",target: path),layout: layout,groups: groups,folders: folders,blocked: blocked || stale,onMove: moveLayout)}}
        }}}
        Section {
            Button("保存排列") {if let layout {Task {await apply(.object(["type": .string("layout"),"entries": .array(layout.entries())]))}}}.disabled(blocked || stale || layout == nil || initialLayout == nil || layout == initialLayout || projectLayoutRelationIssue(initialLayout ?? .init(top: [],members: [:]),next: layout ?? .init(top: [],members: [:]),folders: folders))
            Button("保留排版，返回清单") {persistLayout();arranging = false}.disabled(busy)
        }
    }
    private func layoutBody() throws -> String {guard let layout,let initialLayout else {throw RovaiHostFailure(code: "项目排版草稿不完整")};return try encode(.object(["version": .number(1),"revision": .number(Double(layoutRevision)),"base": initialLayout.value(),"layout": layout.value()]))}
    private func persistLayout() {guard ready,generation == model.connectionGeneration,layout != nil else {return};do {try model.saveBody(layoutOwner,body: layoutBody())} catch {issue = error.localizedDescription}}
    private func beginLayout() {guard let source else {return};if layout == nil {let next = createProjectLayout(source,active: active,folders: folders);initialLayout = next;layout = next;layoutRevision = revision;persistLayout()};arranging = true}
    private func moveLayout(_ entry: RovaiProjectLayoutItem,_ target: RovaiProjectLayoutLocation) {guard let initialLayout,let layout else {return};let next = layout.move(entry,to: target);guard !projectLayoutRelationIssue(initialLayout,next: next,folders: folders) else {issue = "工程关系已有变化或尚未核对，请重新读取后使用最新布局。";return};self.layout = next;persistLayout()}
    @ViewBuilder private var editorSections: some View {
        Section("分组") {
            if stale {Text("分组已有新修改，输入已保留。请核对最新清单后继续。").foregroundStyle(Theme.warning);ForEach(groups.map(RovaiRow.init(value:))) {row in Text(row.value.text("name")+" · \(row.value["projectPaths"]?.rvArray.count ?? 0) 个工程").font(.caption)};Button("已核对最新分组") {editorChange(["revision": .number(Double(revision))])}.disabled(blocked || missing)}
            if missing {Text("原分组已移除，不能用旧输入重新创建。").foregroundStyle(Theme.warning)}
            TextField("分组名称",text: Binding(get: {group.text("name")},set: {groupChange(["name": .string($0)])}))
            Picker("分组颜色",selection: Binding(get: {group.text("color")},set: {groupChange(["color": $0.isEmpty ? .null : .string($0)])})) {ForEach(rovaiCanvasColors,id: \.0) {key,label in HStack {Circle().fill(rovaiCanvasTint(key) ?? Theme.textSecondary).frame(width: 10,height: 10);Text(label)}.tag(key)}}
            if membershipIssue {Text("所选新工程的工作树关系尚未核对，或此工作树跟随主工程；暂不能新增关联。").font(.caption).foregroundStyle(Theme.warning)}
        }.disabled(blocked)
        Section("选择工程") {ForEach(paths,id: \.self) {path in
            let checked = selected.contains(path),parent = folders[path]?.parentPath,owner = groups.first {$0.text("id") != group.text("id") && ($0["projectPaths"]?.rvArray ?? []).contains(.string(path))}
            VStack(alignment: .leading,spacing: 5) {
                Toggle(path,isOn: Binding(get: {checked},set: {value in groupChange(["projectPaths": .array((value ? selected+[path] : selected.filter {$0 != path}).map(JSONValue.string))])})).disabled(blocked || !checked && (!active.contains(path) || parent != nil))
                if let owner,parent == nil {Text("当前属于“\(owner.text("name"))”，保存后移入本组。").font(.caption).foregroundStyle(Theme.textSecondary)}
                if !active.contains(path) {Text("工程已移除，原关联保留；取消勾选会明确移出。").font(.caption).foregroundStyle(Theme.textSecondary)}
                if parent != nil {Text("此工作树跟随主工程归组。").font(.caption).foregroundStyle(Theme.textSecondary)}
            }
        }}
        Section {
            let name = group.text("name").trimmingCharacters(in: .whitespacesAndNewlines)
            Button("保存分组") {if let editor {Task {await apply(.object(["type": editor["mode"] ?? .null,"group": group]))}}}.disabled(blocked || stale || missing || membershipIssue || name.isEmpty || name.unicodeScalars.count>80)
            Button("放弃草稿，返回清单",role: .destructive) {discardDraft()}.disabled(blocked)
        }
    }
    private func encode(_ value: JSONValue) throws -> String {let encoder = JSONEncoder();encoder.outputFormatting = [.sortedKeys];return String(decoding: try encoder.encode(value),as: UTF8.self)}
    private func editorBody(_ value: JSONValue) throws -> String {try encode(.object(["version": .number(1),"editor": value]))}
    private func editorChange(_ patch: [String:JSONValue]) {guard let editor else {return};self.editor = .object(editor.rvObject.merging(patch) {_,next in next});persist()}
    private func groupChange(_ patch: [String:JSONValue]) {editorChange(["group": .object(group.rvObject.merging(patch) {_,next in next})])}
    private func persist() {guard ready,let editor,generation == model.connectionGeneration else {return};do {try model.saveBody(draftOwner,body: editorBody(editor))} catch {issue = error.localizedDescription}}
    private func close() {guard !busy else {return};if arranging,ready,generation == model.connectionGeneration {do {try model.saveBody(layoutOwner,body: layoutBody());dismiss()} catch {issue = error.localizedDescription};return};guard ready,let editor,generation == model.connectionGeneration else {dismiss();return};do {try model.saveBody(draftOwner,body: editorBody(editor));dismiss()} catch {issue = error.localizedDescription}}
    private func edit(_ row: JSONValue?) {
        guard !blocked else {return}
        let group = row ?? .object(["id": .string(UUID().uuidString.lowercased()),"name": .string(""),"color": .null,"projectPaths": .array([])])
        editor = .object(["mode": .string(row == nil ? "create" : "update"),"revision": .number(Double(revision)),"group": group]);persist()
    }
    private func discardDraft() {guard let editor,let generation else {return};do {guard try model.clearBodyIfMatches(draftOwner,expectedBody: editorBody(editor),expectedGeneration: generation) else {throw RovaiHostFailure(code: "草稿已有新修改，已保留")};self.editor = nil} catch {issue = error.localizedDescription}}
    private func load() async throws {
        let value = try await readProjectGroups(model),related = try await readCanvasFolderRelations(model,roots: Array(Set(active+selected)).sorted())
        guard generation == model.connectionGeneration else {throw CancellationError()};source = value;folders = related
    }
    private func restore() async {
        guard !ready else {return};generation = model.connectionGeneration;busy = true;defer {busy = false}
        do {
            let rawPending = try model.readBody(pendingOwner)
            if !rawPending.isEmpty {guard let value = JSONValue.parse(rawPending),value["version"] == .number(1),value["params"]?.text("action") == "replace",value["params"]?["value"]?.isObject == true,case .number(let revision) = value["params"]?["expectedRevision"],revision.isFinite,revision>=0,revision<=9007199254740991,revision.rounded() == revision else {throw RovaiHostFailure(code: "原分组未决记录无法读取，已保留")};pending = value}
            let raw = try model.readBody(draftOwner)
            if !raw.isEmpty {guard let value = JSONValue.parse(raw),value["version"] == .number(1),value["editor"]?.isObject == true else {throw RovaiHostFailure(code: "原分组草稿无法读取，已保留")};editor = value["editor"]}
            let rawLayout = try model.readBody(layoutOwner)
            if !rawLayout.isEmpty {guard let value = JSONValue.parse(rawLayout),value["version"] == .number(1),let base = value["base"],let current = value["layout"] else {throw RovaiHostFailure(code: "原排版草稿无法读取，已保留")};initialLayout = try parseProjectLayout(base);layout = try parseProjectLayout(current);layoutRevision = value["revision"]?.rvInt ?? 0;arranging = editor == nil}
            try await load();ready = true
        } catch is CancellationError {} catch {issue = error.localizedDescription}
    }
    private func finish(_ value: JSONValue,record: JSONValue) throws {
        guard let generation,projectGroupPendingMatches(value,pending: record["params"] ?? .null) else {throw RovaiHostFailure(code: "当前分组与原修改尚未一致；保留原请求核对")}
        guard try model.clearBodyIfMatches(pendingOwner,expectedBody: encode(record),expectedGeneration: generation) else {throw RovaiHostFailure(code: "本机未决记录已有新修改，已保留")}
        if let body = record["layoutBody"]?.stringValue {_ = try model.clearBodyIfMatches(layoutOwner,expectedBody: body,expectedGeneration: generation);layout = nil;initialLayout = nil;arranging = false}
        if let body = record["editorBody"]?.stringValue {_ = try model.clearBodyIfMatches(draftOwner,expectedBody: body,expectedGeneration: generation)}
        source = value;pending = nil;editor = nil;issue = nil
    }
    private func send(_ record: JSONValue) async throws {
        let value = try parseProjectGroups(await model.query(.navigationProjectGroups,params: record["params"] ?? .null))
        guard generation == model.connectionGeneration else {throw CancellationError()};try finish(value,record: record)
    }
    private func apply(_ mutation: JSONValue) async {
        guard !blocked,!stale,let source else {return};busy = true;defer {busy = false}
        do {
            let expected = arranging ? layoutRevision : editor?["revision"]?.rvInt ?? self.revision,value = try projectGroupPlan(source,expectedRevision: expected,input: mutation,activePaths: active)
            let params: JSONValue = .object(["action": .string("replace"),"expectedRevision": .number(Double(expected)),"value": value])
            let record: JSONValue = .object(["version": .number(1),"params": params,"editorBody": try editor.map {JSONValue.string(try editorBody($0))} ?? .null,"layoutBody": arranging ? .string(try layoutBody()) : .null])
            try model.saveBody(pendingOwner,body: encode(record));guard generation == model.connectionGeneration else {throw CancellationError()};pending = record;try await send(record)
        } catch is CancellationError {} catch {issue = error.localizedDescription}
    }
    private func refresh() async {guard !busy else {return};busy = true;defer {busy = false};do {try await load();if let pending,let source,projectGroupPendingMatches(source,pending: pending["params"] ?? .null) {try finish(source,record: pending)}} catch is CancellationError {} catch {issue = error.localizedDescription}}
    private func retry(_ record: JSONValue) async {guard !busy else {return};busy = true;defer {busy = false};do {try await send(record)} catch is CancellationError {} catch {issue = error.localizedDescription}}
    private func abandonSuperseded() async {
        guard !busy,let pending,let generation else {return};busy = true;defer {busy = false}
        do {try await load();guard revision>(pending["params"]?["expectedRevision"]?.rvInt ?? Int.max) else {throw RovaiHostFailure(code: "原修改仍可能生效，请继续核对")};guard try model.clearBodyIfMatches(pendingOwner,expectedBody: encode(pending),expectedGeneration: generation) else {throw RovaiHostFailure(code: "本机记录已变化，已保留")};self.pending = nil;useCurrent = false} catch is CancellationError {} catch {issue = error.localizedDescription}
    }
}
