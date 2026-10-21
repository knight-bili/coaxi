import SwiftUI

/// Original selection dock. Business-object pins retain their original kind and cannot become conversation members.
struct RovaiCanvasSelectionActions: View {
    let model: RovaiWorkspaceModel
    let boardId: String
    let graph: JSONValue
    let nodeIds: Set<String>
    @Environment(\.dismiss) private var dismiss
    @State private var target = "", busy = false
    @State private var generation: UInt64?, issue: String?, sentId: String?, confirmation: JSONValue?
    private var owner: String {"canvas-bulk:\(boardId)"}
    private var nodes: [JSONValue] {(graph["nodes"]?.rvArray ?? []).filter {nodeIds.contains($0.text("id"))}}
    private var stale: Bool {model.boards.first {$0.text("id") == boardId}?["revision"] != graph["board"]?["revision"]}
    private var blocked: Bool {busy || model.busy || model.pending != nil || generation != model.connectionGeneration || nodes.count != nodeIds.count}
    private var groupable: Bool {!nodes.isEmpty && nodes.count <= 200 && nodes.allSatisfy {$0["data"]?.text("kind") == "conversation" && $0["data"]?["conversation"]?.isObject == true && ($0["data"]?["objectReference"] == nil || $0["data"]?["objectReference"] == .null)}}
    var body: some View {
        NavigationStack {Form {
            Section {
                if let issue {Text(issue).foregroundStyle(Theme.warning)}
                if stale {Text("画布已更新；关闭后核对最新卡片，再重新操作。").foregroundStyle(Theme.warning)}
                if model.pending != nil {Text("原请求待核对，当前选择保留。").font(.caption)}
                ForEach(nodes.map(RovaiRow.init(value:))) {row in Text(row.value["data"]?.text("title").isEmpty == false ? row.value["data"]!.text("title") : rovaiCanvasKindNames[row.value["data"]?.text("kind") ?? ""] ?? "卡片")}
            }
            Section("归为分组") {
                Picker("目标分组",selection: $target) {Text("新建自定义分组").tag("");ForEach((graph["nodes"]?.rvArray ?? []).filter {$0["data"]?.text("kind") == "custom"}.map(RovaiRow.init(value:))) {row in Text(row.value["data"]?.text("title").isEmpty == false ? row.value["data"]!.text("title") : "自定义分组").tag(row.id)}}
                Button(target.isEmpty ? "归为新分组" : "归入已有分组") {Task {await group()}}.disabled(blocked || stale || !groupable)
                if !groupable {Text("归组请选择最多200张普通会话卡；使命、任务、文件、终端和便签保留各自的对象类型。").font(.caption).foregroundStyle(Theme.textSecondary)}
            }
            Section {
                Button("整理所选卡片") {confirmation = .object(["type": .string("move"),"moves": .array(canvasPackedMoves(nodes))])}.disabled(blocked || stale || nodes.isEmpty)
                Button("删除所选卡片",role: .destructive) {confirmation = .object(["type": .string("delete"),"nodeIds": .array(nodes.map {$0["id"] ?? .null})])}.disabled(blocked || stale || nodes.isEmpty)
            }
        }.navigationTitle("已选 \(nodes.count) 张卡片").toolbar {Button("关闭") {dismiss()}.disabled(busy)}}.interactiveDismissDisabled(busy)
        .task {generation = model.connectionGeneration}
        .onChange(of: model.lastResult) {_, result in let receipt = result?["commandResult"] ?? result;guard model.lastInputOwner == owner,receipt?.text("commandId") == sentId else {return};if ["applied","accepted"].contains(receipt?.text("status") ?? "") {dismiss()} else if receipt?.text("status") == "rejected" {issue = receipt?.text("code")}}
        .confirmationDialog(confirmation?.text("type") == "delete" ? "删除所选卡片？" : "整理所选卡片？",isPresented: Binding(get: {confirmation != nil},set: {if !$0 {confirmation = nil}}),titleVisibility: .visible) {
            Button(confirmation?.text("type") == "delete" ? "删除卡片" : "整理位置",role: confirmation?.text("type") == "delete" ? .destructive : nil) {if let confirmation {Task {await submit(confirmation)}};confirmation = nil}.disabled(blocked || stale)
            Button("取消",role: .cancel) {confirmation = nil}
        } message: {Text(confirmation?.text("type") == "delete" ? "仅移除画布引用，原会话、使命、任务和文件保留。所选便签的正文会删除，所选运行中终端会关闭。" : "按原版排布规则调整所选卡片位置，其他卡片位置保留。")}
    }
    private func submit(_ mutation: JSONValue) async {
        guard !blocked,!stale else {return};busy = true;defer {busy = false};let commandId = UUID().uuidString.lowercased();sentId = commandId
        do {let body = String(decoding: try JSONEncoder().encode(mutation),as: UTF8.self);try model.saveBody(owner,body: body);guard generation == model.connectionGeneration else {throw CancellationError()};await model.executeOwned(.canvasNodesApply,params: .object(["commandId": .string(commandId),"command": .object(["boardId": .string(boardId),"expectedRevision": graph["board"]?["revision"] ?? .null,"mutation": mutation])]),owner: owner,body: body)} catch {issue = error.localizedDescription}
    }
    private func group() async {
        guard groupable else {return}
        var seen = Set<String>(), references: [JSONValue] = []
        for node in nodes {let reference = node["data"]?["conversation"] ?? .null;if seen.insert(RovaiCanvasReferenceChoice(reference: reference,label: "").id).inserted {references.append(reference)}}
        func scalar(_ node: JSONValue,_ key: String) -> Double {if case .number(let value) = node["data"]?[key] {return value};return 0}
        let count = (references.count+1)/2
        let region: JSONValue = target.isEmpty ? .object(["kind": .string("custom"),"x": .number((nodes.map {scalar($0,"x")}.min() ?? 0)-12),"y": .number((nodes.map {scalar($0,"y")}.min() ?? 0)-52),"width": .number(484),"height": .number(Double(64+count*132+max(0,count-1)*12))]) : .null
        await submit(.object(["type": .string("group"),"targetNodeId": target.isEmpty ? .null : .string(target),"newRegion": region,"conversations": .array(references),"consumeNodeIds": .array(nodes.map {$0["id"] ?? .null})]))
    }
}
