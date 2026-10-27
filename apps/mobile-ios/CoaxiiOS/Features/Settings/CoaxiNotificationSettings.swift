import SwiftUI

struct CoaxiNotificationSettings: View {
    let model: CoaxiWorkspaceModel
    @State private var preference: JSONValue?
    @State private var issue: String?
    @State private var loading = false
    private let groups = [("会话", [("approvalHeadsUpEnabled", "待审批"), ("userMentionHeadsUpEnabled", "提到你"), ("turnCompletedHeadsUpEnabled", "本轮完成"), ("singleChatHeadsUpEnabled", "单聊回复"), ("turnIncompleteHeadsUpEnabled", "执行未完成")]), ("使命", [("missionNeedsYouHeadsUpEnabled", "使命需要你"), ("missionStatusHeadsUpEnabled", "使命状态变更")]), ("任务", [("taskStatusHeadsUpEnabled", "任务状态变更")])]
    private let filters = ["missionStatuses": [("completed", "已完成"), ("in_progress", "进行中"), ("not_started", "未开始")], "taskStatuses": [("completed", "已完成"), ("blocked", "受阻"), ("cancelled", "已取消"), ("in_progress", "进行中"), ("pending", "待开始")]]
    private var disabled: Bool { loading || model.busy || model.pending != nil || preference == nil }
    private var enabled: Bool { preference?["headsUpEnabled"]?.rvBool == true }
    var body: some View {
        Form {
            if let issue { Section { Text(issue).font(.caption).foregroundStyle(Theme.warning) } }
            Section { Toggle("应用内提醒", isOn: Binding(get: { enabled }, set: { update("headsUpEnabled", value: .bool($0)) })).disabled(disabled) }
            ForEach(groups, id: \.0) { title, categories in Section(title) {
                ForEach(categories, id: \.0) { key, label in Toggle(label, isOn: Binding(get: { preference?[key]?.rvBool == true }, set: { update(key, value: .bool($0)) })).disabled(disabled || !enabled) }
                if title == "使命" { statuses("missionStatuses", switchKey: "missionStatusHeadsUpEnabled") }
                if title == "任务" { statuses("taskStatuses", switchKey: "taskStatusHeadsUpEnabled") }
            } }
        }.scrollContentBackground(.hidden).navigationTitle("提醒")
        .toolbar { Button("刷新") { Task { await load() } }.disabled(loading || model.busy) }
        .task { await load() }.onChange(of: model.lastResult) { _, _ in if model.lastOperation == .notificationsPreferenceUpdate { Task { await load() } } }
    }
    private func statuses(_ key: String, switchKey: String) -> some View {
        ForEach(filters[key] ?? [], id: \.0) { id, label in Toggle(label, isOn: Binding(get: { preference?[key]?.rvArray.contains(.string(id)) == true }, set: { checked in let selected = preference?[key]?.rvArray ?? []; let next = (filters[key] ?? []).filter { $0.0 == id ? checked : selected.contains(.string($0.0)) }.map { JSONValue.string($0.0) }; update(key, value: .array(next)) })).disabled(disabled || !enabled || preference?[switchKey]?.rvBool != true) }
    }
    private func update(_ key: String, value: JSONValue) {
        guard let preference, !disabled else { return }
        var command: [String: JSONValue] = ["expectedVersion": preference["version"] ?? .number(0)]
        for field in ["headsUpEnabled"] + groups.flatMap({ $0.1.map(\.0) }) + ["missionStatuses", "taskStatuses"] { command[field] = field == key ? value : preference[field] ?? .null }
        Task { await model.command(.notificationsPreferenceUpdate, value: .object(command)) }
    }
    private func load() async { loading = true; defer { loading = false }; do { let value = try await model.query(.notificationsPreferenceGet); guard value["headsUpEnabled"] != nil, value["version"] != nil else { issue = "提醒设置格式不兼容"; return }; preference = value; issue = nil } catch is CancellationError {} catch { issue = error.localizedDescription } }
}
