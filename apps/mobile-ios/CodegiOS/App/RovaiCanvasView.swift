import SwiftUI

struct RovaiCanvasView: View {
    let model: RovaiWorkspaceModel; let id: String
    let onOpenThread: (String) -> Void
    @Environment(\.scenePhase) private var scenePhase
    @State private var catalog = RovaiCanvasCatalog()
    @State private var expandedRegions = Set<String>()
    @State private var memberOpening: RovaiCanvasMemberSelection?
    @State private var snapshot: JSONValue?
    @State private var issue: String?
    @State private var offset = CGSize.zero
    @State private var zoom: CGFloat = 1
    @GestureState private var pan = CGSize.zero
    @GestureState private var magnification: CGFloat = 1
    @State private var selected: RovaiCanvasSelection?
    @State private var properties: RovaiCanvasSelection?
    @State private var newNote = false
    @State private var selecting = false
    @State private var bulk = false
    @State private var chosen = Set<String>()
    @State private var bulkGraph: JSONValue?
    @State private var viewOwner: String?
    @State private var viewGeneration: UInt64?
    @State private var viewReady = false
    @State private var minimap = true
    @State private var stageSize = CGSize.zero
    @State private var packPlan: JSONValue?
    private var viewState: RovaiCanvasViewport {RovaiCanvasViewport(x: Double(offset.width),y: Double(offset.height),zoom: Double(zoom),minimap: minimap,expandedRegions: expandedRegions.sorted())}
    private var viewSignature: String {"\(viewOwner ?? ""):\(viewReady):\(offset.width):\(offset.height):\(zoom):\(minimap):\(expandedRegions.sorted())"}
    private var revision: Int { model.boards.first { $0.text("id") == id }?["revision"]?.rvInt ?? 0 }
    private var presentationNodes: [JSONValue] {canvasPresentationNodes(snapshot?["nodes"]?.rvArray ?? [],catalog: catalog.snapshot,expanded: expandedRegions)}
    private var catalogScope: String {"\(id):\(model.connectionGeneration)"}
    private var bindings: String {let keys = ["kind","folderPath","folderGroupId","agentId","runtimeKind","conversation","conversations"];let rows = (snapshot?["nodes"]?.rvArray ?? []).map {node in var value: [String:JSONValue] = ["id": node["id"] ?? .null];for key in keys {if let field = node["data"]?[key] {value[key] = field}};return JSONValue.object(value)};return String(decoding: (try? JSONEncoder().encode(JSONValue.array(rows))) ?? Data(),as: UTF8.self)}
    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .topLeading) {
                Theme.bg.ignoresSafeArea()
                Canvas { context, size in let step = 32 * min(max(zoom * magnification, 0.1), 2); for x in stride(from: CGFloat(0), through: size.width, by: step) { for y in stride(from: CGFloat(0), through: size.height, by: step) { context.fill(Path(ellipseIn: CGRect(x: x, y: y, width: 1.3, height: 1.3)), with: .color(Theme.textTertiary.opacity(0.3))) } } }
                    .contentShape(Rectangle()).gesture(DragGesture().updating($pan) { value, state, _ in if viewReady {state = value.translation} }.onEnded { value in if viewReady {offset.width += value.translation.width; offset.height += value.translation.height} })
                    .simultaneousGesture(MagnifyGesture().updating($magnification) { value, state, _ in state = value.magnification }.onEnded { value in if viewReady {setZoom(min(max(zoom * value.magnification, 0.1), 2))} })
                ForEach((snapshot?["nodes"]?.rvArray ?? []).map { RovaiRow(value: $0) }) { row in RovaiCanvasCard(value: row.value, zoom: min(max(zoom * magnification, 0.1), 2), offset: CGSize(width: offset.width + pan.width, height: offset.height + pan.height), disabled: model.pending != nil || selecting, onOpen: {if selecting {if chosen.contains(row.id) {chosen.remove(row.id)} else {chosen.insert(row.id)}} else {selected = RovaiCanvasSelection(value: row.value)}}, onEdit: {properties = RovaiCanvasSelection(value: row.value)}, onMove: { x, y, captured in move(row.value,x: x,y: y,revision: captured) },isSelected: chosen.contains(row.id),boardRevision: snapshot?["board"]?["revision"]?.rvInt ?? 0,onResize: {width,height,captured in mutate(.object(["type": .string("update"),"nodeId": .string(row.id),"patch": .object(["width": .number(width),"height": .number(height)])]),revision: captured)},catalog: catalog.snapshot,profiles: model.members,expanded: expandedRegions.contains(row.id),onExpand: {if expandedRegions.contains(row.id) {expandedRegions.remove(row.id)} else {expandedRegions.insert(row.id)}},onMember: {reference in memberOpening = RovaiCanvasMemberSelection(node: row.value,reference: reference)},onDetach: {reference in detach(row.value,reference: reference)},onMemberMove: {reference,x,y,captured in move(row.value,member: reference,x: x,y: y,revision: captured)}) }
                if minimap {VStack {Spacer();HStack {Spacer();RovaiCanvasMiniMap(nodes: presentationNodes,viewport: viewState,stage: geometry.size) {x,y in offset = CGSize(width: geometry.size.width/2-CGFloat(x)*zoom,height: geometry.size.height/2-CGFloat(y)*zoom)}}}.padding(12).padding(.bottom,80)}
                VStack {if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning).padding(10)};Spacer();HStack {
                    Button {if selecting {bulkGraph = snapshot;bulk = true} else {newNote = true}} label: {Label(selecting ? "操作 \(chosen.count) 张" : "添加",systemImage: selecting ? "square.stack" : "plus")}.disabled(snapshot == nil || model.pending != nil || selecting && chosen.isEmpty)
                    Menu {
                        Button(selecting ? "完成选择" : "多选卡片",systemImage: "checkmark.circle") {selecting.toggle()}
                        if selecting {Button("全选") {chosen = Set((snapshot?["nodes"]?.rvArray ?? []).map {$0.text("id")})};Button("取消选择") {chosen = []}}
                        Button("适合全部内容",systemImage: "arrow.up.left.and.arrow.down.right") {let fit = canvasFit(presentationNodes,width: Double(geometry.size.width),height: Double(geometry.size.height));offset = CGSize(width: fit.x,height: fit.y);zoom = CGFloat(fit.zoom)}
                        Button("恢复100%") {setZoom(1)}
                        Button(minimap ? "隐藏小地图" : "显示小地图",systemImage: "map") {minimap.toggle()}
                        Button("整理全部卡片",systemImage: "square.grid.2x2") {if let snapshot {packPlan = .object(["boardId": .string(id),"expectedRevision": snapshot["board"]?["revision"] ?? .null,"mutation": .object(["type": .string("move"),"moves": .array(canvasPackedMoves(presentationNodes))])])}}.disabled(snapshot == nil || model.pending != nil)
                    } label: {Image(systemName: "ellipsis.circle")}
                    Spacer()
                    Button {setZoom(zoom/1.2)} label: {Image(systemName: "minus.circle")}.disabled(zoom <= 0.1)
                    Button {setZoom(1)} label: {Text("\(Int(zoom*100))%").font(.caption.monospacedDigit())}
                    Button {setZoom(zoom*1.2)} label: {Image(systemName: "plus.circle")}.disabled(zoom >= 2)
                }.padding(12).background(.regularMaterial,in: Capsule()).padding(16)}.frame(width: geometry.size.width,height: geometry.size.height)

            }.clipped().onAppear {stageSize = geometry.size}.onChange(of: geometry.size) {_,size in stageSize = size}
        }.navigationTitle(snapshot?["board"]?.text("name") ?? "无限画布")
        .task(id: "catalog:\(catalogScope):\(scenePhase == .active):\(snapshot != nil)") {catalog.configure(catalogScope);guard scenePhase == .active,snapshot != nil else {return};while !Task.isCancelled {await catalog.load(model,nodes: snapshot?["nodes"]?.rvArray ?? []);do {try await Task.sleep(for: .seconds(20))} catch {return}}}
        .task(id: "bindings:\(catalogScope):\(bindings):\(scenePhase == .active)") {guard scenePhase == .active,snapshot != nil else {return};do {try await Task.sleep(for: .milliseconds(300));try Task.checkCancellation();await catalog.load(model,nodes: snapshot?["nodes"]?.rvArray ?? [])} catch {}}
        .task(id: "view:\(id):\(model.connectionGeneration)") {await restoreView()}
        .task(id: viewSignature) {guard viewReady else {return};do {try await Task.sleep(for: .milliseconds(300));try Task.checkCancellation();saveView()} catch {}}
        .onDisappear {saveView()}
        .task(id: "\(id):\(revision)") { await load() }
        .onChange(of: model.lastResult) { _, _ in Task { await load() } }
        .confirmationDialog("整理全部卡片？",isPresented: Binding(get: {packPlan != nil},set: {if !$0 {packPlan = nil}}),titleVisibility: .visible) {
            Button("整理位置") {if let packPlan {Task {await model.command(.canvasNodesApply,value: packPlan)}};packPlan = nil}.disabled(model.pending != nil || packPlan?["expectedRevision"] != snapshot?["board"]?["revision"])
            Button("取消",role: .cancel) {packPlan = nil}
        } message: {Text("按原版卡片尺寸排布位置；不会改动会话、文件或卡片内容。")}
        .sheet(item: $memberOpening) {target in RovaiCanvasConversationNode(model: model,boardId: id,node: target.node,onOpenThread: onOpenThread,memberReference: target.reference)}
        .sheet(isPresented: $bulk) {if let bulkGraph {RovaiCanvasSelectionActions(model: model,boardId: id,graph: bulkGraph,nodeIds: chosen)}}
        .sheet(item: $properties) {target in if let snapshot {RovaiCanvasNodeProperties(model: model, boardId: id, node: target.value, graph: snapshot)}}
        .sheet(item: $selected) { selected in if ["folder", "group", "agent", "custom"].contains(selected.value["data"]?.text("kind") ?? "") {RovaiCanvasRegion(model: model, boardId: id, node: selected.value, onOpenThread: onOpenThread)} else if selected.value["data"]?.text("kind") == "file", selected.value["data"]?["fileReference"]?.isObject != true {if let snapshot {RovaiCanvasCreator(model: model,boardId: id,graph: snapshot,x: 0,y: 0,bindingNode: selected.value)}} else if selected.value["data"]?.text("kind") == "file" {RovaiCanvasFileNode(model: model, node: selected.value)} else if selected.value["data"]?.text("kind") == "conversation", selected.value["data"]?["objectReference"] == nil || selected.value["data"]?["objectReference"] == .null {RovaiCanvasConversationNode(model: model, boardId: id, node: selected.value, onOpenThread: onOpenThread)} else if selected.value["data"]?.text("kind") == "conversation", selected.value["data"]?["objectReference"] != nil && selected.value["data"]?["objectReference"] != .null {RovaiCanvasObjectNode(model: model, boardId: id, node: selected.value)} else if selected.value["data"]?.text("kind") == "terminal", let controller = model.terminal(boardId: id, nodeId: selected.id) { RovaiTerminalView(controller: controller) } else { NavigationStack { ScrollView { MarkdownContent(raw: selected.value["data"]?.text("content") ?? "").padding(20) }.navigationTitle(selected.value["data"]?.text("title") ?? "卡片").toolbar { ToolbarItem(placement: .confirmationAction) { Button("关闭") { self.selected = nil } } } } } }
        .sheet(isPresented: $newNote) {if let snapshot {RovaiCanvasCreator(model: model, boardId: id, graph: snapshot, x: Double(-offset.width / zoom + 80), y: Double(-offset.height / zoom + 80))}}
    }
    private func move(_ node: JSONValue,member: JSONValue? = nil,x: Double,y: Double,revision: Int) {
        guard snapshot?["board"]?["revision"] == .number(Double(revision)) else {issue = "画布已变化，请核对当前位置后重新拖动";return}
        do {if let mutation = try canvasDropMutation(source: node,member: member,x: x,y: y,nodes: presentationNodes,memberTransferVersion: snapshot?["memberTransferVersion"]?.rvInt ?? 0) {mutate(mutation,revision: revision)}} catch {issue = error.localizedDescription}
    }
    private func detach(_ node: JSONValue,reference: JSONValue) {let data = node["data"] ?? .null;func scalar(_ key: String,_ fallback: Double) -> Double {if case .number(let value) = data[key] {return value};return fallback};mutate(.object(["type": .string("detach"),"regionId": node["id"] ?? .null,"conversation": reference,"x": .number(scalar("x",0)+scalar("width",484)+48),"y": .number(scalar("y",0))]))}
    private func setZoom(_ value: CGFloat) {let next = min(2,max(0.1,value));let ratio = next/zoom;offset = CGSize(width: stageSize.width/2-(stageSize.width/2-offset.width)*ratio,height: stageSize.height/2-(stageSize.height/2-offset.height)*ratio);zoom = next}
    private func restoreView() async {
        viewReady = false;viewOwner = nil;let generation = model.connectionGeneration;viewGeneration = generation
        do {let identity = try await model.presentationIdentity();let tuple = JSONValue.array([.string(identity),.string(id)]);let owner = "canvas-view:" + String(decoding: try JSONEncoder().encode(tuple),as: UTF8.self);let saved = try RovaiCanvasViewport.parse(model.readBody(owner),board: id);try Task.checkCancellation();guard generation == model.connectionGeneration else {throw CancellationError()};offset = CGSize(width: saved.x,height: saved.y);zoom = CGFloat(saved.zoom);minimap = saved.minimap;expandedRegions = Set(saved.expandedRegions);viewOwner = owner;viewReady = true} catch is CancellationError {} catch {issue = error.localizedDescription;viewReady = true}
    }
    private func saveView() {guard viewReady,let viewOwner,viewGeneration == model.connectionGeneration else {return};do {try model.saveBody(viewOwner,body: viewState.document(board: id))} catch {issue = error.localizedDescription}}
    private func load() async { do { snapshot = try await model.query(.canvasNodesSnapshot, params: .object(["boardId": .string(id)])); issue = nil } catch { issue = error.localizedDescription } }
    private func mutate(_ value: JSONValue,revision: Int? = nil) { guard let snapshot else { return }; Task { await model.command(.canvasNodesApply, value: .object(["boardId": .string(id), "expectedRevision": revision.map {JSONValue.number(Double($0))} ?? snapshot["board"]?["revision"] ?? .number(0), "mutation": value])) } }
}
private struct RovaiCanvasSelection: Identifiable { let value: JSONValue; var id: String { value.text("id") } }
private struct RovaiCanvasMemberSelection: Identifiable {let node: JSONValue;let reference: JSONValue;var id: String {node.text("id")+RovaiCanvasReferenceChoice(reference: reference,label: "").id}}
