import SwiftUI

struct RovaiCanvasView: View {
    let model: RovaiWorkspaceModel; let id: String
    let onOpenThread: (String) -> Void
    @State private var snapshot: JSONValue?
    @State private var issue: String?
    @State private var offset = CGSize.zero
    @State private var zoom: CGFloat = 1
    @GestureState private var pan = CGSize.zero
    @GestureState private var magnification: CGFloat = 1
    @State private var selected: RovaiCanvasSelection?
    @State private var properties: RovaiCanvasSelection?
    @State private var newNote = false
    @State private var selecting = false, bulk = false
    @State private var chosen = Set<String>()
    @State private var bulkGraph: JSONValue?
    @State private var viewOwner: String?, viewGeneration: UInt64?
    @State private var viewReady = false, minimap = true
    @State private var stageSize = CGSize.zero
    @State private var packPlan: JSONValue?
    private var viewState: RovaiCanvasViewport {RovaiCanvasViewport(x: Double(offset.width),y: Double(offset.height),zoom: Double(zoom),minimap: minimap)}
    private var viewSignature: String {"\(viewOwner ?? ""):\(viewReady):\(offset.width):\(offset.height):\(zoom):\(minimap)"}
    private var revision: Int { model.boards.first { $0.text("id") == id }?["revision"]?.rvInt ?? 0 }
    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .topLeading) {
                Theme.bg.ignoresSafeArea()
                Canvas { context, size in let step = 32 * min(max(zoom * magnification, 0.1), 2); for x in stride(from: CGFloat(0), through: size.width, by: step) { for y in stride(from: CGFloat(0), through: size.height, by: step) { context.fill(Path(ellipseIn: CGRect(x: x, y: y, width: 1.3, height: 1.3)), with: .color(Theme.textTertiary.opacity(0.3))) } } }
                    .contentShape(Rectangle()).gesture(DragGesture().updating($pan) { value, state, _ in if viewReady {state = value.translation} }.onEnded { value in if viewReady {offset.width += value.translation.width; offset.height += value.translation.height} })
                    .simultaneousGesture(MagnifyGesture().updating($magnification) { value, state, _ in state = value.magnification }.onEnded { value in if viewReady {setZoom(min(max(zoom * value.magnification, 0.1), 2))} })
                ForEach((snapshot?["nodes"]?.rvArray ?? []).map { RovaiRow(value: $0) }) { row in RovaiCanvasCard(value: row.value, zoom: min(max(zoom * magnification, 0.1), 2), offset: CGSize(width: offset.width + pan.width, height: offset.height + pan.height), disabled: model.pending != nil || selecting, onOpen: {if selecting {if chosen.contains(row.id) {chosen.remove(row.id)} else {chosen.insert(row.id)}} else {selected = RovaiCanvasSelection(value: row.value)}}, onEdit: {properties = RovaiCanvasSelection(value: row.value)}, onMove: { x, y in mutate(.object(["type": .string("move"), "moves": .array([.object(["nodeId": .string(row.id), "x": .number(x), "y": .number(y)])])])) },isSelected: chosen.contains(row.id)) }
                if minimap {VStack {Spacer();HStack {Spacer();RovaiCanvasMiniMap(nodes: snapshot?["nodes"]?.rvArray ?? [],viewport: viewState,stage: geometry.size) {x,y in offset = CGSize(width: geometry.size.width/2-CGFloat(x)*zoom,height: geometry.size.height/2-CGFloat(y)*zoom)}}}.padding(12).padding(.bottom,80)}
                VStack {if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning).padding(10)};Spacer();HStack {
                    Button {if selecting {bulkGraph = snapshot;bulk = true} else {newNote = true}} label: {Label(selecting ? "操作 \(chosen.count) 张" : "添加",systemImage: selecting ? "square.stack" : "plus")}.disabled(snapshot == nil || model.pending != nil || selecting && chosen.isEmpty)
                    Menu {
                        Button(selecting ? "完成选择" : "多选卡片",systemImage: "checkmark.circle") {selecting.toggle()}
                        if selecting {Button("全选") {chosen = Set((snapshot?["nodes"]?.rvArray ?? []).map {$0.text("id")})};Button("取消选择") {chosen = []}}
                        Button("适合全部内容",systemImage: "arrow.up.left.and.arrow.down.right") {let fit = canvasFit(snapshot?["nodes"]?.rvArray ?? [],width: Double(geometry.size.width),height: Double(geometry.size.height));offset = CGSize(width: fit.x,height: fit.y);zoom = CGFloat(fit.zoom)}
                        Button("恢复100%") {setZoom(1)}
                        Button(minimap ? "隐藏小地图" : "显示小地图",systemImage: "map") {minimap.toggle()}
                        Button("整理全部卡片",systemImage: "square.grid.2x2") {if let snapshot {packPlan = .object(["boardId": .string(id),"expectedRevision": snapshot["board"]?["revision"] ?? .null,"mutation": .object(["type": .string("move"),"moves": .array(canvasPackedMoves(snapshot["nodes"]?.rvArray ?? []))])])}}.disabled(snapshot == nil || model.pending != nil)
                    } label: {Image(systemName: "ellipsis.circle")}
                    Spacer()
                    Button {setZoom(zoom/1.2)} label: {Image(systemName: "minus.circle")}.disabled(zoom <= 0.1)
                    Button {setZoom(1)} label: {Text("\(Int(zoom*100))%").font(.caption.monospacedDigit())}
                    Button {setZoom(zoom*1.2)} label: {Image(systemName: "plus.circle")}.disabled(zoom >= 2)
                }.padding(12).background(.regularMaterial,in: Capsule()).padding(16)}.frame(width: geometry.size.width,height: geometry.size.height)

            }.clipped().onAppear {stageSize = geometry.size}.onChange(of: geometry.size) {_,size in stageSize = size}
        }.navigationTitle(snapshot?["board"]?.text("name") ?? "无限画布")
        .task(id: "view:\(id):\(model.connectionGeneration)") {await restoreView()}
        .task(id: viewSignature) {guard viewReady else {return};do {try await Task.sleep(for: .milliseconds(300));try Task.checkCancellation();saveView()} catch {}}
        .onDisappear {saveView()}
        .task(id: "\(id):\(revision)") { await load() }
        .onChange(of: model.lastResult) { _, _ in Task { await load() } }
        .confirmationDialog("整理全部卡片？",isPresented: Binding(get: {packPlan != nil},set: {if !$0 {packPlan = nil}}),titleVisibility: .visible) {
            Button("整理位置") {if let packPlan {Task {await model.command(.canvasNodesApply,value: packPlan)}};packPlan = nil}.disabled(model.pending != nil || packPlan?["expectedRevision"] != snapshot?["board"]?["revision"])
            Button("取消",role: .cancel) {packPlan = nil}
        } message: {Text("按原版卡片尺寸排布位置；不会改动会话、文件或卡片内容。")}
        .sheet(isPresented: $bulk) {if let bulkGraph {RovaiCanvasSelectionActions(model: model,boardId: id,graph: bulkGraph,nodeIds: chosen)}}
        .sheet(item: $properties) {target in if let snapshot {RovaiCanvasNodeProperties(model: model, boardId: id, node: target.value, graph: snapshot)}}
        .sheet(item: $selected) { selected in if ["folder", "group", "agent", "custom"].contains(selected.value["data"]?.text("kind") ?? "") {RovaiCanvasRegion(model: model, boardId: id, node: selected.value, onOpenThread: onOpenThread)} else if selected.value["data"]?.text("kind") == "file" {RovaiCanvasFileNode(model: model, node: selected.value)} else if selected.value["data"]?.text("kind") == "conversation", selected.value["data"]?["objectReference"] == nil || selected.value["data"]?["objectReference"] == .null {RovaiCanvasConversationNode(model: model, boardId: id, node: selected.value, onOpenThread: onOpenThread)} else if selected.value["data"]?.text("kind") == "conversation", selected.value["data"]?["objectReference"] != nil && selected.value["data"]?["objectReference"] != .null {RovaiCanvasObjectNode(model: model, boardId: id, node: selected.value)} else if selected.value["data"]?.text("kind") == "terminal", let controller = model.terminal(boardId: id, nodeId: selected.id) { RovaiTerminalView(controller: controller) } else { NavigationStack { ScrollView { MarkdownContent(raw: selected.value["data"]?.text("content") ?? "").padding(20) }.navigationTitle(selected.value["data"]?.text("title") ?? "卡片").toolbar { ToolbarItem(placement: .confirmationAction) { Button("关闭") { self.selected = nil } } } } } }
        .sheet(isPresented: $newNote) {if let snapshot {RovaiCanvasCreator(model: model, boardId: id, graph: snapshot, x: Double(-offset.width / zoom + 80), y: Double(-offset.height / zoom + 80))}}
    }
    private func setZoom(_ value: CGFloat) {let next = min(2,max(0.1,value));let ratio = next/zoom;offset = CGSize(width: stageSize.width/2-(stageSize.width/2-offset.width)*ratio,height: stageSize.height/2-(stageSize.height/2-offset.height)*ratio);zoom = next}
    private func restoreView() async {
        viewReady = false;viewOwner = nil;let generation = model.connectionGeneration;viewGeneration = generation
        do {let identity = try await model.presentationIdentity();let tuple = JSONValue.array([.string(identity),.string(id)]);let owner = "canvas-view:" + String(decoding: try JSONEncoder().encode(tuple),as: UTF8.self);let saved = try RovaiCanvasViewport.parse(model.readBody(owner),board: id);try Task.checkCancellation();guard generation == model.connectionGeneration else {throw CancellationError()};offset = CGSize(width: saved.x,height: saved.y);zoom = CGFloat(saved.zoom);minimap = saved.minimap;viewOwner = owner;viewReady = true} catch is CancellationError {} catch {issue = error.localizedDescription;viewReady = true}
    }
    private func saveView() {guard viewReady,let viewOwner,viewGeneration == model.connectionGeneration else {return};do {try model.saveBody(viewOwner,body: viewState.document(board: id))} catch {issue = error.localizedDescription}}
    private func load() async { do { snapshot = try await model.query(.canvasNodesSnapshot, params: .object(["boardId": .string(id)])); issue = nil } catch { issue = error.localizedDescription } }
    private func mutate(_ value: JSONValue) { guard let snapshot else { return }; Task { await model.command(.canvasNodesApply, value: .object(["boardId": .string(id), "expectedRevision": snapshot["board"]?["revision"] ?? .number(0), "mutation": value])) } }
}
private struct RovaiCanvasSelection: Identifiable { let value: JSONValue; var id: String { value.text("id") } }
private struct RovaiCanvasCard: View {
    let value: JSONValue; let zoom: CGFloat; let offset: CGSize; let disabled: Bool; let onOpen: () -> Void; let onEdit: () -> Void; let onMove: (Double, Double) -> Void
    var isSelected = false
    @GestureState private var drag = CGSize.zero
    private var data: JSONValue { value["data"] ?? .object([:]) }
    private func number(_ key: String, fallback: Double) -> Double { if case .number(let number) = data[key] { return number }; return fallback }
    var body: some View { VStack(alignment: .leading, spacing: 0) { HStack { Image(systemName: data.text("kind") == "terminal" ? "terminal" : data.text("kind") == "note" ? "note.text" : "square.stack"); Text(data.text("title").isEmpty ? data.text("kind") : data.text("title")).font(.headline); Spacer(); Button(action: onOpen) { Image(systemName: "arrow.up.right.square") } }.padding(12).contentShape(Rectangle()).gesture(DragGesture().updating($drag) { value, state, _ in if !disabled { state = value.translation } }.onEnded { gesture in if !disabled { onMove(number("x", fallback: 0) + gesture.translation.width / zoom, number("y", fallback: 0) + gesture.translation.height / zoom) } }); Divider(); if data["collapsed"]?.rvBool != true && !data.text("content").isEmpty { Text(data.text("content")).font(.subheadline).lineLimit(8).padding(12) }; if data["collapsed"]?.rvBool != true && !data.text("path").isEmpty { Text(data.text("path")).font(.caption).foregroundStyle(Theme.textSecondary).lineLimit(2).padding(12) }; Spacer(minLength: 0) }.frame(width: number("width", fallback: 320), height: data["collapsed"]?.rvBool == true ? 52 : number("height", fallback: 220)).background {RoundedRectangle(cornerRadius: 16).fill(Theme.bgElevated).overlay {if let tint = rovaiCanvasTint(data.text("color")) {RoundedRectangle(cornerRadius: 16).fill(tint.opacity(0.12))}}}.hairlineBorder(16).overlay {if isSelected {RoundedRectangle(cornerRadius: 16).stroke(Theme.accent,lineWidth: 2)}}.contextMenu {Button("编辑卡片属性", systemImage: "pencil", action: onEdit)}.scaleEffect(zoom, anchor: .topLeading).offset(x: offset.width + number("x", fallback: 0) * zoom + drag.width, y: offset.height + number("y", fallback: 0) * zoom + drag.height) }
}
