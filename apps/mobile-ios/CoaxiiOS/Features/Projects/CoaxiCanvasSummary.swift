import Foundation

struct CoaxiCanvasSummary {let title: String;let kind: String;let model: String?;let modelTitle: String?;let project: String?;let branch: String?;let active: Bool?;let available: Bool?}
/// canvasSummaryMetadata: historical evidence/configuration labels stay distinct; no inferred branch or running status.
func canvasSummary(_ reference: JSONValue,catalog: CoaxiCanvasCatalogSnapshot,profiles: [JSONValue],fallbackTitle: String = "") -> CoaxiCanvasSummary {
    let thread = catalog.rows.first {$0.text("threadId") == reference.text("threadId")},privateID = reference["conversationId"]?.stringValue
    let privateChat = (thread?["privateConversations"]?.rvArray ?? []).first {$0.text("conversationId") == privateID}
    let members = (thread?["members"]?.rvArray ?? []).filter {privateID == nil || $0.text("agentId") == privateChat?.text("agentId")}
    struct Model {let name: String;let model: String;let historical: Bool}
    let models: [Model] = members.compactMap {member in
        let historical = (privateID == nil ? member["lastRunModel"] : privateChat?["lastRunModel"])?.text("modelId") ?? ""
        if !historical.isEmpty {return Model(name: member.text("displayName"),model: historical,historical: true)}
        let profile = profiles.first {$0.text("agentId") == member.text("agentId") && $0.text("presence") != "removed" && ($0["removedAt"] == nil || $0["removedAt"] == .null)}
        let config = profile?["runtimeConfiguration"],model = config?["model"]
        guard config?.text("adapterKind") == member.text("runtimeKind"),model?.text("mode") == "explicit",let id = model?["modelId"]?.stringValue,!id.isEmpty else {return nil}
        return Model(name: member.text("displayName"),model: id,historical: false)
    }
    let labels: [String] = [true,false].compactMap {historical in var seen = Set<String>();let names = models.filter {$0.historical == historical}.map(\.model).filter {seen.insert($0).inserted};return names.isEmpty ? nil : (historical ? "最近：" : "配置：")+names.joined(separator: " · ")}
    let title = models.map {($0.historical ? "最近运行模型：" : "模型配置：")+$0.name+": "+$0.model}.joined(separator: "；")
    let project = thread?.text("projectBindingKind") == "directory" ? thread?.text("projectPath") : nil
    var branch: String?
    if let project {
        let exact = catalog.folders[project]
        let candidates: [(CoaxiCanvasFolderRelation,Bool)] = exact.map {[($0,true)]} ?? catalog.folders.values.filter {$0.worktrees.contains {$0.text("projectPath") == project}}.map {($0,false)}
        let branches = Set(candidates.compactMap {relation,isExact -> String? in guard relation.complete else {return nil};let path = isExact ? relation.projectPath : project;let checkout = relation.worktrees.first {$0.text("projectPath") == path};guard checkout?["available"] == .bool(true),checkout?["bare"] == .bool(false),checkout?["prunable"] == .bool(false),checkout?["detached"] != .bool(true),let value = checkout?["branch"]?.stringValue,!value.isEmpty else {return nil};return value})
        if branches.count == 1 {branch = branches.first}
    }
    let active: Bool?
    if privateID != nil,case .bool(let value) = privateChat?["hasActiveRun"] {active = value}
    else if privateID == nil,case .number(let value) = thread?["publicActiveRunCount"] {active = value>0}
    else {active = nil}
    let available: Bool? = thread != nil && (privateID == nil || privateChat != nil) ? true : catalog.complete ? false : nil
    let label = thread.map {($0.text("missionId").isEmpty ? "" : "使命：")+$0.text("title")}
    return CoaxiCanvasSummary(title: label ?? (fallbackTitle.isEmpty ? catalog.complete ? "对话已结束或不可用" : "尚未核对的对话" : fallbackTitle),kind: privateID == nil ? "公开对话" : "私有对话",model: labels.isEmpty ? nil : labels.joined(separator: " · "),modelTitle: title.isEmpty ? nil : title,project: project,branch: branch,active: active,available: available)
}
