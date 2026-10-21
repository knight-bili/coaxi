import SwiftUI

private let rovaiTaskStatuses = [("pending", "待处理"), ("in_progress", "进行中"), ("blocked", "已阻塞"), ("completed", "已完成"), ("cancelled", "已取消")]
func taskStatusName(_ value: String) -> String {rovaiTaskStatuses.first {$0.0 == value}?.1 ?? value}

/// TaskPanel field patches and terminal rules, retaining complete native drafts and original request receipts.
struct RovaiTaskEditor: View {
    let model: RovaiWorkspaceModel
    let threadId: String
    let task: JSONValue?
    @Environment(\.dismiss) private var dismiss
    @State private var baseline: JSONValue?
    @State private var title = "", detail = "", assignee = "", status = "pending", blockedReason = "", completion = "", cancelReason = ""
    @State private var commandId = UUID().uuidString.lowercased()
    @State private var generation: UInt64?, issue: String?
    @State private var ready = false, busy = false, cancelling = false
    private var key: String {task?.text("taskId") ?? "new"}
    private var owner: String {"task-edit:\(threadId):\(key)"}
    private var terminal: Bool {["completed", "cancelled"].contains(baseline?.text("status") ?? "")}
    private var locked: Bool {!ready || terminal || busy || model.busy || model.pending != nil || generation != model.connectionGeneration}
    private var members: [JSONValue] {guard model.thread?["thread"]?.text("id") == threadId else {return []};return (model.thread?["members"]?.rvArray ?? []).filter {$0.text("membershipStatus") == "active"}}
    private var valid: Bool {!title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && title.trimmingCharacters(in: .whitespacesAndNewlines).unicodeScalars.count <= 160 && detail.unicodeScalars.count <= 16000 && blockedReason.unicodeScalars.count <= 4000 && completion.unicodeScalars.count <= 4000 && (task != nil || !assignee.isEmpty) && (!["in_progress", "blocked"].contains(status) || !assignee.isEmpty) && (status != "blocked" || !blockedReason.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty) && (status != "completed" || !completion.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)}
    var body: some View {
        NavigationStack {Form {
            if let issue {Section {Text(issue).foregroundStyle(Theme.warning)}}
            if model.pending != nil {Section {Text("原请求待核对，完整草稿保留。关闭后可在工作区核对原结果。").font(.caption)}}
            if terminal {Section {Text("终态任务不可修改。").foregroundStyle(Theme.textSecondary)}}
            Section("责任定义") {
                TextField("标题", text: changed($title))
                TextField("责任范围与要求", text: changed($detail), axis: .vertical).lineLimit(5...16)
            }.disabled(locked)
            Section("负责人") {
                Picker("负责人", selection: changed($assignee)) {
                    Text(task == nil ? "请选择负责人" : "未分配").tag("")
                    ForEach(members, id: \.self) {Text($0.text("displayName")).tag($0.text("agentId"))}
                    if !assignee.isEmpty, !members.contains(where: {$0.text("agentId") == assignee}) {Text("原负责人已不可用").tag(assignee)}
                }
            }.disabled(locked)
            if task != nil {Section("状态") {
                Picker("状态", selection: changed($status)) {ForEach(rovaiTaskStatuses.filter {$0.0 != "cancelled" || terminal}, id: \.0) {value, label in Text(label).tag(value)}}
                if status == "blocked" {TextField("阻塞原因", text: changed($blockedReason), axis: .vertical).lineLimit(3...12)}
                if status == "completed" {TextField("完成摘要", text: changed($completion), axis: .vertical).lineLimit(3...12)}
            }.disabled(locked)}
            if let baseline {Section("记录") {
                LabeledContent("创建", value: baseline.text("createdAt"))
                LabeledContent("创建来源", value: baseline.text("createdByType"))
                LabeledContent("更新", value: baseline.text("updatedAt"))
                if let closed = baseline["closedAt"]?.stringValue {LabeledContent("关闭", value: closed)}
                if !baseline.text("cancelReason").isEmpty {Text("取消原因 · \(baseline.text("cancelReason"))")}
            }}
            if task != nil, !terminal {Section {Button("取消任务", role: .destructive) {cancelling = true}.disabled(locked)}}
        }.navigationTitle(task == nil ? "新建任务" : "任务详情").navigationBarTitleDisplayMode(.inline).toolbar {
            ToolbarItem(placement: .cancellationAction) {Button("保存草稿并关闭") {close()}.disabled(busy)}
            if !terminal {ToolbarItem(placement: .confirmationAction) {Button(task == nil ? "创建任务" : "保存修改") {Task {await submit(cancel: false)}}.disabled(locked || !valid)}}
        }}.interactiveDismissDisabled(true)
        .task {restore()}
        .onChange(of: model.lastResult) {_, result in
            let receipt = result?["commandResult"] ?? result
            guard model.lastInputOwner == owner, receipt?.text("commandId") == commandId else {return}
            if ["applied", "accepted"].contains(receipt?.text("status") ?? "") {ready = false;dismiss()}
            else if receipt?.text("status") == "rejected" {commandId = UUID().uuidString.lowercased();issue = receipt?.text("code");persist()}
        }
        .sheet(isPresented: $cancelling) {NavigationStack {Form {
            Section {Text("取消是终态操作，正文草稿中的其他修改不会随取消一起提交。");TextField("取消原因", text: changed($cancelReason), axis: .vertical).lineLimit(3...12).disabled(locked)}
            Section {Button("确认取消", role: .destructive) {Task {await submit(cancel: true)}}.disabled(locked || cancelReason.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || cancelReason.unicodeScalars.count > 4000)}
        }.navigationTitle("取消这个任务？").toolbar {Button("保留任务") {cancelling = false}.disabled(busy)}}.interactiveDismissDisabled(busy)}
    }
    private func changed<T>(_ binding: Binding<T>) -> Binding<T> {Binding(get: {binding.wrappedValue}, set: {binding.wrappedValue = $0;persist()})}
    private func document() throws -> String {
        let value = JSONValue.object(["version": .number(1), "threadId": .string(threadId), "taskKey": .string(key), "baseline": baseline ?? .null, "title": .string(title), "description": .string(detail), "assignee": .string(assignee), "status": .string(status), "blockedReason": .string(blockedReason), "completionSummary": .string(completion), "cancelReason": .string(cancelReason), "commandId": .string(commandId)])
        let encoder = JSONEncoder();encoder.outputFormatting = [.sortedKeys];return String(decoding: try encoder.encode(value), as: UTF8.self)
    }
    private func persist() {guard ready, generation == model.connectionGeneration else {return};do {try model.saveBody(owner, body: document())} catch {issue = error.localizedDescription}}
    private func restore() {
        guard !ready else {return};generation = model.connectionGeneration;baseline = task;title = task?.text("title") ?? "";detail = task?.text("description") ?? "";assignee = task?.text("assigneeAgentId") ?? "";status = task?.text("status") ?? "pending";blockedReason = task?.text("blockedReason") ?? "";completion = task?.text("completionSummary") ?? ""
        do {let raw = try model.readBody(owner);if !raw.isEmpty {
            guard let value = JSONValue.parse(raw), value["version"]?.rvInt == 1, value.text("threadId") == threadId, value.text("taskKey") == key else {throw RovaiHostFailure(code: "原任务草稿无法读取，原记录保留")}
            if let base = value["baseline"], base != .null {baseline = base}
            title = value.text("title");detail = value.text("description");assignee = value.text("assignee");status = value.text("status");blockedReason = value.text("blockedReason");completion = value.text("completionSummary");cancelReason = value.text("cancelReason");if !value.text("commandId").isEmpty {commandId = value.text("commandId")}
        };ready = true} catch {issue = error.localizedDescription}
    }
    private func close() {guard !busy else {return};guard ready, generation == model.connectionGeneration else {dismiss();return};do {try model.saveBody(owner, body: document());dismiss()} catch {issue = error.localizedDescription}}
    private func submit(cancel: Bool) async {
        guard !locked else {return}
        var args: [String: JSONValue] = ["commandId": .string(commandId), "threadId": .string(threadId)]
        let normalizedTitle = title.trimmingCharacters(in: .whitespacesAndNewlines), normalizedDetail = detail.trimmingCharacters(in: .whitespacesAndNewlines)
        if task == nil {args["title"] = .string(normalizedTitle);args["description"] = .string(normalizedDetail);args["assigneeAgentId"] = .string(assignee)}
        else {
            args["taskId"] = .string(key)
            if cancel {args["status"] = .string("cancelled");args["cancelReason"] = .string(cancelReason.trimmingCharacters(in: .whitespacesAndNewlines))}
            else {
                if normalizedTitle != baseline?.text("title") {args["title"] = .string(normalizedTitle)}
                if normalizedDetail != baseline?.text("description") {args["description"] = .string(normalizedDetail)}
                if status != baseline?.text("status") {args["status"] = .string(status)}
                if assignee != baseline?.text("assigneeAgentId") {args["assignee"] = assignee.isEmpty ? .object(["operation": .string("clear")]) : .object(["operation": .string("assign"), "agentId": .string(assignee)])}
                if status == "blocked", blockedReason.trimmingCharacters(in: .whitespacesAndNewlines) != baseline?.text("blockedReason") {args["blockedReason"] = .string(blockedReason.trimmingCharacters(in: .whitespacesAndNewlines))}
                if status == "completed", completion.trimmingCharacters(in: .whitespacesAndNewlines) != baseline?.text("completionSummary") {args["completionSummary"] = .string(completion.trimmingCharacters(in: .whitespacesAndNewlines))}
            }
            guard args.count > 3 else {issue = "没有需要提交的修改";return}
        }
        busy = true;defer {busy = false}
        do {let body = try document();try model.saveBody(owner, body: body);guard generation == model.connectionGeneration else {throw CancellationError()};await model.executeOwned(task == nil ? .tasksCreate : .tasksUpdate, params: .object(args), owner: owner, body: body)} catch {issue = error.localizedDescription}
    }
}
