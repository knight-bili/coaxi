import SwiftUI

let rovaiCanvasColors = [("", "默认"), ("neutral", "中性"), ("zinc", "锌灰"), ("slate", "蓝灰"), ("stone", "石灰"), ("gray", "灰色"), ("red", "红色"), ("rose", "玫瑰"), ("orange", "橙色"), ("green", "绿色"), ("blue", "蓝色"), ("yellow", "黄色"), ("violet", "紫色")]

/// Codeg BoardCard's footprint preview, compact copy and separate menu, using revision-matched Core summaries.
struct RovaiCanvasBoards: View {
    let model: RovaiWorkspaceModel
    let onOpen: (String) -> Void
    @State private var search = ""
    @State private var editor: Selection?
    private struct Selection: Identifiable {let id = UUID();let board: JSONValue?;let remove: Bool}
    private var blocked: Bool {model.busy || model.pending != nil || !model.connected}
    private var rows: [JSONValue] {model.boards.filter {search.isEmpty || ($0.text("name") + " " + $0.text("description")).localizedCaseInsensitiveContains(search)}}
    var body: some View {
        ScrollView {VStack(alignment: .leading, spacing: 16) {
            HStack {VStack(alignment: .leading, spacing: 6) {Text("画布").font(.title2.weight(.semibold));Text("每个画布都有自己的区域、会话卡片、便签、文件和终端。").font(.caption).foregroundStyle(Theme.textSecondary)};Spacer();Button {editor = Selection(board: nil, remove: false)} label: {Label("新建画布", systemImage: "plus")}.disabled(blocked)}
            TextField("搜索画布", text: $search).textFieldStyle(.roundedBorder)
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 180), spacing: 12)], spacing: 12) {
                ForEach(rows, id: \.self) {board in
                    let summary = model.boardSummaries.first {$0.text("boardId") == board.text("id") && $0["revision"] == board["revision"]}
                    GlassRow {VStack(alignment: .leading, spacing: 8) {
                        Button {onOpen(board.text("id"))} label: {VStack(alignment: .leading, spacing: 10) {
                            RovaiCanvasBoardPreview(rects: summary?["preview"]?.rvArray ?? []).frame(height: 92)
                            Text(board.text("name").isEmpty ? "未命名画布" : board.text("name")).font(.headline).foregroundStyle(Theme.textPrimary)
                            if !board.text("description").isEmpty {Text(board.text("description")).font(.caption).foregroundStyle(Theme.textSecondary).lineLimit(3)}
                        }.frame(maxWidth: .infinity, alignment: .leading)}.buttonStyle(.plain)
                        HStack {Text(summary == nil ? "统计暂不可用" : "\(summary?["nodeCount"]?.rvInt ?? 0)个节点").font(.caption).foregroundStyle(Theme.textSecondary);Spacer();Menu {actions(board)} label: {Image(systemName: "ellipsis").padding(6)}.accessibilityLabel("画布操作")}
                        Text(board.text("updatedAt")).font(.caption2).foregroundStyle(Theme.textSecondary)
                    }}.contextMenu {actions(board)}
                }
            }
            if rows.isEmpty {ContentUnavailableView(model.boards.isEmpty ? "还没有画布" : "没有符合条件的画布", systemImage: "map", description: Text(model.boards.isEmpty ? "创建一个画布开始整理工作。" : "调整搜索内容后重试。"))}
        }.padding(16)}
        .sheet(item: $editor) {target in RovaiCanvasBoardEditor(model: model, board: target.board, remove: target.remove)}
    }
    @ViewBuilder private func actions(_ board: JSONValue) -> some View {
        Button("打开画布", systemImage: "arrow.up.right.square") {onOpen(board.text("id"))}
        Button("编辑画布", systemImage: "pencil") {editor = Selection(board: board, remove: false)}.disabled(blocked)
        Button("删除画布", systemImage: "trash", role: .destructive) {editor = Selection(board: board, remove: true)}.disabled(blocked)
    }
}

