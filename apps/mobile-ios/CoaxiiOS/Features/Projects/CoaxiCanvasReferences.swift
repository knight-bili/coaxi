import Foundation

struct CoaxiCanvasReferenceCursor: Hashable {let after: String;let upper: String;let minimumSequence: Int}
struct CoaxiCanvasReferencePage {let items: [JSONValue];let next: CoaxiCanvasReferenceCursor?;let sequence: Int}

/// canvas-reference-controller.ts: one frozen upper identity, bounded metadata pages, no transcript/draft reads.
@MainActor func readCanvasReferencePage(_ model: CoaxiWorkspaceModel, cursor: CoaxiCanvasReferenceCursor? = nil) async throws -> CoaxiCanvasReferencePage {
    let generation = model.connectionGeneration
    let value = try await model.query(.canvasReferencesPage, params: .object(["afterThreadId": cursor.map {JSONValue.string($0.after)} ?? .null, "upperThreadId": cursor.map {JSONValue.string($0.upper)} ?? .null, "limit": .number(200)]))
    guard generation == model.connectionGeneration else {throw CancellationError()}
    guard value["schemaVersion"] == .number(1), case .number(let sequenceNumber) = value["throughGlobalSequence"], sequenceNumber.isFinite, sequenceNumber.rounded() == sequenceNumber, sequenceNumber >= 0, sequenceNumber <= 9007199254740991, sequenceNumber >= Double(cursor?.minimumSequence ?? 0), case .array(let items) = value["items"], items.count <= 200 else {throw CoaxiHostFailure(code: "对话目录版本或列表格式不一致，请重新读取")}
    let upper = value["upperThreadId"]?.stringValue, next = value["nextAfterThreadId"]?.stringValue, sequence = Int(sequenceNumber)
    guard upper == nil || !(upper?.isEmpty ?? true), cursor == nil || upper == cursor?.upper else {throw CoaxiHostFailure(code: "对话目录分页范围已变化")}
    func validModel(_ value: JSONValue?) -> Bool {
        guard let value, value != .null else {return true}
        return !value.text("runId").trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !value.text("adapterKind").trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && (value["modelId"] == .null || !(value["modelId"]?.stringValue?.isEmpty ?? true)) && (value["reasoningEffort"] == .null || value["reasoningEffort"]?.stringValue != nil)
    }
    var previous = cursor?.after ?? ""
    for item in items {
        let id = item.text("threadId")
        guard id > previous, let upper, id <= upper, item["title"]?.stringValue != nil, item["projectPath"]?.stringValue != nil, case .array(let members) = item["members"], case .array(let privateRows) = item["privateConversations"] else {throw CoaxiHostFailure(code: "对话目录分页未前进或归属不一致")}
        let memberIds = Set(members.map {$0.text("agentId")})
        guard !memberIds.contains(""), members.allSatisfy({validModel($0["lastRunModel"])}) else {throw CoaxiHostFailure(code: "对话成员或模型来源无法读取")}
        guard privateRows.allSatisfy({!$0.text("conversationId").isEmpty && memberIds.contains($0.text("agentId")) && ($0["hasActiveRun"] == nil || $0["hasActiveRun"] == .bool(true) || $0["hasActiveRun"] == .bool(false)) && validModel($0["lastRunModel"])}) else {throw CoaxiHostFailure(code: "私聊与成员归属不一致")}
        if let count = item["publicActiveRunCount"] {guard case .number(let number) = count, number.isFinite, number >= 0, number.rounded() == number, number <= 9007199254740991 else {throw CoaxiHostFailure(code: "公开会话运行数量无效")}}
        previous = id
    }
    guard next == nil || !items.isEmpty && next == previous && next != cursor?.after else {throw CoaxiHostFailure(code: "对话目录分页未前进")}
    if let next {guard let upper else {throw CoaxiHostFailure(code: "对话目录范围缺失")};return CoaxiCanvasReferencePage(items: items, next: .init(after: next, upper: upper, minimumSequence: sequence), sequence: sequence)}
    return CoaxiCanvasReferencePage(items: items, next: nil, sequence: sequence)
}
