import Foundation
import Observation

struct CoaxiCanvasCatalogSnapshot {
    var rows: [JSONValue] = [], groups: [JSONValue] = []
    var folders: [String:CoaxiCanvasFolderRelation] = [:]
    var complete = false
    var issue: String?
}
/// One mounted canvas reads shared metadata; all cards derive from the same bounded pages and folder facts.
@MainActor @Observable final class CoaxiCanvasCatalog {
    private(set) var snapshot = CoaxiCanvasCatalogSnapshot()
    @ObservationIgnored private var scope = ""
    @ObservationIgnored private var epoch: UInt64 = 0
    @ObservationIgnored private var loading = false
    func configure(_ next: String) {guard next != scope else {return};scope = next;epoch &+= 1;snapshot = .init()}
    func load(_ model: CoaxiWorkspaceModel,nodes: [JSONValue]) async {
        guard !loading else {return};loading = true;defer {loading = false}
        let generation = model.connectionGeneration, stamp = epoch
        func current() -> Bool {stamp == epoch && generation == model.connectionGeneration}
        do {
            try Task.checkCancellation();guard current() else {return};snapshot.complete = false;snapshot.issue = nil
            let value = try await model.query(.navigationProjectGroups,params: .object(["action": .string("get")]))
            guard value["schemaVersion"] == .number(1),case .array(let groups) = value["projectGroups"] else {throw CoaxiHostFailure(code: "项目分组目录无法读取")}
            var cursor: CoaxiCanvasReferenceCursor?,rows: [JSONValue] = [],seen = Set<String>()
            repeat {
                try Task.checkCancellation();let page = try await readCanvasReferencePage(model,cursor: cursor);guard current() else {return}
                guard page.items.allSatisfy({seen.insert($0.text("threadId")).inserted}) else {throw CoaxiHostFailure(code: "对话目录分页重复，请重新读取")}
                rows += page.items;cursor = page.next
                var known = snapshot.rows
                for row in rows {if let index = known.firstIndex(where: {$0.text("threadId") == row.text("threadId")}) {known[index] = row} else {known.append(row)}}
                snapshot.rows = known;snapshot.groups = groups;snapshot.complete = false
            } while cursor != nil
            var roots = Set(nodes.flatMap {canvasRegionRoots($0,groups: groups)}),summaryThreads = Set<String>()
            for node in nodes {
                let data = node["data"] ?? .null
                if data.text("kind") == "conversation",data["objectReference"] == nil || data["objectReference"] == .null {if let id = data["conversation"]?["threadId"]?.stringValue {summaryThreads.insert(id)}}
                if data.text("kind") == "custom" {summaryThreads.formUnion((data["conversations"]?.rvArray ?? []).map {$0.text("threadId")})}
                if data.text("kind") == "agent" {summaryThreads.formUnion(rows.filter {thread in (thread["members"]?.rvArray ?? []).contains {(data.text("agentId").isEmpty || $0.text("agentId") == data.text("agentId")) && (data.text("runtimeKind").isEmpty || $0.text("runtimeKind") == data.text("runtimeKind"))}}.map {$0.text("threadId")})}
            }
            roots.formUnion(rows.filter {summaryThreads.contains($0.text("threadId")) && $0.text("projectBindingKind") == "directory"}.map {$0.text("projectPath")})
            let read = try await readCanvasFolderRelations(model,roots: roots.sorted());try Task.checkCancellation();guard current() else {return}
            var folders = read
            for (root,value) in read where !value.complete {if var previous = snapshot.folders[root] {previous = CoaxiCanvasFolderRelation(paths: previous.paths,complete: false,parentPath: previous.parentPath,issue: value.issue,projectPath: previous.projectPath,worktrees: previous.worktrees);folders[root] = previous}}
            snapshot = CoaxiCanvasCatalogSnapshot(rows: rows,groups: groups,folders: folders,complete: true)
        } catch is CancellationError {} catch {if current() {snapshot.complete = false;snapshot.issue = error.localizedDescription}}
    }
}
