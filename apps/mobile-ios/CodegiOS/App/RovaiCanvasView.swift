import SwiftUI

struct RovaiCanvasView: View {
    let model: RovaiWorkspaceModel; let id: String
    @State private var snapshot: JSONValue?
    @State private var issue: String?
    @State private var offset = CGSize.zero
    @State private var zoom: CGFloat = 1
    @GestureState private var pan = CGSize.zero
    @GestureState private var magnification: CGFloat = 1
    @State private var selected: RovaiCanvasSelection?
    @State private var note = ""
    @State private var newNote = false
    private var revision: Int { model.boards.first { $0.text("id") == id }?["revision"]?.rvInt ?? 0 }
    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .topLeading) {
                Theme.bg.ignoresSafeArea()
                Canvas { context, size in let step = 32 * min(max(zoom * magnification, 0.1), 2); for x in stride(from: CGFloat(0), through: size.width, by: step) { for y in stride(from: CGFloat(0), through: size.height, by: step) { context.fill(Path(ellipseIn: CGRect(x: x, y: y, width: 1.3, height: 1.3)), with: .color(Theme.textTertiary.opacity(0.3))) } } }
                    .contentShape(Rectangle()).gesture(DragGesture().updating($pan) { value, state, _ in state = value.translation }.onEnded { value in offset.width += value.translation.width; offset.height += value.translation.height })
                    .simultaneousGesture(MagnifyGesture().updating($magnification) { value, state, _ in state = value.magnification }.onEnded { value in zoom = min(max(zoom * value.magnification, 0.1), 2) })
                ForEach((snapshot?["nodes"]?.rvArray ?? []).map { RovaiRow(value: $0) }) { row in RovaiCanvasCard(value: row.value, zoom: min(max(zoom * magnification, 0.1), 2), offset: CGSize(width: offset.width + pan.width, height: offset.height + pan.height), disabled: model.pending != nil, onOpen: { selected = RovaiCanvasSelection(value: row.value) }, onMove: { x, y in mutate(.object(["type": .string("move"), "moves": .array([.object(["nodeId": .string(row.id), "x": .number(x), "y": .number(y)])])])) }) }
                VStack { if let issue { Text(issue).font(.caption).foregroundStyle(Theme.warning).padding(10) }; Spacer(); HStack { Button { newNote = true } label: { Label("便签", systemImage: "plus") }.disabled(snapshot == nil || model.pending != nil); Button { offset = .zero; zoom = 1 } label: { Image(systemName: "arrow.up.left.and.arrow.down.right") }; Text("\(Int(zoom * 100))%").font(.caption.monospacedDigit()); Spacer() }.padding(12).background(.regularMaterial, in: Capsule()).padding(16) }.frame(width: geometry.size.width, height: geometry.size.height)
            }.clipped()
        }.navigationTitle(snapshot?["board"]?.text("name") ?? "无限画布")
        .task(id: "\(id):\(revision)") { await load() }
        .onChange(of: model.lastResult) { _, _ in Task { await load() } }
        .sheet(item: $selected) { selected in if selected.value["data"]?.text("kind") == "terminal", let controller = model.terminal(boardId: id, nodeId: selected.id) { RovaiTerminalView(controller: controller) } else { NavigationStack { ScrollView { MarkdownContent(raw: selected.value["data"]?.text("content") ?? "").padding(20) }.navigationTitle(selected.value["data"]?.text("title") ?? "卡片").toolbar { ToolbarItem(placement: .confirmationAction) { Button("关闭") { self.selected = nil } } } } } }
        .alert("添加便签", isPresented: $newNote) { TextField("内容", text: $note); Button("取消", role: .cancel) {}; Button("添加") { mutate(.object(["type": .string("create"), "node": .object(["kind": .string("note"), "title": .string("便签"), "content": .string(note), "x": .number(100), "y": .number(100), "width": .number(320), "height": .number(220)])])); note = "" } }
    }
    private func load() async { do { snapshot = try await model.query(.canvasNodesSnapshot, params: .object(["boardId": .string(id)])); issue = nil } catch { issue = error.localizedDescription } }
    private func mutate(_ value: JSONValue) { guard let snapshot else { return }; Task { await model.command(.canvasNodesApply, value: .object(["boardId": .string(id), "expectedRevision": snapshot["board"]?["revision"] ?? .number(0), "mutation": value])) } }
}
private struct RovaiCanvasSelection: Identifiable { let value: JSONValue; var id: String { value.text("id") } }
private struct RovaiCanvasCard: View {
    let value: JSONValue; let zoom: CGFloat; let offset: CGSize; let disabled: Bool; let onOpen: () -> Void; let onMove: (Double, Double) -> Void
    @GestureState private var drag = CGSize.zero
    private var data: JSONValue { value["data"] ?? .object([:]) }
    private func number(_ key: String, fallback: Double) -> Double { if case .number(let number) = data[key] { return number }; return fallback }
    var body: some View { VStack(alignment: .leading, spacing: 0) { HStack { Image(systemName: data.text("kind") == "terminal" ? "terminal" : data.text("kind") == "note" ? "note.text" : "square.stack"); Text(data.text("title").isEmpty ? data.text("kind") : data.text("title")).font(.headline); Spacer(); Button(action: onOpen) { Image(systemName: "arrow.up.right.square") } }.padding(12).contentShape(Rectangle()).gesture(DragGesture().updating($drag) { value, state, _ in if !disabled { state = value.translation } }.onEnded { gesture in if !disabled { onMove(number("x", fallback: 0) + gesture.translation.width / zoom, number("y", fallback: 0) + gesture.translation.height / zoom) } }); Divider(); if !data.text("content").isEmpty { Text(data.text("content")).font(.subheadline).lineLimit(8).padding(12) }; if !data.text("path").isEmpty { Text(data.text("path")).font(.caption).foregroundStyle(Theme.textSecondary).lineLimit(2).padding(12) }; Spacer(minLength: 0) }.frame(width: number("width", fallback: 320), height: number("height", fallback: 220)).background(Theme.bgElevated, in: RoundedRectangle(cornerRadius: 16)).hairlineBorder(16).scaleEffect(zoom, anchor: .topLeading).offset(x: offset.width + number("x", fallback: 0) * zoom + drag.width, y: offset.height + number("y", fallback: 0) * zoom + drag.height) }
}
