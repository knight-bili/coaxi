import Foundation

struct RovaiCanvasFolderRelation {let paths: [String];let complete: Bool;var parentPath: String? = nil;var issue: String? = nil;var projectPath: String? = nil;var worktrees: [JSONValue] = []}
struct RovaiCanvasRegionMember: Identifiable {
    let reference: JSONValue
    let label: String
    let available: Bool?
    var updatedAt = ""
    var id: String {RovaiCanvasReferenceChoice(reference: reference, label: label).id}
}
struct RovaiCanvasRegionResolution {let members: [RovaiCanvasRegionMember];let complete: Bool;var issue: String? = nil}
func canvasRegionRoots(_ node: JSONValue, groups: [JSONValue]) -> [String] {
    let data = node["data"] ?? .null
    if data.text("kind") == "folder" {return data.text("folderPath").isEmpty ? [] : [data.text("folderPath")]}
    if data.text("kind") == "group" {return (groups.first {$0.text("id") == data.text("folderGroupId") }?["projectPaths"]?.rvArray ?? []).compactMap(\.stringValue)}
    return []
}
/// canvasFolderResolution: exact root facts; linked checkouts and subdirectories never become parent regions.
@MainActor func readCanvasFolderRelations(_ model: RovaiWorkspaceModel, roots: [String]) async throws -> [String:RovaiCanvasFolderRelation] {
    let generation = model.connectionGeneration
    var result: [String:RovaiCanvasFolderRelation] = [:]
    for root in roots where result[root] == nil {
        try Task.checkCancellation()
        do {
            let value = try await model.query(.workspacesWorktrees, params: .object(["path": .string(root)]))
            guard value["schemaVersion"] == .number(1), value.text("requestedPath") == root, !value.text("projectPath").isEmpty, case .array(let rows) = value["worktrees"], rows.count <= 4096 else {throw RovaiHostFailure(code: "工程工作树关系不一致")}
            let repository = value["repositoryRoot"]?.stringValue, main = value["mainWorktreePath"]?.stringValue, project = value.text("projectPath")
            var seen = Set<String>()
            for (index, row) in rows.enumerated() {
                guard !row.text("projectPath").isEmpty, seen.insert(row.text("projectPath")).inserted, ["available", "bare", "locked", "prunable"].allSatisfy({row[$0] == .bool(true) || row[$0] == .bool(false)}), row["isMain"] == .bool(index == 0), row["branch"] == nil || row["branch"] == .null || row["branch"]?.stringValue?.isEmpty == false, row["detached"] != .bool(true) || row["branch"] == nil || row["branch"] == .null else {throw RovaiHostFailure(code: "工程工作树关系不一致")}
            }
            if let repository {guard rows.contains(where: {$0.text("projectPath") == repository && $0["available"] == .bool(true) && $0["bare"] == .bool(false)}), main == (rows.first?["bare"] == .bool(true) ? nil : rows.first?.text("projectPath")) else {throw RovaiHostFailure(code: "工程工作树关系不一致")}}
            else {guard rows.isEmpty, main == nil else {throw RovaiHostFailure(code: "工程工作树关系不一致")}}
            guard generation == model.connectionGeneration else {throw CancellationError()}
            var paths = [root, project]
            if project == repository, repository == main {paths += rows.filter {$0["bare"] == .bool(false)}.map {$0.text("projectPath")}}
            result[root] = RovaiCanvasFolderRelation(paths: Array(Set(paths)), complete: true, parentPath: project == repository && main != nil && main != repository ? main : nil,projectPath: project,worktrees: rows)
        } catch is CancellationError {throw CancellationError()} catch {result[root] = RovaiCanvasFolderRelation(paths: [root], complete: false, issue: "工作树关系读取失败，已保留已知对话。")}
    }
    return result
}
func resolveCanvasRegion(_ node: JSONValue, rows: [JSONValue], catalogComplete: Bool, groups: [JSONValue], folders: [String:RovaiCanvasFolderRelation]) -> RovaiCanvasRegionResolution {
    let data = node["data"] ?? .null, kind = data.text("kind")
    var complete = catalogComplete, issue: String? = catalogComplete ? nil : "对话清单读取未完成"
    func options(_ thread: JSONValue, memberIDs: Set<String>? = nil) -> [RovaiCanvasRegionMember] {
        let privateRows = thread["privateConversations"]?.rvArray ?? []
        return canvasReferenceChoices([thread]).compactMap {choice in
            let privateID = choice.reference["conversationId"]?.stringValue
            let privateChat = privateRows.first {$0.text("conversationId") == privateID}
            if privateID != nil, let memberIDs, !memberIDs.contains(privateChat?.text("agentId") ?? "") {return nil}
            return RovaiCanvasRegionMember(reference: choice.reference, label: choice.label, available: true, updatedAt: privateID == nil ? thread.text("updatedAt") : privateChat?.text("updatedAt") ?? "")
        }
    }
    func ordered(_ rows: [RovaiCanvasRegionMember]) -> [RovaiCanvasRegionMember] {
        let fractional = ISO8601DateFormatter();fractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        let ordinary = ISO8601DateFormatter()
        func time(_ value: String) -> Double {(fractional.date(from: value) ?? ordinary.date(from: value))?.timeIntervalSince1970 ?? 0}
        return rows.sorted {a, b in let left = time(a.updatedAt), right = time(b.updatedAt);return left == right ? a.id < b.id : left > right}
    }
    if kind == "custom" {
        let known = rows.flatMap {options($0)}
        let selected = (data["conversations"]?.rvArray ?? []).map {reference in let key = RovaiCanvasReferenceChoice(reference: reference, label: "").id;return known.first {$0.id == key} ?? RovaiCanvasRegionMember(reference: reference, label: catalogComplete ? "对话已结束或不可用" : "尚未核对的对话", available: catalogComplete ? false : nil)}
        return RovaiCanvasRegionResolution(members: ordered(selected), complete: complete, issue: issue)
    }
    if kind == "agent" {
        let selected = rows.flatMap {thread -> [RovaiCanvasRegionMember] in
            let ids = Set((thread["members"]?.rvArray ?? []).filter {(data.text("agentId").isEmpty || $0.text("agentId") == data.text("agentId")) && (data.text("runtimeKind").isEmpty || $0.text("runtimeKind") == data.text("runtimeKind"))}.map {$0.text("agentId")})
            return ids.isEmpty ? [] : options(thread, memberIDs: ids)
        }
        return RovaiCanvasRegionResolution(members: ordered(selected), complete: complete, issue: issue)
    }
    guard ["folder", "group"].contains(kind) else {return RovaiCanvasRegionResolution(members: [], complete: false, issue: "此卡片不是区域")}
    if kind == "group", !groups.contains(where: {$0.text("id") == data.text("folderGroupId")}) {return RovaiCanvasRegionResolution(members: [], complete: false, issue: "项目分组在当前工作区不可用")}
    var paths = Set<String>()
    for root in canvasRegionRoots(node, groups: groups) {
        let relation = folders[root]
        if relation?.complete != true {complete = false;issue = relation?.issue ?? "工作树关系尚未核对"}
        if kind != "group" || relation?.parentPath == nil {paths.formUnion(relation?.paths ?? [root])}
    }
    return RovaiCanvasRegionResolution(members: ordered(rows.filter {$0.text("projectBindingKind") == "directory" && paths.contains($0.text("projectPath"))}.flatMap {options($0)}), complete: complete, issue: issue)
}
/// Re-resolve a derived card from its current region before opening the saved public/private conversation.
@MainActor func requireCanvasRegionReference(_ model: RovaiWorkspaceModel, boardId: String, nodeId: String, reference: JSONValue) async throws {
    let generation = model.connectionGeneration
    let graph = try await model.query(.canvasNodesSnapshot, params: .object(["boardId": .string(boardId)]))
    guard let node = (graph["nodes"]?.rvArray ?? []).first(where: {$0.text("id") == nodeId}) else {throw RovaiHostFailure(code: "原区域已删除")}
    let groups = try await model.query(.navigationProjectGroups, params: .object(["action": .string("get")]))["projectGroups"]?.rvArray ?? []
    let folders = try await readCanvasFolderRelations(model, roots: canvasRegionRoots(node, groups: groups))
    let key = RovaiCanvasReferenceChoice(reference: reference, label: "").id
    var cursor: RovaiCanvasReferenceCursor?
    repeat {
        try Task.checkCancellation()
        let page = try await readCanvasReferencePage(model, cursor: cursor)
        if let thread = page.items.first(where: {$0.text("threadId") == reference.text("threadId")}) {
            guard resolveCanvasRegion(node, rows: [thread], catalogComplete: false, groups: groups, folders: folders).members.contains(where: {$0.id == key && $0.available == true}), generation == model.connectionGeneration else {throw RovaiHostFailure(code: "原区域与会话关系已变化")}
            return
        }
        cursor = page.next
    } while cursor != nil
    throw RovaiHostFailure(code: "原区域会话已结束或不可用，引用保留")
}
