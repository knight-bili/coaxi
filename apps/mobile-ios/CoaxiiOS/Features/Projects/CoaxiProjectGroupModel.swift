import Foundation

private let projectGroupPattern = "^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$"
func coaxiProjectPath(_ value: String) -> Bool {
    !value.isEmpty && value.utf16.count<=8192 && !value.contains("\0") && (value.hasPrefix("/") || value.hasPrefix("\\\\") || value.range(of: "^[A-Za-z]:[\\\\/]",options: .regularExpression) != nil) && !value.components(separatedBy: CharacterSet(charactersIn: "/\\")).contains(where: {$0 == "." || $0 == ".."})
}
private func projectGroupStrings(_ value: JSONValue?) throws -> [String] {
    guard case .array(let rows) = value,rows.allSatisfy({$0.stringValue != nil}) else {throw CoaxiHostFailure(code: "项目分组清单无法读取")}
    return rows.compactMap(\.stringValue)
}
private func projectGroup(_ value: JSONValue?) throws -> JSONValue {
    guard let value,case .object(var group) = value,Set(group.keys) == Set(["id","name","color","projectPaths"]),value.text("id").range(of: projectGroupPattern,options: .regularExpression) != nil else {throw CoaxiHostFailure(code: "项目分组身份无效")}
    let name = value.text("name").trimmingCharacters(in: .whitespacesAndNewlines).replacingOccurrences(of: "\\s+",with: " ",options: .regularExpression)
    guard !name.isEmpty,name.unicodeScalars.count<=80 else {throw CoaxiHostFailure(code: "分组名称需为1到80个字符")}
    guard value["color"] == .null || coaxiCanvasColors.dropFirst().contains(where: {value["color"] == .string($0.0)}) else {throw CoaxiHostFailure(code: "项目分组颜色无效")}
    let paths = try projectGroupStrings(value["projectPaths"])
    guard paths.allSatisfy(coaxiProjectPath),Set(paths).count == paths.count else {throw CoaxiHostFailure(code: "项目分组工程路径无效")}
    group["name"] = .string(name);return .object(group)
}
func parseProjectGroups(_ value: JSONValue) throws -> JSONValue {
    guard value["schemaVersion"] == .number(1),case .number(let revision) = value["projectGroupsRevision"],revision.isFinite,revision>=0,revision<=9007199254740991,revision.rounded() == revision,value["initialized"] == .bool(revision>0),case .array(let groups) = value["projectGroups"] else {throw CoaxiHostFailure(code: "项目分组版本无法读取")}
    var ids = Set<String>(),paths = Set<String>()
    for item in groups {let group = try projectGroup(item);guard group == item,ids.insert(group.text("id")).inserted else {throw CoaxiHostFailure(code: "项目分组目录重复或格式不一致")};for path in try projectGroupStrings(group["projectPaths"]) {guard paths.insert(path).inserted else {throw CoaxiHostFailure(code: "项目分组工程重复")}}}
    if value["projectOrder"] != .null {
        let order = try projectGroupStrings(value["projectOrder"])
        guard Set(order).count == order.count,order.allSatisfy({key in if key.hasPrefix("folder-group:") {return ids.contains(String(key.dropFirst(13)))};return key.hasPrefix("directory:") && coaxiProjectPath(String(key.dropFirst(10))) && !paths.contains(String(key.dropFirst(10)))}) else {throw CoaxiHostFailure(code: "项目分组排序无效")}
    }
    return value
}
@MainActor func readProjectGroups(_ model: CoaxiWorkspaceModel) async throws -> JSONValue {
    let generation = model.connectionGeneration,value = try await model.query(.navigationProjectGroups,params: .object(["action": .string("get")]))
    guard generation == model.connectionGeneration else {throw CancellationError()};return try parseProjectGroups(value)
}
/// Original applyNavigationProjectLayout; all unmentioned group identities, membership and order survive.
func projectGroupPlan(_ snapshot: JSONValue,expectedRevision: Int,input: JSONValue,activePaths: [String]) throws -> JSONValue {
    _ = try parseProjectGroups(snapshot)
    guard expectedRevision == snapshot["projectGroupsRevision"]?.rvInt,activePaths.allSatisfy(coaxiProjectPath) else {throw CoaxiHostFailure(code: "项目分组已有新修改，请核对后再保存")}
    var groups = snapshot["projectGroups"]?.rvArray ?? [],order = try snapshot["projectOrder"] == .null ? activePaths.map {"directory:"+$0} : projectGroupStrings(snapshot["projectOrder"])
    let active = Set(activePaths)
    switch input.text("type") {
    case "create","update":
        guard Set(input.rvObject.keys) == Set(["type","group"]) else {throw CoaxiHostFailure(code: "项目分组操作无效")}
        let group = try projectGroup(input["group"]),id = group.text("id"),previous = groups.first {$0.text("id") == id}
        guard input.text("type") == "create" ? previous == nil : previous != nil else {throw CoaxiHostFailure(code: "项目分组身份已变化，请重新读取")}
        let selected = try projectGroupStrings(group["projectPaths"]),previousPaths = try previous.map {try projectGroupStrings($0["projectPaths"])} ?? []
        guard selected.allSatisfy({active.contains($0) || previousPaths.contains($0)}) else {throw CoaxiHostFailure(code: "请选择已登记且未移除的工程")}
        groups = try groups.map {row in if row.text("id") == id {return group};var value = row.rvObject;value["projectPaths"] = .array(try projectGroupStrings(row["projectPaths"]).filter {!selected.contains($0)}.map(JSONValue.string));return .object(value)}
        if previous == nil {groups.append(group);order.append("folder-group:"+id)} else {order += previousPaths.filter {active.contains($0) && !selected.contains($0)}.map {"directory:"+$0}}
    case "delete":
        guard Set(input.rvObject.keys) == Set(["type","groupId"]),let group = groups.first(where: {$0.text("id") == input.text("groupId")}) else {throw CoaxiHostFailure(code: "项目分组身份已变化，请重新读取")}
        let id = group.text("id"),paths = try projectGroupStrings(group["projectPaths"]),members = Set(paths.map {"directory:"+$0})
        groups.removeAll {$0.text("id") == id};order = order.filter {$0 != "folder-group:"+id && !members.contains($0)}+paths.filter {active.contains($0)}.map {"directory:"+$0}
    case "layout":
        guard Set(input.rvObject.keys) == Set(["type","entries"]),case .array(let entries) = input["entries"] else {throw CoaxiHostFailure(code: "项目布局无效")}
        if entries.isEmpty {return .object(["projectGroups": snapshot["projectGroups"] ?? .array([]),"projectOrder": snapshot["projectOrder"] ?? .null])}
        let ids = Set(groups.map {$0.text("id")});var named = Set<String>(),top: [String] = [],members: [String:[String]] = [:]
        for row in entries {
            if row.text("kind") == "group",Set(row.rvObject.keys) == Set(["kind","groupId"]),ids.contains(row.text("groupId")) {let key = "folder-group:"+row.text("groupId");guard named.insert(key).inserted else {throw CoaxiHostFailure(code: "项目布局有重复项")};top.append(key)}
            else if row.text("kind") == "project",Set(row.rvObject.keys) == Set(["kind","projectPath","groupId"]),active.contains(row.text("projectPath")),row["groupId"] == .null || ids.contains(row.text("groupId")) {let path = row.text("projectPath");guard named.insert("directory:"+path).inserted else {throw CoaxiHostFailure(code: "项目布局有重复项")};if row["groupId"] == .null {top.append("directory:"+path)} else {members[row.text("groupId"),default: []].append(path)}}
            else {throw CoaxiHostFailure(code: "项目布局无效")}
        }
        groups = try groups.map {row in var value = row.rvObject;value["projectPaths"] = .array(((members[row.text("id")] ?? [])+(try projectGroupStrings(row["projectPaths"])).filter {!named.contains("directory:"+$0)}).map(JSONValue.string));return .object(value)}
        order = top+order.filter {!named.contains($0)}
    default: throw CoaxiHostFailure(code: "项目分组操作无效")
    }
    let grouped = Set(try groups.flatMap {try projectGroupStrings($0["projectPaths"])}.map {"directory:"+$0});var seen = Set<String>()
    return .object(["projectGroups": .array(groups),"projectOrder": .array(order.filter {!grouped.contains($0) && seen.insert($0).inserted}.map(JSONValue.string))])
}
func projectGroupPendingMatches(_ current: JSONValue,pending: JSONValue) -> Bool {
    let expected = pending["expectedRevision"]?.rvInt ?? -1,revision = current["projectGroupsRevision"]?.rvInt ?? -2,wanted = pending["value"]
    return revision>=expected && revision<=expected+1 && current["projectGroups"] == wanted?["projectGroups"] && current["projectOrder"] == wanted?["projectOrder"]
}
