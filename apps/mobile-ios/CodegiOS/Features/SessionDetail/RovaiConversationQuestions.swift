import SwiftUI

func conversationScope(_ thread: String, _ conversation: String?, _ child: Bool) -> JSONValue {
    .object(["threadId": .string(thread), "conversationId": conversation.map(JSONValue.string) ?? .null, "scopeKind": .string(conversation == nil ? "public" : child ? "delegation_child" : "single_chat")])
}
private func conversationQuestionStatus(_ value: String) -> String {
    switch value {case "awaiting_answer": "等待你回答";case "answered": "答案已保存，等待原连接确认";case "delivered", "applied": "已写回原队员连接";case "cancelled": "原问题已停止，记录保留";case "outcome_unknown": "原连接结果待核对，不自动重发";default: value}
}
private struct ConversationQuestionRow: Identifiable {let value: JSONValue;var id: String {value.text("requestId")}}
/// 原会话容器中的原Codeg问题卡；消息草稿与回答分别保存。
struct RovaiConversationQuestions: View {
    let model: RovaiWorkspaceModel;let threadId: String;var conversationId: String? = nil;var child = false
    @State private var rows: [JSONValue] = [], next: String?, issue: String?, generation: UInt64?
    @State private var history = false
    private var scope: JSONValue {conversationScope(threadId, conversationId, child)}
    private var active: [JSONValue] {rows.filter {["awaiting_answer", "answered", "outcome_unknown"].contains($0.text("state"))}}
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            ForEach(active.map {ConversationQuestionRow(value: $0)}) {row in RovaiConversationQuestion(model: model, value: row.value) {Task {await refresh()}}}
            if rows.count > active.count {DisclosureGroup("已结束的问题记录", isExpanded: $history) {if history {ForEach(rows.filter {!active.contains($0)}.map {ConversationQuestionRow(value: $0)}) {row in RovaiConversationQuestion(model: model, value: row.value) {Task {await refresh()}}}}}}
            if next != nil {Button("更多问题记录") {Task {await refresh(more: true)}}}
            if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning)}
        }.task(id: scope) {generation = model.connectionGeneration;rows = [];next = nil;await refresh()}
            .task(id: model.resourceVersion) {do {try await Task.sleep(for: .milliseconds(250));try Task.checkCancellation();await refresh()} catch {}}
    }
    private func refresh(more: Bool = false) async {
        let observed = model.connectionGeneration, selected = scope
        do {let value = try await nativeUserCall(model, .conversationToolsQuestionsList, .object(["scope": selected, "after": more ? next.map(JSONValue.string) ?? .null : .null, "limit": .number(50)]));guard observed == model.connectionGeneration, selected == scope else {return};let items = value["items"]?.rvArray ?? [];rows = more ? rows + items.filter {item in !rows.contains {$0.text("requestId") == item.text("requestId")}} : items;next = value["next"]?.stringValue;issue = nil} catch is CancellationError {} catch {if observed == model.connectionGeneration {issue = error.localizedDescription}}
    }
}
private struct RovaiConversationQuestion: View {
    let model: RovaiWorkspaceModel;let value: JSONValue;let onChanged: () -> Void
    @State private var original: JSONValue?, result: JSONValue?, issue: String?, generation: UInt64?
    @State private var ready = false, busy = false
    private var id: String {value.text("requestId")}
    private var owner: String {"conversation-question:" + id}
    private var state: String {["delivered", "cancelled", "outcome_unknown"].contains(value.text("state")) ? value.text("state") : result?.text("state").isEmpty == false ? result!.text("state") : value.text("state")}
    private var pending: PendingQuestion? {
        do {let questions = try (value["questions"]?.rvArray ?? []).map {question -> QuestionSpec in var fields = question["spec"]?.rvObject ?? [:];fields["id"] = question["id"] ?? .null;return try JSONDecoder().decode(QuestionSpec.self, from: JSONEncoder().encode(JSONValue.object(fields)))};return PendingQuestion(questionId: id, questions: questions)} catch {return nil}
    }
    private var selections: [String: AskQuestionSelection] {
        let outcomes = result?["outcome"]?["answers"]?.rvArray ?? []
        return Dictionary(uniqueKeysWithValues: (pending?.questions ?? []).enumerated().map {index, question in let selected = outcomes.indices.contains(index) ? outcomes[index]["selected"]?.rvArray.compactMap(\.stringValue) ?? [] : [];let known = Set(question.options.map(\.label));return (question.id, AskQuestionSelection(chosen: selected.filter {known.contains($0)}, otherText: selected.filter {!known.contains($0)}.joined(separator: "\n")))})
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text((model.members.first {$0.text("agentId") == value.text("agentId")}?.text("displayName") ?? "原队员") + " · " + conversationQuestionStatus(state)).font(.caption).foregroundStyle(Theme.textSecondary)
            if let pending {AskQuestionCard(pending: pending, onAnswer: answer, readOnly: !ready || busy || original != nil || state != "awaiting_answer", initialSelections: selections, titleOverride: state == "awaiting_answer" ? "队员需要你的选择" : "原问题记录", subtitleOverride: LocalizedStringKey(conversationQuestionStatus(state))).id(id + ":" + String(value["revision"]?.rvInt ?? 0) + ":" + state)} else {Text("原问题数据暂不可读，请保留记录并核对。").foregroundStyle(Theme.warning)}
            if result?["outcome"]?["declined"] == .bool(true) {Text("你已明确选择不回答。").font(.caption)}
            HStack {Button(original == nil ? "查看原回执" : "核对原回答") {Task {await reconcile()}};if ["awaiting_answer", "outcome_unknown"].contains(state) {Button("取消剩余投递") {Task {await cancel()}}}}.font(.caption).disabled(busy || generation != model.connectionGeneration)
            if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning)}
        }.task(id: id) {
            generation = model.connectionGeneration;ready = false;issue = nil
            do {let raw = try model.readBody(owner);if !raw.isEmpty {guard raw.utf8.count <= 1024 else {throw RovaiHostFailure(code: "原回答记录过大")};let packet = try JSONDecoder().decode(JSONValue.self, from: Data(raw.utf8));guard Set(packet.rvObject.keys) == ["operation", "commandId", "requestId"], packet.text("requestId") == id, ["conversationTools.questions.answer", "conversationTools.questions.decline", "conversationTools.requests.resolve"].contains(packet.text("operation")), UUID(uuidString: packet.text("commandId")) != nil else {throw RovaiHostFailure(code: "原回答记录损坏")};original = packet};ready = true} catch {issue = error.localizedDescription}
        }
    }
    private func retain(_ packet: JSONValue) throws {
        guard generation == model.connectionGeneration else {throw CancellationError()};try model.saveBody(owner, body: String(decoding: JSONEncoder().encode(packet), as: UTF8.self));original = packet
    }
    private func clear(_ packet: JSONValue) throws {
        guard let generation else {throw CancellationError()};let raw = String(decoding: try JSONEncoder().encode(packet), as: UTF8.self);guard try model.clearBodyIfMatches(owner, expectedBody: raw, expectedGeneration: generation) else {throw RovaiHostFailure(code: "原回答记录已变化")};original = nil
    }
    private func answer(_ answer: QuestionAnswer) async -> Bool {
        guard ready, !busy, original == nil, state == "awaiting_answer", generation == model.connectionGeneration else {return false};busy = true;issue = nil;defer {busy = false}
        let operation: RovaiOperation = answer.declined ? .conversationToolsQuestionsDecline : .conversationToolsQuestionsAnswer
        let command = UUID().uuidString.lowercased(), packet = JSONValue.object(["operation": .string(operation.rawValue), "commandId": .string(command), "requestId": .string(id)])
        do {try retain(packet);var input: [String: JSONValue] = ["commandId": .string(command), "requestId": .string(id), "expectedRevision": value["revision"] ?? .number(0)];if !answer.declined {let encoded = try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(answer));input["answers"] = encoded["answers"] ?? .array([])};result = try await nativeUserCall(model, operation, .object(input));try clear(packet);onChanged();return true} catch {if generation == model.connectionGeneration {issue = error.localizedDescription};return false}
    }
    private func reconcile() async {
        guard !busy, generation == model.connectionGeneration else {return};busy = true;issue = nil;defer {busy = false}
        do {if let original {let response = try await nativeUserCall(model, .commandsReconcile, .object(["operation": original["operation"] ?? .null, "params": .object(["commandId": original["commandId"] ?? .null])]));if response.text("state") == "recorded", let receipt = response["result"], receipt.text("commandId") == original.text("commandId"), receipt.text("commandType") == original.text("operation") {try clear(original);if receipt.text("status") == "rejected" {issue = receipt.text("code")}} else {issue = "原回答还没有确定回执，没有重复发送"}};result = try await nativeUserCall(model, .conversationToolsQuestionsReconcile, .object(["requestId": .string(id)]));onChanged()} catch {if generation == model.connectionGeneration {issue = error.localizedDescription}}
    }
    private func cancel() async {
        guard !busy, generation == model.connectionGeneration else {return};busy = true;issue = nil;defer {busy = false}
        let packet = JSONValue.object(["operation": .string("conversationTools.requests.resolve"), "commandId": .string(UUID().uuidString.lowercased()), "requestId": .string(id)])
        do {try retain(packet);result = try await nativeUserCall(model, .conversationToolsRequestsResolve, .object(["commandId": packet["commandId"] ?? .null, "requestId": .string(id), "action": .string("cancel_remaining")]));try clear(packet);onChanged()} catch {if generation == model.connectionGeneration {issue = error.localizedDescription}}
    }
}
