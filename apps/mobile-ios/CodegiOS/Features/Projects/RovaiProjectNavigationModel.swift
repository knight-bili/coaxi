import Foundation

struct RovaiProjectNavigationSection: Identifiable {
    let id: String
    let name: String
    let color: String?
    let groupId: String?
    let projects: [JSONValue]
}

/// Port of resolveSidebarProjectGroups; a checked main worktree owns its linked checkouts.
func projectNavigationSections(_ snapshot: JSONValue,projects: [JSONValue],folders: [String:RovaiCanvasFolderRelation]) -> [RovaiProjectNavigationSection] {
    let groups = snapshot["projectGroups"]?.rvArray ?? []
    func paths(_ group: JSONValue) -> [String] {(group["projectPaths"]?.rvArray ?? []).compactMap(\.stringValue)}
    var owners: [String:String] = [:]
    for group in groups {for root in paths(group) where folders[root]?.parentPath == nil {owners[root] = group.text("id")}}
    for group in groups {for root in paths(group) where folders[root]?.parentPath == nil {for path in folders[root]?.paths ?? [] where path != root {owners[path] = group.text("id")}}}
    let byPath = projects.reduce(into: [String:JSONValue]()) {$0[$1.text("projectPath")] = $1}
    let grouped = groups.map {group in
        let id = group.text("id");var seen = Set<String>()
        let expanded = paths(group).flatMap {folders[$0]?.paths ?? [$0]}.filter {seen.insert($0).inserted && owners[$0] == id}
        return RovaiProjectNavigationSection(id: "folder-group:\(id)",name: group.text("name"),color: group["color"]?.stringValue,groupId: id,projects: expanded.compactMap {byPath[$0]})
    }
    let loose = projects.filter {owners[$0.text("projectPath")] == nil}.map {RovaiProjectNavigationSection(id: "directory:\($0.text("projectPath"))",name: $0.text("name"),color: nil,groupId: nil,projects: [$0])}
    let order = (snapshot["projectOrder"]?.rvArray ?? []).compactMap(\.stringValue)
    return (loose+grouped).enumerated().sorted {a,b in
        let left = order.firstIndex(of: a.element.id) ?? Int.max,right = order.firstIndex(of: b.element.id) ?? Int.max
        return left == right ? a.offset<b.offset : left<right
    }.map(\.element)
}
