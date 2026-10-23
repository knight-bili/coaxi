import SwiftUI

/// CanvasObjectDialog/readCanvasObject: resolve the exact saved object and its public Thread anchor.
struct RovaiCanvasObjectNode: View {
    let model: RovaiWorkspaceModel
    let boardId: String
    let node: JSONValue
    @Environment(\.dismiss) private var dismiss
    @State private var value: JSONValue?
    @State private var taskProjection: JSONValue?
    @State private var issue: String?
    @State private var busy = false
    @State private var workspace = false
    @State private var version: UInt64 = 0
    private var data: JSONValue {node["data"] ?? .null}
    private var reference: JSONValue {data["conversation"] ?? .null}
    private var target: JSONValue {data["objectReference"] ?? .null}
    private var kind: String {target.text("kind")}
    private var threadId: String {reference.text("threadId")}
    private var objectId: String {target.text(kind == "task" ? "taskId" : "missionId")}
    var body: some View {
        NavigationStack {VStack(spacing: 0) {
            if busy {ProgressView().frame(maxWidth: .infinity).padding(8)}
            if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning).padding(12)}
            if value != nil, version != model.resourceVersion {Text("对象可能已变化，可重新读取。").font(.caption).foregroundStyle(Theme.textSecondary).padding(.horizontal, 12)}
            if let value {
                if kind == "mission_diff" {RovaiCanvasMissionChanges(model: model, missionId: objectId, path: target["path"]?.stringValue, revision: version)}
                else {ScrollView {VStack(alignment: .leading, spacing: 16) {
                    Text(value.text("title")).font(.title2.weight(.semibold))
                    MarkdownContent(raw: value.text("description"))
                    Text(kind == "task" ? taskStatusName(value.text("status")) : missionStatusName(value.text("status")))
                    if kind == "task" {ForEach([("blockedReason", "阻塞原因"), ("completionSummary", "完成摘要"), ("cancelReason", "取消原因")], id: \.0) {key, label in if !value.text(key).isEmpty {Text(label).font(.headline);Text(value.text(key))}}}
                }.frame(maxWidth: .infinity, alignment: .leading).padding(16)}}
            } else {Spacer()}
        }.navigationTitle(kind == "task" ? "原任务" : kind == "mission_diff" ? "原使命累计文件变更" : "原使命").navigationBarTitleDisplayMode(.inline).toolbar {
            ToolbarItem(placement: .cancellationAction) {Button("关闭") {dismiss()}}
            ToolbarItemGroup(placement: .confirmationAction) {
                Button("重新读取") {Task {await refresh()}}.disabled(busy)
                if kind != "mission_diff" {Button("在原工作区操作") {Task {await openWorkspace()}}.disabled(busy || value == nil || model.pending != nil)}
            }
        }}.task {await refresh()}
        .sheet(isPresented: $workspace) {if let value {if kind == "task" {RovaiTaskEditor(model: model, threadId: threadId, task: value, projection: taskProjection)} else {RovaiMissionDetail(model: model, initial: value)}}}
    }
    private func read() async throws -> JSONValue {
        let generation = model.connectionGeneration
        guard data.text("kind") == "conversation", ["task", "mission", "mission_diff"].contains(kind), !threadId.isEmpty, reference["conversationId"] == nil || reference["conversationId"] == .null else {throw RovaiHostFailure(code: "对象卡片的公开会话锚点无效")}
        let graph = try await model.query(.canvasNodesSnapshot, params: .object(["boardId": .string(boardId)])), saved = (graph["nodes"]?.rvArray ?? []).first {$0.text("id") == node.text("id") }?["data"]
        guard saved?.text("kind") == "conversation", saved?["conversation"] == reference, saved?["objectReference"] == target else {throw RovaiHostFailure(code: "原卡片已删除或绑定已变化，引用保留")}
        let result: JSONValue
        if kind == "task" {result = try await model.query(.tasksGet, params: .object(["threadId": .string(threadId), "taskId": .string(objectId)]))}
        else {result = try await model.query(.missionsGet, params: .object(["missionId": .string(objectId)]))}
        guard result.text(kind == "task" ? "taskId" : "missionId") == objectId, result.text("threadId") == threadId else {throw RovaiHostFailure(code: "原对象已删除或归属已变化，卡片引用保留")}
        if kind == "mission_diff" {
            let delivery = try await model.query(.missionsDelivery, params: .object(["missionId": .string(objectId)]))
            guard delivery.text("threadId") == threadId else {throw RovaiHostFailure(code: "原累计Diff工作区归属已变化")}
            if let workspace = delivery["workspace"], workspace != .null {guard workspace.text("threadId") == threadId, workspace.text("missionId") == objectId else {throw RovaiHostFailure(code: "原累计Diff工作区归属已变化")}}
        }
        guard generation == model.connectionGeneration else {throw CancellationError()};return result
    }
    private func refresh() async {
        guard !busy else {return};busy = true;defer {busy = false}
        do {value = try await read();version = model.resourceVersion;issue = nil} catch is CancellationError {} catch {value = nil;issue = error.localizedDescription}
    }
    private func openWorkspace() async {
        guard !busy, model.pending == nil else {return};busy = true;defer {busy = false};let generation = model.connectionGeneration
        do {value = try await read();if kind == "task" {let snapshot = try await model.query(.threadsOpen, params: .object(["threadId": .string(threadId), "traceId": .string(UUID().uuidString.lowercased())]));guard snapshot["thread"]?.text("id") == threadId else {throw RovaiHostFailure(code: "原任务会话锚点已不可用")};taskProjection = snapshot};guard generation == model.connectionGeneration else {throw CancellationError()};version = model.resourceVersion;issue = nil;workspace = true} catch is CancellationError {} catch {issue = error.localizedDescription}
    }
}

