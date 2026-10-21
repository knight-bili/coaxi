import SwiftUI

/// Native Codeg cards, retaining Core's original request, option and version.
struct RovaiThreadControls: View {
    let model: RovaiWorkspaceModel
    let threadId: String
    var projection: JSONValue? = nil
    private var snapshot: JSONValue? { projection ?? model.thread }
    @State private var creating = false
    @State private var managingMembers = false
    @State private var editing: TaskSelection?
    private struct TaskSelection: Identifiable { let value: JSONValue; var id: String { value.text("taskId") } }
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Button("会话队员与队长") {managingMembers = true}
            ForEach((snapshot?["approvals"]?.rvArray ?? []).filter { $0.text("status") == "pending" }, id: \.self) { approval in
                GlassRow { VStack(alignment: .leading, spacing: 10) {
                    Text("需要你的许可").font(.headline)
                    Text(approval.text("actionSummary")); if !approval.text("reason").isEmpty { Text(approval.text("reason")).font(.caption).foregroundStyle(Theme.textSecondary) }
                    ForEach(approval["options"]?.rvArray ?? [], id: \.self) { option in Button { Task { await model.execute(.actionApprovalsResolve, params: .object(["commandId": .string(UUID().uuidString.lowercased()), "threadId": .string(threadId), "approvalId": .string(approval.text("id")), "expectedVersion": approval["version"] ?? .number(0), "optionId": .string(option.text("optionId")), "reason": .string("用户选择智能体原生选项：\(option.text("optionId"))。")])) } } label: { VStack(alignment: .leading, spacing: 4) { Text(option.text("label")); Text(option.text("consequence")).font(.caption).foregroundStyle(Theme.textSecondary) }.frame(maxWidth: .infinity, alignment: .leading) }.disabled(model.pending != nil || model.busy) }
                } }
            }
            ForEach(Array(Set((snapshot?["agentRuns"]?.rvArray ?? []).map { $0.text("conversationId") }.filter { !$0.isEmpty })).sorted(), id: \.self) { id in RovaiInteractionCards(model: model, conversationId: id); RovaiDelegationList(model: model, threadId: threadId, parentConversationId: id) }
            ForEach((snapshot?["agentRuns"]?.rvArray ?? []).map { RovaiRow(value: $0) }) { row in RovaiExecutionHistory(model: model, threadId: threadId, run: row.value, observed: snapshot?["executionEvidence"]?.rvArray ?? []) }
            HStack { Text("任务").font(.headline); Spacer(); Button("新建任务") { creating = true }.disabled(model.pending != nil) }
            ForEach(snapshot?["tasks"]?.rvArray ?? [], id: \.self) { task in GlassRow { VStack(alignment: .leading, spacing: 8) { HStack { Text(task.text("title")).font(.headline); Spacer(); Text(taskStatusName(task.text("status"))).font(.caption).foregroundStyle(Theme.textSecondary) }; Text(task.text("description")); if !task.text("blockedReason").isEmpty { Text(task.text("blockedReason")).foregroundStyle(Theme.warning) }; if !task.text("completionSummary").isEmpty { MarkdownContent(raw: task.text("completionSummary")) }; Button("任务详情") { editing = TaskSelection(value: task) }.disabled(model.pending != nil) } } }
        }
        .sheet(isPresented: $managingMembers) {RovaiThreadMembers(model: model, threadId: threadId)}
        .sheet(isPresented: $creating) { RovaiTaskEditor(model: model, threadId: threadId, task: nil, projection: snapshot) }
        .sheet(item: $editing) { RovaiTaskEditor(model: model, threadId: threadId, task: $0.value, projection: snapshot) }
    }
}

