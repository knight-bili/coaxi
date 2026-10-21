import SwiftUI

struct RovaiCanvasReferenceChoice: Identifiable {
    let reference: JSONValue
    let label: String
    var id: String {let value = JSONValue.array([reference["threadId"] ?? .null, reference["conversationId"] ?? .null]);return (try? JSONEncoder().encode(value)).map {String(decoding: $0, as: UTF8.self)} ?? ""}
}
func canvasReferenceChoices(_ rows: [JSONValue]) -> [RovaiCanvasReferenceChoice] {
    rows.flatMap {thread in
        let threadID = thread.text("threadId"), title = thread.text("title"), members = thread["members"]?.rvArray ?? []
        return [RovaiCanvasReferenceChoice(reference: .object(["threadId": .string(threadID), "conversationId": .null]), label: title + " · 公开对话")] + (thread["privateConversations"]?.rvArray ?? []).map {privateChat in
            let member = members.first {$0.text("agentId") == privateChat.text("agentId")}
            let model = privateChat["lastRunModel"]?.text("modelId") ?? ""
            let runtime = model.isEmpty ? member?.text("runtimeKind") ?? "" : model
            return RovaiCanvasReferenceChoice(reference: .object(["threadId": .string(threadID), "conversationId": privateChat["conversationId"] ?? .null]), label: title + " · " + (member?.text("displayName") ?? "队员") + "私聊" + (runtime.isEmpty ? "" : " · " + runtime))
        }
    }
}