private struct RovaiCanvasMissionChanges: View {
    let model: RovaiWorkspaceModel
    let missionId: String
    let path: String?
    let revision: UInt64
    @State private var changes: JSONValue?
    @State private var issue: String?
    @State private var patches: [String: String] = [:]
    @State private var busy = false
    @State private var release: (() -> Void)?
    private var files: [JSONValue] {(changes?["files"]?.rvArray ?? []).filter {path == nil || $0.text("path") == path}}
    var body: some View {
        List {
            if busy {ProgressView()}
            if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning)}
            if let changes {
                Section {if !changes.text("diffError").isEmpty {Text(changes.text("diffError")).foregroundStyle(Theme.warning)};Text((changes["checkoutState"]?.text("kind") ?? "") + " · " + (changes["checkoutState"]?.text("branch") ?? "")).font(.caption).foregroundStyle(Theme.textSecondary)}
                ForEach(files, id: \.self) {file in Section {
                    Button(file.text("path")) {Task {busy = true;defer {busy = false};do {try await patch(file, view: changes)} catch {issue = error.localizedDescription}}}.disabled(busy)
                    if file["binary"]?.rvBool == true {Text("二进制文件")}
                    if let patch = patches[file.text("id")] {if patch.isEmpty {Text("没有文本行差异")} else {DiffView(files: UnifiedDiff.parse(patch) ?? [])}}
                }}
                if path == nil, files.isEmpty {Text("当前没有累计文件变更").foregroundStyle(Theme.textSecondary)}
            }
        }.scrollContentBackground(.hidden)
        .onAppear {if release == nil {release = model.retainMissionDiff(missionId)}}
        .onDisappear {release?();release = nil}
        .task(id: revision) {
            busy = true;defer {busy = false};patches = [:];issue = nil;let generation = model.connectionGeneration
            do {let view = try await model.query(.missionsChanges, params: .object(["missionId": .string(missionId)]));guard generation == model.connectionGeneration else {throw CancellationError()};changes = view;if let path {guard let file = (view["files"]?.rvArray ?? []).first(where: {$0.text("path") == path}) else {throw RovaiHostFailure(code: "原固定文件已不在当前累计Diff中，卡片引用保留")};if file["binary"]?.rvBool != true {try await patch(file, view: view)}}} catch is CancellationError {} catch {issue = error.localizedDescription}
        }
    }
    private func patch(_ file: JSONValue, view: JSONValue) async throws {
        let generation = model.connectionGeneration
        let result = try await model.query(.missionsFileDiff, params: .object(["missionId": .string(missionId), "fileId": file["id"] ?? .null, "viewId": view["viewId"] ?? .null]))
        guard generation == model.connectionGeneration else {throw CancellationError()};patches[file.text("id")] = result.text("patch")
    }
}