struct RovaiCanvasBoardPreview: View {
    let rects: [JSONValue]
    private func number(_ value: JSONValue, _ key: String) -> Double {if case .number(let number) = value[key] {return number};return 0}
    var body: some View {
        Canvas {context, size in
            guard !rects.isEmpty else {let box = CGRect(x: size.width * 0.38, y: size.height * 0.2, width: size.width * 0.24, height: size.height * 0.6);context.stroke(Path(box), with: .color(Theme.textSecondary.opacity(0.18)), lineWidth: 1);return}
            let minX = rects.map {number($0, "x")}.min() ?? 0, minY = rects.map {number($0, "y")}.min() ?? 0
            let maxX = rects.map {number($0, "x") + number($0, "width")}.max() ?? 0, maxY = rects.map {number($0, "y") + number($0, "height")}.max() ?? 0
            let width = max(1200, maxX - minX), height = max(720, maxY - minY), left = (minX + maxX) / 2 - width * 0.58, top = (minY + maxY) / 2 - height * 0.58
            let scale = min(Double(size.width) / (width * 1.16), Double(size.height) / (height * 1.16)), dx = (Double(size.width) - width * 1.16 * scale) / 2, dy = (Double(size.height) - height * 1.16 * scale) / 2
            for rect in rects.prefix(200) {
                let region = ["folder", "group", "agent", "custom"].contains(rect.text("kind"))
                let box = CGRect(x: dx + (number(rect, "x") - left) * scale, y: dy + (number(rect, "y") - top) * scale, width: number(rect, "width") * scale, height: number(rect, "height") * scale)
                let path = Path(roundedRect: box, cornerRadius: region ? 2 : 1)
                context.fill(path, with: .color(region ? Theme.textSecondary.opacity(0.06) : Theme.bgElevated));context.stroke(path, with: .color(Theme.textSecondary.opacity(0.22)), lineWidth: 1)
            }
        }.accessibilityHidden(true)
    }
}