/// Original CanvasNodeEditor/CanvasObjectPicker, bound IDs and original card geometry.
struct RovaiCanvasCreator: View {
    let model: RovaiWorkspaceModel
    let boardId: String
    let graph: JSONValue
    let x: Double, y: Double
    var bindingNode: JSONValue? = nil
    @Environment(\.dismiss) private var dismiss
    @State private var form: [String: JSONValue] = ["kind": .string("note"), "title": .string(""), "content": .string(""), "color": .string(""), "columns": .number(0), "rows": .number(0)]
    @State private var revision = 0, commandId = UUID().uuidString.lowercased()
    @State private var ready = false, busy = false, reading = false, catalogLoaded = false, objectBusy = false, objectReady = false
    @State private var generation: UInt64?, issue: String?
    @State private var references: [JSONValue] = [], groups: [JSONValue] = [], tasks: [JSONValue] = []
    @State private var cursor: RovaiCanvasReferenceCursor?, taskCursor: String?, mission: JSONValue?, diffFiles: [JSONValue]?
    private var owner: String {if let bindingNode {return "canvas-file-binding:\(boardId):\(bindingNode.text("id"))"};return "canvas-create:\(boardId)"}
    private var kind: String {text("kind")}
    private var objectKind: Bool {["task", "mission", "mission_diff"].contains(kind)}
    private var region: Bool {["folder", "group", "agent", "custom"].contains(kind)}
    private var blocked: Bool {!ready || busy || model.busy || model.pending != nil || generation != model.connectionGeneration}
    private var stale: Bool {model.boards.first {$0.text("id") == boardId}?["revision"]?.rvInt != revision}
    private var choices: [RovaiCanvasReferenceChoice] {canvasReferenceChoices(references)}
    private var members: [String] {(form["members"]?.rvArray ?? []).compactMap {if case .string(let value) = $0 {return value};return nil}}
    private var selectedObject: JSONValue {form["objectReference"] ?? .null}
    private var sourceThread: JSONValue? {references.first {$0.text("threadId") == text("objectThreadId")}}
    private var valid: Bool {
        if bindingNode != nil && (kind != "file" || graph["fileReferenceVersion"] != .number(1)) {return false}
        switch kind {
        case "note": return true
        case "custom": return members.count <= 200 && members.allSatisfy {key in choices.contains {$0.id == key}}
        case "folder", "terminal": return !text("path").trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        case "file": return !text("path").isEmpty && references.contains {$0.text("threadId") == text("fileThreadId") && $0.text("projectBindingKind") == "directory"}
        case "group": return groups.contains {$0.text("id") == text("groupId")}
        case "agent": return model.members.contains {$0.text("agentId") == text("agentId") && ($0["removedAt"] == nil || $0["removedAt"] == .null)}
        case "conversation": return choices.contains {$0.id == text("referenceKey")}
        case "task": return objectReady && selectedObject.text("kind") == kind && tasks.contains {$0.text("taskId") == selectedObject.text("taskId")}
        case "mission", "mission_diff": return objectReady && selectedObject.text("kind") == kind && mission != nil && mission?.text("missionId") == selectedObject.text("missionId") && (selectedObject.text("path").isEmpty || diffFiles?.contains {$0.text("path") == selectedObject.text("path")} == true)
        default: return false
        }
    }
    var body: some View {
        NavigationStack {Form {
            Section {Text("画布只保存布局和引用；删除节点不会删除工程、对话或文件。").font(.caption).foregroundStyle(Theme.textSecondary)
                if let issue {Text(issue).foregroundStyle(Theme.warning)}
                if stale {Text("画布已更新，输入仍保留。").foregroundStyle(Theme.warning);Button("核对后使用最新画布版本") {Task {await latestRevision()}}.disabled(blocked)}
                if model.pending != nil {Text("原请求待核对，可关闭后从工作区核对或重试。").font(.caption)}
            }
            Section("属性") {
                if bindingNode == nil {choice("类型", key: "kind", values: ["folder", "group", "agent", "conversation", "custom", "note", "file", "terminal", "mission", "task", "mission_diff"].map {($0, rovaiCanvasKindNames[$0] ?? ["mission": "使命", "task": "任务", "mission_diff": "累计文件变更"][$0] ?? $0)})
                TextField("标题", text: binding("title"))
                Picker("标记颜色", selection: binding("color")) {ForEach(rovaiCanvasColors, id: \.0) {key, label in HStack {Circle().fill(rovaiCanvasTint(key) ?? Theme.textSecondary).frame(width: 12, height: 12).accessibilityHidden(true);Text(label)}.tag(key)}}}
                if kind == "note" {TextField("便签内容（支持Markdown）", text: binding("content"), axis: .vertical).lineLimit(8...24)}
                if ["folder", "file", "terminal"].contains(kind) {if bindingNode != nil {Text(text("path")).font(.caption)} else {TextField(kind == "file" ? "文件绝对路径" : "工作目录绝对路径", text: binding("path")).textInputAutocapitalization(.never).autocorrectionDisabled()}}
                if kind == "agent" {choice("队员", key: "agentId", values: model.members.filter {$0["removedAt"] == nil || $0["removedAt"] == .null}.map {($0.text("agentId"), $0.text("displayName"))})}
                if kind == "group" {choice("项目分组", key: "groupId", values: groups.map {($0.text("id"), $0.text("name"))});Button("重新读取原项目分组") {Task {await loadGroups()}}}
            }.disabled(blocked)
            if kind == "conversation" {Section("对话") {choice("对话", key: "referenceKey", values: choices.map {($0.id, $0.label)})}.disabled(blocked)}
            if kind == "custom" {Section("分组内的对话") {ForEach(choices) {choice in Toggle(choice.label, isOn: Binding(get: {members.contains(choice.id)}, set: {selected in field("members", .array((selected ? members + [choice.id] : members.filter {$0 != choice.id}).map(JSONValue.string)))})).disabled(blocked || !members.contains(choice.id) && members.count >= 200)}}}
            if kind == "file" {Section {choice("所属工程对话", key: "fileThreadId", values: references.filter {$0.text("projectBindingKind") == "directory"}.map {($0.text("threadId"), $0.text("title"))})} footer: {Text("来源绑定后保持不变；实际读取仍由原文件权限检查。")}.disabled(blocked)}
            if objectKind {objectSection}
            if region {Section("区域网格") {Stepper("固定列数 · \(number("columns") == 0 ? "自动" : String(number("columns")))", value: integerBinding("columns"), in: 0...12);Stepper("固定行数 · \(number("rows") == 0 ? "自动" : String(number("rows")))", value: integerBinding("rows"), in: 0...12)}.disabled(blocked)}
            Section {
                if reading || objectBusy {ProgressView()}
                if !catalogLoaded || cursor != nil {Text("对话清单尚未完整载入，当前结果仍在核对。").font(.caption).foregroundStyle(Theme.textSecondary)}
                Button("重新载入对话") {Task {await page(reset: true)}}.disabled(blocked || reading)
                if cursor != nil {Button("载入更多对话") {Task {await page(reset: false)}}.disabled(blocked || reading)}
                Button(bindingNode == nil ? "保存到画布" : "绑定来源") {Task {await submit()}}.disabled(blocked || stale || !valid || objectBusy)
            }
        }.navigationTitle(bindingNode == nil ? "添加到画布" : "绑定文件来源").toolbar {Button("保留草稿并关闭") {close()}.disabled(busy)}}.interactiveDismissDisabled(true)
        .task {await restore()}
        .task(id: "\(kind):\(text("objectThreadId")):\(sourceThread?.text("missionId") ?? "")") {await loadObject()}
        .onChange(of: model.lastResult) {_, result in
            let receipt = result?["commandResult"] ?? result
            guard model.lastInputOwner == owner, receipt?.text("commandId") == commandId else {return}
            if ["applied", "accepted"].contains(receipt?.text("status") ?? "") {ready = false;dismiss()}
            else if receipt?.text("status") == "rejected" {commandId = UUID().uuidString.lowercased();issue = receipt?.text("code");persist()}
        }
    }
    @ViewBuilder private var objectSection: some View {
        Section("原对象") {
            choice("所属对话", key: "objectThreadId", values: references.filter {kind == "task" || !$0.text("missionId").isEmpty}.map {($0.text("threadId"), $0.text("title"))})
            if kind == "task" {
                Picker("任务", selection: Binding(get: {selectedObject.text("taskId")}, set: {field("objectReference", .object(["kind": .string("task"), "taskId": .string($0)]))})) {Text("请选择").tag("");ForEach(tasks, id: \.self) {task in Text(task.text("title")).tag(task.text("taskId"))}}
                if taskCursor != nil {Button("载入更多任务") {Task {await moreTasks()}}}
            }
            if let mission {
                Text(mission.text("title"))
                Button(kind == "mission_diff" ? "引用整个使命的累计变更" : "引用此使命") {field("objectReference", .object(["kind": .string(kind), "missionId": mission["missionId"] ?? .null]))}
                if kind == "mission_diff" {
                    Button("读取可固定的变更文件") {Task {await loadDiff()}}
                    if let diffFiles {Picker("定位文件", selection: Binding(get: {selectedObject.text("path")}, set: {path in var object: [String:JSONValue] = ["kind": .string("mission_diff"), "missionId": mission["missionId"] ?? .null];if !path.isEmpty {object["path"] = .string(path)};field("objectReference", .object(object))})) {Text("整个使命的累计变更").tag("");ForEach(diffFiles.map(RovaiRow.init(value:))) {row in Text(row.value.text("path")).tag(row.value.text("path"))}}}
                }
            }
        }.disabled(blocked || objectBusy)
    }
    private func text(_ key: String) -> String {if case .string(let value) = form[key] {return value};return ""}
    private func number(_ key: String) -> Int {form[key]?.rvInt ?? 0}
    private func field(_ key: String, _ value: JSONValue) {form[key] = value;if key == "kind" || key == "objectThreadId" {form["objectReference"] = .null};persist()}
    private func binding(_ key: String) -> Binding<String> {Binding(get: {text(key)}, set: {field(key, .string($0))})}
    private func integerBinding(_ key: String) -> Binding<Int> {Binding(get: {number(key)}, set: {field(key, .number(Double($0)))})}
    private func choice(_ label: String, key: String, values: [(String,String)]) -> some View {Picker(label, selection: binding(key)) {Text("请选择").tag("");ForEach(values, id: \.0) {key, title in Text(title).tag(key)}}}
    private func document() throws -> String {let value: JSONValue = .object(["version": .number(1), "boardId": .string(boardId), "bindingNodeId": bindingNode?["id"] ?? .null, "form": .object(form), "revision": .number(Double(revision)), "commandId": .string(commandId)]);return String(decoding: try JSONEncoder().encode(value), as: UTF8.self)}
    private func persist() {guard ready, generation == model.connectionGeneration else {return};do {try model.saveBody(owner, body: document())} catch {issue = error.localizedDescription}}
    private func close() {guard !busy else {return};guard ready, generation == model.connectionGeneration else {dismiss();return};do {try model.saveBody(owner, body: document());dismiss()} catch {issue = error.localizedDescription}}
    private func restore() async {
        guard !ready else {return};generation = model.connectionGeneration;revision = graph["board"]?["revision"]?.rvInt ?? 0;form["x"] = .number(x);form["y"] = .number(y);if let bindingNode {form["kind"] = .string("file");form["path"] = bindingNode["data"]?["path"] ?? .null}
        do {let raw = try model.readBody(owner);if !raw.isEmpty {guard let value = JSONValue.parse(raw), value["version"]?.rvInt == 1, value.text("boardId") == boardId, case .object(let saved) = value["form"] else {throw RovaiHostFailure(code: "原创建草稿无法读取，原记录保留")};if let bindingNode {guard value["bindingNodeId"] == bindingNode["id"], saved["path"] == bindingNode["data"]?["path"] else {throw RovaiHostFailure(code: "原文件绑定草稿不属于这张卡片")}};form = saved;revision = value["revision"]?.rvInt ?? 0;if !value.text("commandId").isEmpty {commandId = value.text("commandId")}};ready = true;await loadGroups();await page(reset: true)} catch {issue = error.localizedDescription}
    }
    private func loadGroups() async {do {let value = try await model.query(.navigationProjectGroups, params: .object(["action": .string("get")]));guard generation == model.connectionGeneration else {throw CancellationError()};groups = value["projectGroups"]?.rvArray ?? []} catch is CancellationError {} catch {issue = error.localizedDescription}}
    private func page(reset: Bool) async {
        guard !reading else {return};reading = true;defer {reading = false}
        do {if reset {references = [];cursor = nil;catalogLoaded = false};let page = try await readCanvasReferencePage(model, cursor: reset ? nil : cursor);guard generation == model.connectionGeneration else {throw CancellationError()};guard !page.items.contains(where: {row in references.contains {$0.text("threadId") == row.text("threadId")}}) else {throw RovaiHostFailure(code: "对话目录出现重复页，请重新读取")};references += page.items;cursor = page.next;catalogLoaded = true} catch is CancellationError {} catch {issue = error.localizedDescription}
    }
    private func taskPage(_ after: String?) async throws {
        let value = try await model.query(.tasksList, params: .object(["threadId": .string(text("objectThreadId")), "cursor": after.map(JSONValue.string) ?? .null, "limit": .number(50)]))
        guard generation == model.connectionGeneration else {throw CancellationError()}
        guard case .array(let rows) = value["tasks"], rows.count <= 50, rows.allSatisfy({!$0.text("taskId").isEmpty}) else {throw RovaiHostFailure(code: "任务列表无法读取")}
        let next = value["nextCursor"]?.stringValue
        guard Set(rows.map {$0.text("taskId")}).count == rows.count, !rows.contains(where: {row in tasks.contains {$0.text("taskId") == row.text("taskId")}}), next == nil || next != after && next != "" && !rows.isEmpty, value["truncated"]?.rvBool != true || next != nil else {throw RovaiHostFailure(code: "任务分页未前进")}
        tasks += rows;taskCursor = next
    }
    private func loadObject() async {
        tasks = [];taskCursor = nil;mission = nil;diffFiles = nil;objectReady = false
        guard objectKind, let sourceThread else {return};objectBusy = true;defer {objectBusy = false}
        do {if kind == "task" {try await taskPage(nil)} else {let id = sourceThread.text("missionId");guard !id.isEmpty else {throw RovaiHostFailure(code: "此对话没有使命")};let value = try await model.query(.missionsGet, params: .object(["missionId": .string(id)]));guard value.text("missionId") == id, value.text("threadId") == sourceThread.text("threadId") else {throw RovaiHostFailure(code: "使命归属已变化")};try Task.checkCancellation();mission = value};guard generation == model.connectionGeneration else {throw CancellationError()};objectReady = true} catch is CancellationError {} catch {issue = error.localizedDescription}
    }
    private func moreTasks() async {guard !objectBusy, let taskCursor else {return};objectBusy = true;defer {objectBusy = false};do {try await taskPage(taskCursor)} catch {issue = error.localizedDescription}}
    private func loadDiff() async {
        guard !objectBusy, let mission else {return};objectBusy = true;defer {objectBusy = false}
        do {let value = try await model.query(.missionsChanges, params: .object(["missionId": mission["missionId"] ?? .null]));guard generation == model.connectionGeneration else {throw CancellationError()};guard !value.text("viewId").isEmpty, value.text("diffError").isEmpty, case .array(let files) = value["files"], files.allSatisfy({!$0.text("id").isEmpty && !$0.text("path").isEmpty}) else {throw RovaiHostFailure(code: value.text("diffError").isEmpty ? "当前变更清单暂不可用" : value.text("diffError"))};diffFiles = files} catch is CancellationError {} catch {issue = error.localizedDescription}
    }
    private func latestRevision() async {
        guard !blocked else {return};busy = true;defer {busy = false}
        do {let value = try await model.query(.canvasNodesSnapshot, params: .object(["boardId": .string(boardId)]));guard generation == model.connectionGeneration else {throw CancellationError()};revision = value["board"]?["revision"]?.rvInt ?? 0;commandId = UUID().uuidString.lowercased();persist()} catch {issue = error.localizedDescription}
    }
    private func submit() async {
        guard !blocked, !stale, valid else {return};busy = true;defer {busy = false}
        let columns = min(12,max(0,number("columns"))), rows = min(12,max(0,number("rows")))
        let c = columns == 0 ? 2 : columns, r = rows == 0 ? 1 : rows
        let size = kind == "conversation" || objectKind ? (224,132) : kind == "file" ? (460,360) : kind == "terminal" ? (480,300) : region ? (24+c*224+(c-1)*12,64+r*132+(r-1)*12) : (320,220)
        var node: [String:JSONValue] = ["kind": .string(objectKind ? "conversation" : kind), "title": .string(text("title")), "color": .string(text("color")), "x": form["x"] ?? .number(x), "y": form["y"] ?? .number(y), "width": .number(Double(size.0)), "height": .number(Double(size.1))]
        if region {node["gridColumns"] = .number(Double(columns));node["gridRows"] = .number(Double(rows))}
        switch kind {
        case "note": node["content"] = form["content"] ?? .string("")
        case "folder": node["folderPath"] = form["path"] ?? .null
        case "group": node["folderGroupId"] = form["groupId"] ?? .null
        case "agent": node["agentId"] = form["agentId"] ?? .null
        case "file": node["path"] = form["path"] ?? .null;node["fileReference"] = .object(["threadId": form["fileThreadId"] ?? .null])
        case "terminal": node["path"] = form["path"] ?? .null
        case "conversation": node["conversation"] = choices.first {$0.id == text("referenceKey")}?.reference
        case "custom": node["conversations"] = .array(choices.filter {members.contains($0.id)}.map(\.reference))
        default: if objectKind {node["conversation"] = .object(["threadId": form["objectThreadId"] ?? .null,"conversationId": .null]);node["objectReference"] = selectedObject}
        }
        do {let body = try document();try model.saveBody(owner, body: body);guard generation == model.connectionGeneration else {throw CancellationError()};await model.executeOwned(.canvasNodesApply, params: .object(["commandId": .string(commandId), "command": .object(["boardId": .string(boardId), "expectedRevision": .number(Double(revision)), "mutation": bindingNode.map {JSONValue.object(["type": .string("bind_file"),"nodeId": $0["id"] ?? .null,"threadId": form["fileThreadId"] ?? .null])} ?? .object(["type": .string("create"), "node": .object(node)])])]), owner: owner, body: body)} catch {issue = error.localizedDescription}
    }
}