struct RovaiInteractionCards: View {
    let model: RovaiWorkspaceModel
    let conversationId: String
    @State private var values: [JSONValue] = []
    @State private var cursor: String?
    @State private var issue: String?
    @State private var busy = false
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if let issue { Text(issue).font(.caption).foregroundStyle(Theme.warning) }
            ForEach(values, id: \.self) { value in RovaiInteractionCard(model: model, value: value).id("\(value.text("id")):\(value["version"]?.rvInt ?? 0)") }
            if let cursor { Button("更多待回答事项") { Task { await load(after: cursor) } }.disabled(busy) }
        }.task(id: model.thread) { await load() }.task(id: model.lastResult) { await load() }
    }
    private func load(after: String? = nil) async {
        guard !busy else { return }; busy = true; defer { busy = false }
        do {
            var args: [String: JSONValue] = ["conversationId": .string(conversationId), "limit": .number(50)]; if let after { args["cursor"] = .string(after) }
            let page = try await model.query(.runtimeInteractionsList, params: .object(args)); var next: [JSONValue] = []
            for summary in page["items"]?.rvArray ?? [] where summary.text("status") == "pending" { let value = try await model.query(.runtimeInteractionsGet, params: .object(["interactionId": .string(summary.text("id"))])); if value.text("conversationId") == conversationId && value.text("status") == "pending" { next.append(value) } }
            values = after == nil ? next : values + next; cursor = page["nextCursor"]?.stringValue; issue = nil
        } catch { issue = error.localizedDescription }
    }
}

private struct RovaiInteractionCard: View {
    let model: RovaiWorkspaceModel
    let value: JSONValue
    @State private var feedback = ""
    @State private var choices: [String: Set<String>] = [:]
    @State private var custom: [String: String] = [:]
    private var bodyValue: JSONValue { value["body"] ?? .object([:]) }
    private var blocked: Bool { model.pending != nil || model.busy }
    var body: some View {
        GlassRow { VStack(alignment: .leading, spacing: 12) {
            if value.text("kind") == "plan_approval" {
                Text("计划待确认").font(.headline); MarkdownContent(raw: bodyValue.text("planMarkdown"))
                TextField("反馈（可选）", text: $feedback, axis: .vertical).textFieldStyle(.roundedBorder).disabled(blocked)
                HStack { planButton("同意计划", decision: "approve"); planButton("要求修改", decision: "request_changes"); planButton("放弃计划", decision: "abandon") }
            } else {
                Text("队员需要你的回答").font(.headline)
                ForEach(bodyValue["questions"]?.rvArray ?? [], id: \.self) { question in
                    Text(question.text("question"))
                    ForEach(question["options"]?.rvArray ?? [], id: \.self) { option in let id = question.text("id"); let label = option.text("label"); Button { var selected = choices[id] ?? []; if selected.contains(label) { selected.remove(label) } else if question["multiSelect"] == .bool(true) { selected.insert(label) } else { selected = [label] }; choices[id] = selected } label: { HStack(alignment: .top) { Image(systemName: choices[id]?.contains(label) == true ? "checkmark.circle.fill" : "circle"); VStack(alignment: .leading, spacing: 4) { Text(label); Text(option.text("description")).font(.caption).foregroundStyle(Theme.textSecondary) } } }.disabled(blocked) }
                    TextField("补充回答", text: Binding(get: { custom[question.text("id")] ?? "" }, set: { custom[question.text("id")] = $0 }), axis: .vertical).textFieldStyle(.roundedBorder).disabled(blocked)
                }
                HStack { Button("提交回答") { answerQuestions(declined: false) }; Button("暂不回答") { answerQuestions(declined: true) } }.disabled(blocked)
            }
        } }
    }
    private func planButton(_ title: String, decision: String) -> some View { Button(title) { resolve(.object(["kind": .string("plan_approval"), "decision": .string(decision), "feedback": feedback.isEmpty ? .null : .string(feedback)])) }.disabled(blocked) }
    private func answerQuestions(declined: Bool) {
        let answers: [JSONValue] = declined ? [] : (bodyValue["questions"]?.rvArray ?? []).map { question in let id = question.text("id"); var labels = choices[id] ?? []; if let text = custom[id], !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { labels.insert(text) }; return .object(["questionId": .string(id), "labels": .array(labels.sorted().map { .string($0) })]) }
        resolve(.object(["kind": .string("question"), "answers": .array(answers), "declined": .bool(declined)]))
    }
    private func resolve(_ answer: JSONValue) { Task { await model.command(.runtimeInteractionsResolve, value: .object(["interactionId": .string(value.text("id")), "expectedVersion": value["version"] ?? .number(0), "answer": answer])) } }
}
