import SwiftUI

private let notificationNames = ["approval_pending": "待审批", "user_mention": "提到你", "turn_completed": "本轮完成", "turn_failed": "执行失败", "turn_incomplete": "执行未完成", "round_completed": "协作完成", "single_chat_reply": "单聊回复", "mission_needs_you": "使命需要你", "mission_status_changed": "使命状态变更", "task_status_changed": "任务状态变更"]
private struct NotificationSourceSelection: Identifiable { let value: JSONValue; var id: String { value.text("actionId") } }
struct RovaiNotificationInbox: View {
    let model: RovaiWorkspaceModel
    @Environment(\.dismiss) private var dismiss
    @State private var page: JSONValue = .object([:])
    @State private var cursor: String?
    @State private var unread = false
    @State private var loading = false
    @State private var issue: String?
    @State private var source: NotificationSourceSelection?
    var body: some View {
        NavigationStack {
            List {
                Section { Toggle("只看未读", isOn: $unread).disabled(loading); Text("\(page["unreadCount"]?.rvInt ?? 0) 条未读").font(.caption).foregroundStyle(Theme.textSecondary) }
                if let issue { Section { Text(issue).font(.caption).foregroundStyle(Theme.warning) } }
                ForEach((page["items"]?.rvArray ?? []).map { RovaiRow(value: $0) }) { row in episode(row.value) }
                if let next = page["nextCursor"]?.stringValue { Button("更早的提醒") { Task { await load(after: next) } }.disabled(loading) }
            }.scrollContentBackground(.hidden).navigationTitle("提醒")
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("关闭") { dismiss() } }; ToolbarItem(placement: .topBarTrailing) { Button("最新") { Task { await load() } }.disabled(loading) } }
        }.task(id: unread) { await load() }
        .onChange(of: model.lastResult) { _, _ in if model.lastOperation == .notificationsAcknowledge { Task { await load(after: cursor) } } }
        .sheet(item: $source) { item in
            if item.value.text("kind") == "open_single_chat", let single = item.value["singleChat"], !single.text("conversationId").isEmpty { RovaiPrivateConversationView(model: model, threadId: item.value.text("threadId"), initialConversationId: single.text("conversationId")) }
            else { RovaiNotificationSource(model: model, action: item.value) }
        }
    }
    private func episode(_ value: JSONValue) -> some View {
        GlassRow(isSelected: value["unread"]?.rvBool == true) { VStack(alignment: .leading, spacing: 10) {
            Text(notificationNames[value.text("primarySemantic")] ?? "提醒").font(.headline)
            Text(value["thread"]?.text("title") ?? "").font(.subheadline).foregroundStyle(Theme.textSecondary)
            if let summary = value["mention"]?["summary"]?.stringValue, !summary.isEmpty { Text(summary) }
            Text(value.text("updatedAt")).font(.caption2).foregroundStyle(Theme.textSecondary)
            ForEach(([value["primaryAction"] ?? .null] + (value["secondaryActions"]?.rvArray ?? [])).filter { $0["available"]?.rvBool == true }, id: \.self) { action in HStack {
                if action.text("kind") != "acknowledge_only" { Button(actionName(action.text("kind"))) { source = NotificationSourceSelection(value: action) } }
                if let acknowledgement = action["acknowledgementId"]?.stringValue { Button(action.text("kind") == "acknowledge_only" ? "知道了" : "标记此条提醒已读") { Task { await model.command(.notificationsAcknowledge, value: .object(["episodeId": value["id"] ?? .null, "observedEpisodeVersion": action["observedEpisodeVersion"] ?? .number(0), "acknowledgementId": .string(acknowledgement)])) } }.disabled(model.pending != nil || model.busy) }
            }.buttonStyle(.borderless) }
        } }.listRowBackground(Color.clear).listRowSeparator(.hidden)
    }
    private func actionName(_ kind: String) -> String { ["open_approval": "查看审批", "open_single_chat": "打开私聊", "open_mission": "查看使命", "open_task": "查看任务"][kind] ?? "查看来源" }
    private func load(after: String? = nil) async {
        guard !loading else { return }; loading = true; defer { loading = false }
        do { var params: [String: JSONValue] = ["filter": .string(unread ? "unread" : "all"), "limit": .number(50)]; if let after { params["cursor"] = .string(after) }; let value = try await model.query(.notificationsInbox, params: .object(params)); guard value["schemaVersion"]?.rvInt == 9 else { issue = "提醒收件箱格式不兼容"; return }; page = value; cursor = after; issue = nil } catch is CancellationError {} catch { issue = error.localizedDescription }
    }
}

/// A separate authorized projection leaves the active Thread and its composer untouched.
private struct RovaiNotificationSource: View {
    let model: RovaiWorkspaceModel
    let action: JSONValue
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase
    @State private var snapshot: JSONValue = .object([:])
    @State private var message: JSONValue?
    @State private var issue: String?
    private var threadId: String { action.text("threadId") }
    var body: some View {
        NavigationStack {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 14) {
                    if let issue { Text(issue).font(.caption).foregroundStyle(Theme.warning) }
                    if model.pending != nil { HStack { Text("原操作待核对").font(.caption); Spacer(); Button("核对") { Task { await model.reconcile() } }; Button("重试原操作") { Task { await model.retry() } } }.disabled(model.busy) }
                    if let message { GlassRow(isSelected: true) { VStack(alignment: .leading, spacing: 8) { Text("来源消息").font(.headline); if message["withdrawn"]?.rvBool == true { Text("消息已撤回") } else { MarkdownContent(raw: message.text("body")) } } } }
                    if let subject = action["subject"], subject != .null { GlassRow { VStack(alignment: .leading, spacing: 8) { Text(subject.text("title")).font(.headline); Text(subject.text("status")).font(.caption).foregroundStyle(Theme.textSecondary) } } }
                    if snapshot["thread"]?.text("id") == threadId { RovaiThreadControls(model: model, threadId: threadId, projection: snapshot) }
                }.padding(16)
            }.navigationTitle(snapshot["thread"]?.text("title") ?? "提醒来源").toolbar { ToolbarItem(placement: .cancellationAction) { Button("返回提醒") { dismiss() } } }
        }.task(id: scenePhase) { guard scenePhase == .active else { return }; while !Task.isCancelled { await load(); do { try await Task.sleep(for: .seconds(2)) } catch { return } } }
        .onChange(of: model.lastResult) { _, _ in Task { await load() } }
    }
    private func load() async {
        do { let value = try await model.query(.threadsOpen, params: .object(["threadId": .string(threadId), "traceId": .string(UUID().uuidString.lowercased())])); guard value["thread"]?.text("id") == threadId else { issue = "提醒来源已不可用"; return }; snapshot = value
            if !action.text("messageId").isEmpty { let around = try await model.query(.threadMessagesAround, params: .object(["threadId": .string(threadId), "messageId": action["messageId"] ?? .null])); message = around["messages"]?.rvArray.first { $0.text("id") == action.text("messageId") }; issue = message == nil ? "来源消息已不可用" : nil } else { issue = nil }
        } catch is CancellationError {} catch { issue = error.localizedDescription }
    }
}
