import SwiftUI

/// Original derived member cards and atomic custom membership/group/detach commands.
struct RovaiCanvasRegion: View {
    let model: RovaiWorkspaceModel
    let boardId: String
    let node: JSONValue
    let onOpenThread: (String) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var graph: JSONValue?
    @State private var current: JSONValue?
    @State private var issue: String?
    @State private var generation: UInt64?
    @State private var rows: [JSONValue] = []
    @State private var groups: [JSONValue] = []
    @State private var folders: [String:RovaiCanvasFolderRelation] = [:]
    @State private var cursor: RovaiCanvasReferenceCursor?
    @State private var loaded = false
    @State private var busy = false
    @State private var search = ""
    @State private var target = ""
    @State private var selected = Set<String>()
    @State private var opening: RovaiCanvasRegionMember?
    private var id: String {node.text("id")}
    private var owner: String {"canvas-region-op:\(boardId):\(id)"}
    private var activeNode: JSONValue {current ?? node}
    private var data: JSONValue {activeNode["data"] ?? .null}
    private var custom: Bool {data.text("kind") == "custom"}
    private var resolution: RovaiCanvasRegionResolution {resolveCanvasRegion(activeNode, rows: rows, catalogComplete: loaded && cursor == nil, groups: groups, folders: folders)}
    private var blocked: Bool {busy || graph == nil || model.busy || model.pending != nil || generation != model.connectionGeneration}
    private var stale: Bool {graph?["board"]?["revision"] != model.boards.first {$0.text("id") == boardId}?["revision"]}
    private var targetGroups: [JSONValue] {(graph?["nodes"]?.rvArray ?? []).filter {$0.text("id") != id && $0["data"]?.text("kind") == "custom"}}
    var body: some View {
        NavigationStack {Form {
            Section {
                if busy {ProgressView()}
                if let issue {Text(issue).foregroundStyle(Theme.warning)}
                if let issue = resolution.issue {Text(issue).font(.caption).foregroundStyle(Theme.textSecondary)}
                if stale {Text("画布已有其他修改，请重新读取后操作。").foregroundStyle(Theme.warning)}
                TextField("搜索区域会话", text: $search)
                Text("\(resolution.members.count) 个已载入会话" + (resolution.complete ? "" : " · 尚未完整载入")).font(.caption).foregroundStyle(Theme.textSecondary)
                Button("重新读取区域与会话") {Task {await refresh()}}.disabled(busy)
            }
            Section("区域会话") {ForEach(resolution.members.filter {search.isEmpty || $0.label.localizedCaseInsensitiveContains(search)}) {member in
                VStack(alignment: .leading, spacing: 8) {
                    Toggle(member.label, isOn: Binding(get: {selected.contains(member.id)}, set: {checked in if checked {selected.insert(member.id)} else {selected.remove(member.id)}})).disabled(blocked || member.available != true || !selected.contains(member.id) && selected.count >= 200)
                    HStack {
                        Button("打开原会话") {opening = member}.disabled(busy || member.available != true)
                        Spacer()
                        Menu("卡片操作") {
                            Button("拆出卡片") {Task {await detach(member)}}.disabled(blocked || stale || member.available != true)
                            if custom {Button("移出分组") {Task {await remove(member)}}.disabled(blocked || stale)}
                        }
                    }.font(.subheadline)
                }.padding(.vertical, 5)
            }}
            if custom {Section("添加已有会话") {Menu("选择会话") {
                let known = Set(resolution.members.map(\.id))
                ForEach(canvasReferenceChoices(rows).filter {!known.contains($0.id)}) {choice in Button(choice.label) {Task {await mutate(.object(["type": .string("update"), "nodeId": .string(id), "patch": .object(["memberAdd": choice.reference])]))}}}
            }.disabled(blocked || stale)}}
            if cursor != nil {Section {Button("载入更多会话") {Task {await more()}}.disabled(busy)}}
            Section("选中的会话 · \(selected.count)") {
                Picker("目标分组", selection: $target) {Text("新建自定义分组").tag("");ForEach(targetGroups.map(RovaiRow.init(value:))) {row in Text(row.value["data"]?.text("title").isEmpty == false ? row.value["data"]!.text("title") : "自定义分组").tag(row.id)}}
                Button(target.isEmpty ? "归为新分组" : "转移到所选分组") {Task {await transfer()}}.disabled(blocked || stale || selected.isEmpty || graph?["memberTransferVersion"] != .number(1))
                Text("自定义分组会移出原成员；工程、队员等自动区域保留其派生关系。不会移动实际会话或文件。").font(.caption).foregroundStyle(Theme.textSecondary)
            }
        }.navigationTitle(data.text("title").isEmpty ? rovaiCanvasKindNames[data.text("kind")] ?? "区域" : data.text("title")).toolbar {Button("关闭") {dismiss()}.disabled(busy)}}.interactiveDismissDisabled(busy)
        .task(id: model.lastResult) {if generation == nil {generation = model.connectionGeneration};await refresh()}
        .sheet(item: $opening) {member in RovaiCanvasConversationNode(model: model, boardId: boardId, node: activeNode, onOpenThread: {id in dismiss();onOpenThread(id)}, memberReference: member.reference)}
    }
    private func scalar(_ key: String, fallback: Double = 0) -> Double {if case .number(let value) = data[key] {return value};return fallback}
    private func page(reset: Bool) async throws {
        if reset {rows = [];cursor = nil;loaded = false}
        let page = try await readCanvasReferencePage(model, cursor: reset ? nil : cursor)
        guard generation == model.connectionGeneration else {throw CancellationError()}
        guard !page.items.contains(where: {item in rows.contains {$0.text("threadId") == item.text("threadId")}}) else {throw RovaiHostFailure(code: "对话目录分页出现重复，请重新载入")}
        rows += page.items;cursor = page.next;loaded = true
    }
    private func refresh() async {
        guard !busy else {return};busy = true;defer {busy = false}
        do {
            let next = try await model.query(.canvasNodesSnapshot, params: .object(["boardId": .string(boardId)]))
            guard let region = (next["nodes"]?.rvArray ?? []).first(where: {$0.text("id") == id}) else {throw RovaiHostFailure(code: "原区域已删除")}
            let grouping = try await model.query(.navigationProjectGroups, params: .object(["action": .string("get")]))["projectGroups"]?.rvArray ?? []
            let related = try await readCanvasFolderRelations(model, roots: canvasRegionRoots(region, groups: grouping))
            guard generation == model.connectionGeneration else {throw CancellationError()}
            current = region;graph = next;groups = grouping;folders = related;try await page(reset: true);issue = nil
        } catch is CancellationError {} catch {issue = error.localizedDescription}
    }
    private func more() async {guard !busy else {return};busy = true;defer {busy = false};do {try await page(reset: false)} catch {issue = error.localizedDescription}}
    private func mutate(_ mutation: JSONValue) async {
        guard !blocked, !stale, let graph else {return};busy = true;defer {busy = false}
        do {let body = String(decoding: try JSONEncoder().encode(mutation), as: UTF8.self);try model.saveBody(owner, body: body);guard generation == model.connectionGeneration else {throw CancellationError()};await model.executeOwned(.canvasNodesApply, params: .object(["commandId": .string(UUID().uuidString.lowercased()), "command": .object(["boardId": .string(boardId), "expectedRevision": graph["board"]?["revision"] ?? .null, "mutation": mutation])]), owner: owner, body: body)} catch {issue = error.localizedDescription}
    }
    private func detach(_ member: RovaiCanvasRegionMember) async {await mutate(.object(["type": .string("detach"), "regionId": .string(id), "conversation": member.reference, "x": .number(scalar("x") + scalar("width", fallback: 484) + 48), "y": .number(scalar("y"))]))}
    private func remove(_ member: RovaiCanvasRegionMember) async {await mutate(.object(["type": .string("update"), "nodeId": .string(id), "patch": .object(["memberRemove": member.reference])]))}
    private func transfer() async {
        let members = resolution.members.filter {selected.contains($0.id) && $0.available == true}
        guard members.count == selected.count else {issue = "所选会话来源已变化，请重新选择";return}
        let count = (members.count + 1) / 2
        let region: JSONValue = target.isEmpty ? .object(["kind": .string("custom"), "x": .number(scalar("x") - 12), "y": .number(scalar("y") - 52), "width": .number(484), "height": .number(Double(64 + count * 132 + max(0,count-1) * 12))]) : .null
        await mutate(.object(["type": .string("group"), "targetNodeId": target.isEmpty ? .null : .string(target), "newRegion": region, "conversations": .array(members.map(\.reference)), "consumeNodeIds": .array([]), "sourceMembers": .array(members.map {.object(["regionId": .string(id), "conversation": $0.reference])})]))
    }
}
