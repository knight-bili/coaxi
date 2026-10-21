import SwiftUI

struct RovaiDelegationList: View {
    let model: RovaiWorkspaceModel
    let threadId: String
    let parentConversationId: String
    @State private var values: [JSONValue] = []
    @State private var cursor: String?
    @State private var issue: String?
    @State private var busy = false
    @State private var opened: HelperSelection?
    @State private var action: HelperAction?
    private struct HelperSelection: Identifiable { let value: JSONValue; var id: String { value.text("delegationId") } }
    private struct HelperAction: Identifiable { let operation: RovaiOperation; let value: JSONValue; let title: String; let detail: String; let id = UUID() }
    private var disabled: Bool { busy || model.busy || model.pending != nil }
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if let issue { Text(issue).font(.caption).foregroundStyle(Theme.warning) }
            ForEach(values.map { HelperSelection(value: $0) }) { row in
                GlassRow { VStack(alignment: .leading, spacing: 10) {
                    Text("独立帮手 · \(row.value.text("agentType"))").font(.headline)
                    Text("\(row.value.text("status")) · 第 \(row.value["currentAttemptNo"]?.rvInt ?? 0) 次").font(.caption).foregroundStyle(Theme.textSecondary)
                    HStack {
                        Button("查看") { opened = row }.disabled(busy)
                        if row.value["takenOver"]?.rvBool != true { Button("接管") { action = HelperAction(operation: .delegationsTakeover, value: .object(["delegationId": .string(row.id), "expectedControlVersion": row.value["controlVersion"] ?? .number(0)]), title: "接管独立帮手", detail: "接管后由你直接发消息，原队员不再继续控制这位帮手。历史记录保留。") }.disabled(disabled) }
                        if ["queued", "running", "waiting", "waiting_permission", "waiting_question", "waiting_plan_approval"].contains(row.value.text("status")) {
                            Button("停止", role: .destructive) { action = HelperAction(operation: .delegationsCancel, value: .object(["delegationId": .string(row.id), "attemptNo": row.value["currentAttemptNo"] ?? .number(0)]), title: "停止这次帮手执行", detail: "停止当前尝试，保留记录和实际输出。") }.disabled(disabled)
                        } else {
                            Button("继续") { action = HelperAction(operation: .delegationsResume, value: .object(["delegationId": .string(row.id), "expectedAttemptNo": row.value["currentAttemptNo"] ?? .number(0), "reason": .string("用户从移动客户端请求继续独立帮手。")]), title: "继续独立帮手", detail: "使用Core保存的材料开始下一次尝试，已完成的尝试保留。") }.disabled(disabled)
                        }
                    }
                } }
            }
            if let cursor { Button("更多独立帮手") { Task { await load(after: cursor) } }.disabled(busy) }
        }.task(id: parentConversationId) { await load() }.onChange(of: model.thread) { _, _ in Task { await load() } }.onChange(of: model.lastResult) { _, _ in Task { await load() } }
        .sheet(item: $opened) { item in RovaiPrivateConversationView(model: model, threadId: threadId, childId: item.value.text("conversationId")) }
        .confirmationDialog(action?.title ?? "确认操作", isPresented: Binding(get: { action != nil }, set: { if !$0 { action = nil } }), titleVisibility: .visible) { if let action { Button("确认") { Task { await model.command(action.operation, value: action.value) }; self.action = nil } } } message: { Text(action?.detail ?? "") }
    }
    private func load(after: String? = nil) async {
        guard !busy else { return }; busy = true; defer { busy = false }
        do { var params: [String: JSONValue] = ["parentConversationId": .string(parentConversationId), "limit": .number(50)]; if let after { params["cursor"] = .string(after) }; let page = try await model.query(.delegationsList, params: .object(params)); let rows = (page["items"]?.rvArray ?? []).filter { $0.text("threadId") == threadId && $0.text("parentConversationId") == parentConversationId }; values = after == nil ? rows : values + rows; cursor = page["nextCursor"]?.stringValue; issue = nil } catch is CancellationError {} catch { issue = error.localizedDescription }
    }
}
