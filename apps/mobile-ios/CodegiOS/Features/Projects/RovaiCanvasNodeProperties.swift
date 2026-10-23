import SwiftUI

let rovaiCanvasKindNames = ["folder": "工程区域", "group": "项目分组", "agent": "队员区域", "conversation": "对话", "custom": "自定义分组", "note": "便签", "file": "文件", "terminal": "终端"]

/// Original CanvasNodeEditor properties; saves never rewrite an immutable binding or a concurrent position.
struct RovaiCanvasNodeProperties: View {
    let model: RovaiWorkspaceModel
    let boardId: String
    let node: JSONValue
    let graph: JSONValue
    @Environment(\.dismiss) private var dismiss
    @State private var title = ""
    @State private var content = ""
    @State private var color = ""
    @State private var columns = 0
    @State private var rows = 0
    @State private var revision = 0
    @State private var collapsed = false
    @State private var ready = false
    @State private var busy = false
    @State private var removal = false
    @State private var commandId = UUID().uuidString.lowercased()
    @State private var generation: UInt64?
    @State private var issue: String?
    @State private var latest: JSONValue?
    private var data: JSONValue {node["data"] ?? .null}
    private var kind: String {data.text("kind")}
    private var owner: String {"canvas-node:\(boardId):\(node.text("id"))"}
    private var region: Bool {["folder", "group", "agent", "custom"].contains(kind)}
    private var current: JSONValue? {model.boards.first {$0.text("id") == boardId}}
    private var stale: Bool {current == nil || current?["revision"]?.rvInt != revision}
    private var blocked: Bool {!ready || busy || model.busy || model.pending != nil || generation != model.connectionGeneration}
    var body: some View {
        NavigationStack {Form {
            if let issue {Section {Text(issue).foregroundStyle(Theme.warning)}}
            if stale {Section {Text("画布版本已变化，当前草稿保留。需读取最新属性后重新编辑。").foregroundStyle(Theme.warning)}}
            if model.pending != nil {Section {Text("原请求待核对，可关闭后从工作区核对或重试。").font(.caption)}}
            Section("属性") {
                TextField("名称", text: changed($title))
                if kind == "note" {TextField("便签内容（支持Markdown）", text: changed($content), axis: .vertical).lineLimit(8...24)}
                Picker("颜色", selection: changed($color)) {ForEach(rovaiCanvasColors, id: \.0) {option in colorOption(option)}}
                Toggle("折叠卡片", isOn: changed($collapsed))
            }.disabled(blocked)
            if region {Section("区域网格") {
                Stepper("列数 · \(columns == 0 ? "自动" : String(columns))", value: changed($columns), in: 0...12)
                Stepper("行数 · \(rows == 0 ? "自动" : String(rows))", value: changed($rows), in: 0...12)
            }.disabled(blocked)}
            Section("原绑定") {
                ForEach(["folderPath", "folderGroupId", "agentId", "runtimeKind", "path"], id: \.self) {key in if !data.text(key).isEmpty {Text(data.text(key)).font(.caption).foregroundStyle(Theme.textSecondary)}}
                if data["conversation"] != nil, data["conversation"] != .null {Text("会话引用保留").font(.caption).foregroundStyle(Theme.textSecondary)}
            } footer: {Text("属性保存不改绑会话或文件，不重写其他端移动后的位置。")}
            Section {
                Button("读取最新属性") {Task {await loadLatest()}}.disabled(blocked)
                Button("保存属性") {Task {await submit(remove: false)}}.disabled(blocked || stale)
                Button("删除这张卡片", role: .destructive) {removal = true}.disabled(blocked || stale)
            }
        }.navigationTitle((rovaiCanvasKindNames[kind] ?? kind) + "属性").toolbar {Button("保留草稿并关闭") {close()}.disabled(busy)}}.interactiveDismissDisabled(true)
        .task {restore()}
        .onChange(of: model.lastResult) {_, result in
            let receipt = result?["commandResult"] ?? result
            guard model.lastInputOwner == owner, receipt?.text("commandId") == commandId else {return}
            if ["applied", "accepted"].contains(receipt?.text("status") ?? "") {ready = false;dismiss()}
            else if receipt?.text("status") == "rejected" {commandId = UUID().uuidString.lowercased();issue = receipt?.text("code");persist()}
        }
        .confirmationDialog("删除这张卡片？", isPresented: $removal, titleVisibility: .visible) {
            Button("删除卡片", role: .destructive) {Task {await submit(remove: true)}}.disabled(blocked || stale)
            Button("保留卡片", role: .cancel) {}
        } message: {Text(kind == "terminal" ? "此终端卡片会移除，仍运行的终端会关闭。工程文件保留。" : "只从画布移除此卡片；原会话、使命、任务和文件保留。便签正文仅属于此卡片，删除后无法恢复。")}
        .sheet(isPresented: Binding(get: {latest != nil}, set: {if !$0 {latest = nil}})) {
            if let latest, let next = (latest["nodes"]?.rvArray ?? []).first(where: {$0.text("id") == node.text("id")})?["data"] {NavigationStack {Form {
                Section("最新属性") {Text(next.text("title"));if kind == "note" {Text(next.text("content"))}}
                Section {Text("当前未保存的属性草稿将被替换，此操作不提交画布修改。");Button("放弃草稿，载入最新属性", role: .destructive) {applyLatest(latest, data: next)}.disabled(blocked)}
            }.navigationTitle("重新编辑属性？").toolbar {Button("保留当前草稿") {self.latest = nil}}}}
        }
    }
    private func changed<T>(_ binding: Binding<T>) -> Binding<T> {Binding(get: {binding.wrappedValue}, set: {binding.wrappedValue = $0;persist()})}
    private func colorOption(_ option: (String, String)) -> some View {
        let value: String = option.0
        let label: String = option.1
        let tint: Color = rovaiCanvasTint(value) ?? Theme.textSecondary
        return HStack {Circle().fill(tint).frame(width: 12, height: 12).accessibilityHidden(true);Text(label)}.tag(value)
    }
    private func document() throws -> String {let value = JSONValue.object(["version": .number(1), "boardId": .string(boardId), "nodeId": node["id"] ?? .null, "title": .string(title), "content": .string(content), "color": .string(color), "gridColumns": .number(Double(columns)), "gridRows": .number(Double(rows)), "collapsed": .bool(collapsed), "revision": .number(Double(revision)), "commandId": .string(commandId)]);let encoder = JSONEncoder();encoder.outputFormatting = [.sortedKeys];return String(decoding: try encoder.encode(value), as: UTF8.self)}
    private func persist() {guard ready, generation == model.connectionGeneration else {return};do {try model.saveBody(owner, body: document())} catch {issue = error.localizedDescription}}
    private func restore() {
        guard !ready else {return};generation = model.connectionGeneration;title = data.text("title");content = data.text("content");color = data.text("color");columns = data["gridColumns"]?.rvInt ?? 0;rows = data["gridRows"]?.rvInt ?? 0;collapsed = data["collapsed"]?.rvBool ?? false;revision = graph["board"]?["revision"]?.rvInt ?? 0
        do {let raw = try model.readBody(owner);if !raw.isEmpty {guard let value = JSONValue.parse(raw), value["version"]?.rvInt == 1, value.text("boardId") == boardId, value.text("nodeId") == node.text("id") else {throw RovaiHostFailure(code: "原卡片草稿无法读取，原记录保留")};title = value.text("title");content = value.text("content");color = value.text("color");columns = value["gridColumns"]?.rvInt ?? 0;rows = value["gridRows"]?.rvInt ?? 0;collapsed = value["collapsed"]?.rvBool ?? false;revision = value["revision"]?.rvInt ?? 0;if !value.text("commandId").isEmpty {commandId = value.text("commandId")}};ready = true} catch {issue = error.localizedDescription}
    }
    private func close() {guard !busy else {return};guard ready, generation == model.connectionGeneration else {dismiss();return};do {try model.saveBody(owner, body: document());dismiss()} catch {issue = error.localizedDescription}}
    private func loadLatest() async {
        guard !blocked else {return};busy = true;defer {busy = false}
        do {let value = try await model.query(.canvasNodesSnapshot, params: .object(["boardId": .string(boardId)]));guard generation == model.connectionGeneration else {throw CancellationError()};guard (value["nodes"]?.rvArray ?? []).contains(where: {$0.text("id") == node.text("id")}) else {throw RovaiHostFailure(code: "卡片已删除，当前草稿保留")};latest = value} catch {issue = error.localizedDescription}
    }
    private func applyLatest(_ value: JSONValue, data: JSONValue) {
        guard !blocked else {return};title = data.text("title");content = data.text("content");color = data.text("color");columns = data["gridColumns"]?.rvInt ?? 0;rows = data["gridRows"]?.rvInt ?? 0;collapsed = data["collapsed"]?.rvBool ?? false;revision = value["board"]?["revision"]?.rvInt ?? 0;commandId = UUID().uuidString.lowercased();latest = nil;persist()
    }
    private func submit(remove: Bool) async {
        guard !blocked, !stale else {return};busy = true;defer {busy = false}
        var patch: [String: JSONValue] = ["title": .string(title), "color": .string(color), "collapsed": .bool(collapsed)]
        if kind == "note" {patch["content"] = .string(content)};if region {patch["gridColumns"] = .number(Double(columns));patch["gridRows"] = .number(Double(rows))}
        let mutation: JSONValue = remove ? .object(["type": .string("delete"), "nodeIds": .array([node["id"] ?? .null])]) : .object(["type": .string("update"), "nodeId": node["id"] ?? .null, "patch": .object(patch)])
        do {let body = try document();try model.saveBody(owner, body: body);guard generation == model.connectionGeneration else {throw CancellationError()};await model.executeOwned(.canvasNodesApply, params: .object(["commandId": .string(commandId), "command": .object(["boardId": .string(boardId), "expectedRevision": .number(Double(revision)), "mutation": mutation])]), owner: owner, body: body)} catch {issue = error.localizedDescription}
    }
}
