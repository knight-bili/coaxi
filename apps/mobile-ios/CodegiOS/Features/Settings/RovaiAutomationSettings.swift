import SwiftUI

private let rovaiSchedules = [("daily", "每天"), ("weekdays", "工作日"), ("weekly", "每周"), ("once", "仅一次"), ("cron", "自定义"), ("manual", "手动触发")]
private let rovaiWeekdays = [("monday", "周一"), ("tuesday", "周二"), ("wednesday", "周三"), ("thursday", "周四"), ("friday", "周五"), ("saturday", "周六"), ("sunday", "周日")]
private func scheduleText(_ value: JSONValue) -> String { switch value.text("kind") { case "manual": return "手动触发"; case "daily": return "每天 " + value.text("at"); case "weekdays": return "工作日 " + value.text("at"); case "weekly": return "每周 " + (rovaiWeekdays.first { $0.0 == value.text("weekday") }?.1 ?? "") + " " + value.text("at"); case "once": return value.text("date") + " " + value.text("at"); case "cron": return "自定义 · " + value.text("expression"); default: return "未识别的计划" } }
struct RovaiAutomationSettings: View {
    let model: RovaiWorkspaceModel
    let onOpenThread: (String) -> Void
    @State private var items: [JSONValue] = []
    @State private var cursor: String?
    @State private var search = ""
    @State private var filter = "all"
    @State private var issue: String?
    @State private var busy = false
    @State private var editor: Editor?
    @State private var history: JSONValue?
    @State private var runs: [JSONValue] = []
    @State private var runCursor: String?
    @State private var action: Action?
    private struct Editor: Identifiable { let id = UUID(); let value: JSONValue? }
    private struct Action: Identifiable { let id = UUID(); let operation: RovaiOperation; let command: JSONValue; let title: String }
    var body: some View {
        List {
            if let issue { Section { Text(issue).font(.caption).foregroundStyle(Theme.warning) } }
            if let history {
                Section(history.text("name")) { Button("返回任务列表") { self.history = nil }; ForEach(runs, id: \.self) { run in GlassRow { VStack(alignment: .leading, spacing: 8) { Text(run.text("scheduledFor")).font(.headline); Text(run.text("status")).font(.caption).foregroundStyle(Theme.textSecondary); if !run.text("reason").isEmpty { Text(run.text("reason")) }; if !run.text("threadId").isEmpty { Button("打开结果对话") { onOpenThread(run.text("threadId")) } }; if ["running", "cancelling"].contains(run.text("status")) { Button("停止本次运行") { action = Action(operation: .automationsCancelRun, command: .object(["automationId": history["automationId"] ?? .null, "runId": run["runId"] ?? .null]), title: "停止所选运行？") }.disabled(model.pending != nil) } } } }; if let runCursor { Button("更早的运行") { Task { await readRuns(history, cursor: runCursor) } }.disabled(busy) } }
            } else {
                Section { Picker("状态", selection: $filter) { Text("全部").tag("all"); Text("启用").tag("enabled"); Text("已关闭").tag("closed") }.pickerStyle(.segmented); TextField("搜索任务", text: $search).submitLabel(.search).onSubmit { Task { await load() } }; Button("搜索") { Task { await load() } }.disabled(busy) }
                ForEach(items, id: \.self) { item in GlassRow { VStack(alignment: .leading, spacing: 8) { Text(item.text("name")).font(.headline); Text(item.text("prompt")).lineLimit(3); Text(scheduleText(item["schedule"] ?? .null)).font(.caption).foregroundStyle(Theme.textSecondary); if !item.text("nextRunAt").isEmpty { Text("下次 · " + item.text("nextRunAt")).font(.caption) }; ScrollView(.horizontal, showsIndicators: false) { HStack {
                    Button("编辑") { editor = Editor(value: item) }; Button("运行记录") { Task { await readRuns(item) } }; Button("立即运行") { action = Action(operation: .automationsRun, command: .object(["automationId": item["automationId"] ?? .null]), title: "立即运行此任务？") }; Button(item["enabled"]?.rvBool == true ? "关闭" : "启用") { var fields: [String: JSONValue] = ["automationId": item["automationId"] ?? .null, "expectedVersion": item["version"] ?? .null]; let enabled = item["enabled"]?.rvBool == true; if !enabled { fields["enabled"] = .bool(true) }; action = Action(operation: enabled ? .automationsClose : .automationsUpdate, command: .object(fields), title: "修改任务启用状态？") }; Button("删除", role: .destructive) { action = Action(operation: .automationsDelete, command: .object(["automationId": item["automationId"] ?? .null, "expectedVersion": item["version"] ?? .null]), title: "删除定时任务？") }
                }.disabled(model.pending != nil || model.busy) } } } }
                if let cursor { Button("加载更多") { Task { await load(cursor: cursor) } }.disabled(busy) }
            }
            if busy { ProgressView().frame(maxWidth: .infinity) }
        }.scrollContentBackground(.hidden).navigationTitle("定时任务")
        .toolbar { Button { editor = Editor(value: nil) } label: { Image(systemName: "plus") }.disabled(model.pending != nil) }
        .task(id: "\(filter):\(model.resourceVersion)") { try? await Task.sleep(for: .milliseconds(250)); if !Task.isCancelled { await load(); if let history { await readRuns(history) } } }.onChange(of: model.lastResult) { _, _ in Task { await load(); if let history { await readRuns(history) } } }
        .sheet(item: $editor) { item in RovaiAutomationEditor(model: model, original: item.value) { editor = nil; Task { await load() } } }
        .confirmationDialog(action?.title ?? "确认操作", isPresented: Binding(get: { action != nil }, set: { if !$0 { action = nil } }), titleVisibility: .visible) { if let action { Button("确认", role: action.operation == .automationsDelete ? .destructive : nil) { Task { await model.command(action.operation, value: action.command); self.action = nil } }.disabled(model.pending != nil || model.busy) }; Button("取消", role: .cancel) { action = nil } } message: { Text("按所选任务执行。立即运行会在宿主创建实际执行；已有结果仍保留在原对话中。") }
    }
    private func load(cursor: String? = nil) async { guard !busy else { return }; busy = true; defer { busy = false }; do { var fields: [String: JSONValue] = ["status": .string(filter), "query": .string(search), "limit": .number(30)]; if let cursor { fields["cursor"] = .string(cursor) }; let value = try await model.query(.automationsList, params: .object(fields)); items = cursor == nil ? value["automations"]?.rvArray ?? [] : items + (value["automations"]?.rvArray ?? []); self.cursor = value["nextCursor"]?.stringValue; issue = nil } catch is CancellationError {} catch { issue = error.localizedDescription } }
    private func readRuns(_ item: JSONValue, cursor: String? = nil) async { guard !busy else { return }; busy = true; defer { busy = false }; do { var fields: [String: JSONValue] = ["automationId": item["automationId"] ?? .null, "limit": .number(20)]; if let cursor { fields["cursor"] = .string(cursor) }; let value = try await model.query(.automationsRunsList, params: .object(fields)); runs = cursor == nil ? value["runs"]?.rvArray ?? [] : runs + (value["runs"]?.rvArray ?? []); runCursor = value["nextCursor"]?.stringValue; history = item; issue = nil } catch is CancellationError {} catch { issue = error.localizedDescription } }
}
private struct RovaiAutomationEditor: View {
    let model: RovaiWorkspaceModel
    let original: JSONValue?
    let onSaved: () -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var prompt = ""
    @State private var member = ""
    @State private var path = ""
    @State private var kind = "daily"
    @State private var at = "09:00"
    @State private var date = ""
    @State private var weekday = "monday"
    @State private var cron = ""
    @State private var channels: Set<String> = []
    @State private var ready = false
    @State private var issue: String?
    @State private var sent: String?
    private var owner: String { "automation-draft:" + (original?.text("automationId") ?? "new") }
    private var blocked: Bool { !ready || model.pending != nil || model.busy }
    private var draft: JSONValue { .object(["name": .string(name), "prompt": .string(prompt), "memberId": .string(member), "path": .string(path), "kind": .string(kind), "at": .string(at), "date": .string(date), "weekday": .string(weekday), "cron": .string(cron), "channels": .array(channels.sorted().map { .string($0) })]) }
    var body: some View {
        NavigationStack { Form {
            if let issue { Section { Text(issue).font(.caption).foregroundStyle(Theme.warning) } }
            Section("内容") { TextField("名称", text: $name); TextField("任务内容", text: $prompt, axis: .vertical).lineLimit(4...12) }
            Section("执行队员和工作目录") { Picker("队员", selection: $member) { Text("选择队员").tag(""); ForEach(model.members.filter { $0.text("presence") == "present" }, id: \.self) { agent in Text(agent.text("displayName")).tag(agent.text("agentId")) } }; Picker("工程", selection: $path) { Text("快速对话").tag(""); ForEach(model.navigation["projects"]?.rvArray ?? [], id: \.self) { project in Text(project.text("name")).tag(project.text("projectPath")) } } }
            Section("计划（宿主时区）") { Picker("重复", selection: $kind) { ForEach(rovaiSchedules, id: \.0) { value in Text(value.1).tag(value.0) } }; if !["manual", "cron"].contains(kind) { TextField("时间 HH:mm", text: $at).keyboardType(.numbersAndPunctuation) }; if kind == "weekly" { Picker("星期", selection: $weekday) { ForEach(rovaiWeekdays, id: \.0) { value in Text(value.1).tag(value.0) } } }; if kind == "once" { TextField("日期 yyyy-MM-dd", text: $date).keyboardType(.numbersAndPunctuation) }; if kind == "cron" { TextField("分钟 小时 日期 月份 星期", text: $cron).autocorrectionDisabled().textInputAutocapitalization(.never) } }
            Section("结果通知") { ForEach([("feishu", "飞书"), ("lark", "Lark"), ("dingtalk", "钉钉")], id: \.0) { value in Toggle(value.1, isOn: Binding(get: { channels.contains(value.0) }, set: { if $0 { channels.insert(value.0) } else { channels.remove(value.0) } })) } }
        }.disabled(blocked).navigationTitle(original == nil ? "新建定时任务" : "编辑定时任务").toolbar { ToolbarItem(placement: .cancellationAction) { Button("返回，保留草稿") { dismiss() } }; ToolbarItem(placement: .confirmationAction) { Button("保存") { Task { await save() } }.disabled(blocked || prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || member.isEmpty) } } }
        .task { do { let raw = try model.readBody(owner); if !raw.isEmpty { guard let value = JSONValue.parse(raw) else { throw RovaiHostFailure(code: "任务草稿无法读取") }; restore(value) } else if let original { name = original.text("name"); prompt = original.text("prompt"); member = original.text("memberId"); path = original["projectRef"]?.text("path") ?? ""; kind = original["schedule"]?.text("kind") ?? "daily"; at = original["schedule"]?.text("at") ?? "09:00"; date = original["schedule"]?.text("date") ?? ""; weekday = original["schedule"]?.text("weekday") ?? "monday"; cron = original["schedule"]?.text("expression") ?? ""; channels = Set((original["notifyChannels"]?.rvArray ?? []).compactMap(\.stringValue)) }; ready = true } catch { issue = error.localizedDescription } }
        .onChange(of: draft) { _, value in if ready { do { try model.saveBody(owner, body: value.prettyString) } catch { issue = error.localizedDescription } } }
        .onChange(of: model.lastResult) { _, value in let receipt = value?["commandResult"] ?? value; if sent != nil, receipt?.text("commandId") == sent, ["applied", "accepted"].contains(receipt?.text("status") ?? "") { do { try model.saveBody(owner, body: ""); onSaved() } catch { issue = error.localizedDescription } } }
    }
    private func restore(_ value: JSONValue) { name = value.text("name"); prompt = value.text("prompt"); member = value.text("memberId"); path = value.text("path"); kind = value.text("kind"); at = value.text("at"); date = value.text("date"); weekday = value.text("weekday"); cron = value.text("cron"); channels = Set((value["channels"]?.rvArray ?? []).compactMap(\.stringValue)) }
    private func save() async { do {
        if !["manual", "cron"].contains(kind), at.range(of: "^([01][0-9]|2[0-3]):[0-5][0-9]$", options: .regularExpression) == nil { throw RovaiHostFailure(code: "请填写有效时间") }
        if kind == "once" { let formatter = DateFormatter(); formatter.locale = Locale(identifier: "en_US_POSIX"); formatter.dateFormat = "yyyy-MM-dd"; formatter.isLenient = false; guard let day = formatter.date(from: date), formatter.string(from: day) == date else { throw RovaiHostFailure(code: "请填写有效日期") } }
        if kind == "cron", cron.split(whereSeparator: \.isWhitespace).count != 5 { throw RovaiHostFailure(code: "Cron需要5个字段") }
        try model.saveBody(owner, body: draft.prettyString); var schedule: [String: JSONValue] = ["kind": .string(kind)]; if !["manual", "cron"].contains(kind) { schedule["at"] = .string(at) }; if kind == "once" { schedule["date"] = .string(date) }; if kind == "weekly" { schedule["weekday"] = .string(weekday) }; if kind == "cron" { schedule["expression"] = .string(cron) }
        var value: [String: JSONValue] = ["name": .string(name), "prompt": .string(prompt), "memberId": .string(member), "projectRef": path.isEmpty ? .object(["kind": .string("quick_chat")]) : .object(["kind": .string("directory"), "path": .string(path)]), "schedule": .object(schedule), "notifyChannels": .array(channels.sorted().map { .string($0) })]
        if let original { value["automationId"] = original["automationId"]; value["expectedVersion"] = original["version"] }; let id = UUID().uuidString.lowercased(); sent = id; await model.execute(original == nil ? .automationsCreate : .automationsUpdate, params: .object(["commandId": .string(id), "command": .object(value)]))
    } catch { issue = error.localizedDescription } }
}