struct RovaiCanvasBoardEditor: View {
    let model: RovaiWorkspaceModel
    let board: JSONValue?
    let remove: Bool
    @Environment(\.dismiss) private var dismiss
    @State private var name = "", detail = "", color = ""
    @State private var revision = 0
    @State private var commandId = UUID().uuidString.lowercased()
    @State private var ready = false, busy = false
    @State private var generation: UInt64?, issue: String?
    private var owner: String {"canvas-board:\(board?.text("id") ?? "new"):\(remove ? "remove" : "edit")"}
    private var latest: JSONValue? {model.boards.first {$0.text("id") == board?.text("id")}}
    private var stale: Bool {board != nil && (latest == nil || latest?["revision"]?.rvInt != revision)}
    private var blocked: Bool {!ready || busy || model.busy || model.pending != nil || generation != model.connectionGeneration}
    private var summary: JSONValue? {model.boardSummaries.first {$0.text("boardId") == board?.text("id") && $0["revision"]?.rvInt == revision && !stale}}
    var body: some View {
        NavigationStack {Form {
            if let issue {Section {Text(issue).foregroundStyle(Theme.warning)}}
            if remove {Section {
                Text(board?.text("name") ?? "未命名画布")
                Text("删除画布会移除其中的卡片、区域和便签，保留原对话、使命、任务及工程文件。")
                if let summary {Text("包含\(summary["nodeCount"]?.rvInt ?? 0)个节点，其中\(summary["terminalCount"]?.rvInt ?? 0)张终端卡；仍运行的终端会关闭。")} else {Text("当前数量统计暂不可用，不能假定画布为空。")}
            }} else {Section {
                TextField("名称", text: changed($name))
                TextField("说明（可选）", text: changed($detail), axis: .vertical).lineLimit(3...8)
                Picker("颜色", selection: changed($color)) {ForEach(rovaiCanvasColors, id: \.0) {key, label in Text(label).tag(key)}}
            }.disabled(blocked)}
            if stale {Section {Text("画布已有新修改，当前输入保留。请核对后继续。").foregroundStyle(Theme.warning);if !remove, let latest {Button("使用最新画布版本") {revision = latest["revision"]?.rvInt ?? 0;commandId = UUID().uuidString.lowercased();persist()}.disabled(blocked)}}}
            if model.pending != nil {Section {Text("原请求待核对；可关闭后从工作区核对或重试。").font(.caption)}}
            Section {Button(remove ? "删除画布" : "保存", role: remove ? .destructive : nil) {Task {await submit()}}.disabled(blocked || stale || name.unicodeScalars.count > 200 || detail.unicodeScalars.count > 2000)}
        }.navigationTitle(remove ? "删除画布？" : board == nil ? "新建画布" : "编辑画布").toolbar {Button("关闭并保留草稿") {close()}.disabled(busy)}}.interactiveDismissDisabled(true)
        .task {restore()}
        .onChange(of: model.lastResult) {_, result in
            let receipt = result?["commandResult"] ?? result
            guard model.lastInputOwner == owner, receipt?.text("commandId") == commandId else {return}
            if ["applied", "accepted"].contains(receipt?.text("status") ?? "") {ready = false;dismiss()}
            else if receipt?.text("status") == "rejected" {commandId = UUID().uuidString.lowercased();issue = receipt?.text("code");persist()}
        }
    }
    private func changed<T>(_ binding: Binding<T>) -> Binding<T> {Binding(get: {binding.wrappedValue}, set: {binding.wrappedValue = $0;persist()})}
    private func document() throws -> String {let value = JSONValue.object(["version": .number(1), "boardId": board?["id"] ?? .null, "name": .string(name), "description": .string(detail), "color": .string(color), "revision": .number(Double(revision)), "commandId": .string(commandId)]);let encoder = JSONEncoder();encoder.outputFormatting = [.sortedKeys];return String(decoding: try encoder.encode(value), as: UTF8.self)}
    private func persist() {guard ready, generation == model.connectionGeneration else {return};do {try model.saveBody(owner, body: document())} catch {issue = error.localizedDescription}}
    private func restore() {
        guard !ready else {return};generation = model.connectionGeneration;name = board?.text("name") ?? "";detail = board?.text("description") ?? "";color = board?.text("color") ?? "";revision = board?["revision"]?.rvInt ?? 0
        do {let raw = try model.readBody(owner);if !raw.isEmpty {guard let value = JSONValue.parse(raw), value["version"]?.rvInt == 1, value["boardId"]?.stringValue == board?["id"]?.stringValue else {throw RovaiHostFailure(code: "原画布草稿无法读取，原记录保留")};name = value.text("name");detail = value.text("description");color = value.text("color");revision = value["revision"]?.rvInt ?? 0;if !value.text("commandId").isEmpty {commandId = value.text("commandId")}};ready = true} catch {issue = error.localizedDescription}
    }
    private func close() {guard !busy else {return};guard ready, generation == model.connectionGeneration else {dismiss();return};do {try model.saveBody(owner, body: document());dismiss()} catch {issue = error.localizedDescription}}
    private func submit() async {
        guard !blocked, !stale else {return};busy = true;defer {busy = false}
        var command: [String: JSONValue] = [:];if let board {command["boardId"] = board["id"] ?? .null;command["expectedRevision"] = .number(Double(revision))};if !remove {command["name"] = .string(name);command["description"] = .string(detail);command["color"] = .string(color)}
        do {let body = try document();try model.saveBody(owner, body: body);guard generation == model.connectionGeneration else {throw CancellationError()};await model.executeOwned(remove ? .canvasBoardsDelete : board == nil ? .canvasBoardsCreate : .canvasBoardsUpdate, params: .object(["commandId": .string(commandId), "command": .object(command)]), owner: owner, body: body)} catch {issue = error.localizedDescription}
    }
}
