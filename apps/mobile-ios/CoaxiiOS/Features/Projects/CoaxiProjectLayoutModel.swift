import Foundation

struct CoaxiProjectLayoutItem: Hashable {let kind: String;let target: String;var key: String {(kind == "group" ? "folder-group:" : "directory:")+target}}
struct CoaxiProjectLayoutLocation: Equatable {let groupId: String?;let index: Int}
struct CoaxiProjectLayout: Equatable {
    var top: [CoaxiProjectLayoutItem]
    var members: [String:[String]]
    func value() -> JSONValue {.object(["top": .array(top.map {.object(["kind": .string($0.kind),$0.kind == "group" ? "groupId" : "projectPath": .string($0.target)])}),"members": .object(members.mapValues {.array($0.map(JSONValue.string))})])}
    func entries() -> [JSONValue] {top.flatMap {item -> [JSONValue] in if item.kind == "project" {return [.object(["kind": .string("project"),"projectPath": .string(item.target),"groupId": .null])]};return [.object(["kind": .string("group"),"groupId": .string(item.target)])]+(members[item.target] ?? []).map {.object(["kind": .string("project"),"projectPath": .string($0),"groupId": .string(item.target)])}}}
    func locate(_ item: CoaxiProjectLayoutItem) -> CoaxiProjectLayoutLocation? {
        if let index = top.firstIndex(of: item) {return .init(groupId: nil,index: index)}
        if item.kind == "project" {for (id,paths) in members {if let index = paths.firstIndex(of: item.target) {return .init(groupId: id,index: index)}}}
        return nil
    }
    /// Indices are measured after removal; unknown targets and nested groups never silently retarget.
    func move(_ item: CoaxiProjectLayoutItem,to target: CoaxiProjectLayoutLocation) -> CoaxiProjectLayout {
        guard let source = locate(item),target.index>=0,item.kind != "group" || target.groupId == nil,target.groupId == nil || members[target.groupId!] != nil else {return self}
        var next = self
        if let id = source.groupId {next.members[id]?.remove(at: source.index)} else {next.top.remove(at: source.index)}
        if let id = target.groupId {let index = min(target.index,next.members[id]?.count ?? 0);next.members[id]?.insert(item.target,at: index)} else {next.top.insert(item,at: min(target.index,next.top.count))}
        return next
    }
}
/// Original createProjectLayout; checked linked worktrees remain attached to their main project.
func createProjectLayout(_ snapshot: JSONValue,active: [String],folders: [String:CoaxiCanvasFolderRelation]) -> CoaxiProjectLayout {
    var seen = Set<String>()
    let projects = active.filter {seen.insert($0).inserted && folders[$0]?.parentPath == nil},groups = snapshot["projectGroups"]?.rvArray ?? []
    let grouped = Set(groups.flatMap {($0["projectPaths"]?.rvArray ?? []).compactMap(\.stringValue)})
    let order = (snapshot["projectOrder"]?.rvArray ?? []).compactMap(\.stringValue)
    let all = projects.filter {!grouped.contains($0)}.map {CoaxiProjectLayoutItem(kind: "project",target: $0)}+groups.map {CoaxiProjectLayoutItem(kind: "group",target: $0.text("id"))}
    let top = all.enumerated().sorted {a,b in let left = order.firstIndex(of: a.element.key) ?? Int.max,right = order.firstIndex(of: b.element.key) ?? Int.max;return left == right ? a.offset<b.offset : left<right}.map(\.element)
    let members = groups.reduce(into: [String:[String]]()) {$0[$1.text("id")] = ($1["projectPaths"]?.rvArray ?? []).compactMap(\.stringValue).filter {projects.contains($0)}}
    return .init(top: top,members: members)
}
func parseProjectLayout(_ value: JSONValue) throws -> CoaxiProjectLayout {
    guard Set(value.rvObject.keys) == Set(["top","members"]),case .array(let rows) = value["top"],case .object(let rawMembers) = value["members"] else {throw CoaxiHostFailure(code: "原项目排版草稿无法读取")}
    let top: [CoaxiProjectLayoutItem] = try rows.map {row in let kind = row.text("kind"),key = kind == "group" ? "groupId" : "projectPath";guard ["group","project"].contains(kind),Set(row.rvObject.keys) == Set(["kind",key]),let target = row[key]?.stringValue else {throw CoaxiHostFailure(code: "原项目排版项目无效")};return .init(kind: kind,target: target)}
    let members: [String:[String]] = try rawMembers.mapValues {value in guard case .array(let rows) = value,rows.allSatisfy({$0.stringValue != nil}) else {throw CoaxiHostFailure(code: "原分组成员排版无效")};return rows.compactMap(\.stringValue)}
    let groupIds = top.filter {$0.kind == "group"}.map(\.target),paths = top.filter {$0.kind == "project"}.map(\.target)+members.values.flatMap {$0}
    guard Set(top.map(\.key)).count == top.count,Set(members.keys) == Set(groupIds),Set(groupIds).count == groupIds.count,paths.allSatisfy(coaxiProjectPath),Set(paths).count == paths.count else {throw CoaxiHostFailure(code: "原项目排版包含重复或未知项目")}
    return .init(top: top,members: members)
}
func projectLayoutRelationIssue(_ base: CoaxiProjectLayout,next: CoaxiProjectLayout,folders: [String:CoaxiCanvasFolderRelation]) -> Bool {
    next.entries().filter {$0.text("kind") == "project"}.contains {entry in let path = entry.text("projectPath"),item = CoaxiProjectLayoutItem(kind: "project",target: path),relation = folders[path];return relation?.parentPath != nil || base.locate(item) != next.locate(item) && relation?.complete != true}
}
