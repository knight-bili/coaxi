import SwiftUI

let rovaiMissionStatuses: [(String, String)] = [("needs_you", "需要你"), ("not_started", "未开始"), ("in_progress", "进行中"), ("completed", "已完成")]
func missionStatusName(_ value: String) -> String {rovaiMissionStatuses.first {$0.0 == value}?.1 ?? value}

struct RovaiMissionBoard: View {
    let model: RovaiWorkspaceModel
    let onOpen: (String) -> Void
    @State private var status = "", search = ""
    @State private var selected: Selection?
    private struct Selection: Identifiable {let value: JSONValue;var id: String {value.text("missionId")}}
    private var rows: [JSONValue] {model.missions.filter {(status.isEmpty || $0.text("status") == status) && (search.isEmpty || "\($0.text("title")) \($0.text("description")) \(($0["tags"]?.rvArray ?? []).compactMap(\.stringValue).joined(separator: " ")) \($0.text("projectPath"))".localizedCaseInsensitiveContains(search))}}
    var body: some View {
        VStack(spacing: 0) {
            ScrollView(.horizontal, showsIndicators: false) {HStack(spacing: 8) {ForEach([("", "全部")] + rovaiMissionStatuses, id: \.0) {id, title in Button {status = id} label: {Text("\(title) \(model.missions.filter {id.isEmpty || $0.text("status") == id}.count)").font(.caption).padding(.horizontal, 12).padding(.vertical, 8).background(Capsule().fill(status == id ? Theme.accent.opacity(0.22) : Theme.accent.opacity(0.06)))}.buttonStyle(.plain)}}.padding(.horizontal, 12)}
            List {ForEach(rows, id: \.self) {mission in Button {onOpen(mission.text("threadId"))} label: {GlassRow {VStack(alignment: .leading, spacing: 10) {
                HStack {Text(String(format: "M-%03d", mission["number"]?.rvInt ?? 0)).font(.caption).foregroundStyle(Theme.textSecondary);Spacer();Text(missionStatusName(mission.text("status"))).font(.caption);if !(mission["runningAgentIds"]?.rvArray ?? []).isEmpty {Text("执行中").font(.caption)}}
                Text(mission.text("title")).font(.headline);Text(mission.text("description")).lineLimit(3).foregroundStyle(Theme.textSecondary)
                Text(roster(mission)).font(.caption)
                Text("\(mission.text("projectPath").isEmpty ? "快速对话" : mission.text("projectPath")) · \((mission["tags"]?.rvArray ?? []).compactMap(\.stringValue).joined(separator: " · "))").font(.caption2).foregroundStyle(Theme.textSecondary)
                HStack {Button("详情和交付") {selected = Selection(value: mission)};Spacer();if mission["startAvailable"]?.rvBool == true {Button("开始使命") {Task {await model.command(.missionsStart, value: .object(["missionId": mission["missionId"] ?? .null]))}}.disabled(model.pending != nil || model.busy)};if mission["hasUnread"]?.rvBool == true {Text("有新回复").font(.caption).foregroundStyle(Theme.accent)}}
            }}}.buttonStyle(.plain).onLongPressGesture {selected = Selection(value: mission)}.listRowBackground(Color.clear).listRowSeparator(.hidden)}
                if rows.isEmpty {Text("没有符合条件的使命").foregroundStyle(Theme.textSecondary)}
            }.scrollContentBackground(.hidden).searchable(text: $search, prompt: "搜索使命、标签或工程").refreshable {await model.refresh()}
        }.sheet(item: $selected) {selection in RovaiMissionDetail(model: model, initial: selection.value)}
    }
    private func roster(_ mission: JSONValue) -> String {let lead = mission.text("defaultLeadAgentId");let ids = (mission["memberAgentIds"]?.rvArray ?? []).compactMap(\.stringValue);let ordered = (lead.isEmpty ? [] : [lead]) + ids.filter {$0 != lead};return ordered.prefix(5).map {id in model.members.first {$0.text("agentId") == id}?.text("displayName") ?? "不可用队员"}.joined(separator: " · ") + (ordered.count > 5 ? " +\(ordered.count - 5)" : "")}
}

struct RovaiMissionDetail: View {
    let model: RovaiWorkspaceModel
    let initial: JSONValue
    @Environment(\.dismiss) private var dismiss
    @State private var tab = 0, refresh = 0
    @State private var delivery: JSONValue?, changes: JSONValue?, source: JSONValue?
    @State private var history: [JSONValue] = []
    @State private var patches: [String: String] = [:]
    @State private var busy = false, more = false
    @State private var issue: String?
    private var id: String {initial.text("missionId")}
    private var mission: JSONValue {model.missions.first {$0.text("missionId") == id} ?? initial}
    var body: some View {
        NavigationStack {VStack(spacing: 0) {
            Picker("使命内容", selection: $tab) {Text("详情").tag(0);Text("交付").tag(1);Text("活动").tag(2);Text("累计Diff").tag(3)}.pickerStyle(.segmented).padding(12)
            List {
                if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning)}
                if tab == 0 {Section {MarkdownContent(raw: mission.text("description"));Text("状态 · \(missionStatusName(mission.text("status")))");Text(mission.text("projectPath").isEmpty ? "快速对话" : mission.text("projectPath")).font(.caption).foregroundStyle(Theme.textSecondary)};Section("使命附件") {ForEach((mission["attachments"]?.rvArray ?? []).map {RovaiRow(value: $0)}) {row in Button(row.value.text("displayName")) {source = attachment(owner: "mission", id: row.id)}}}}
                if tab == 1, let delivery {Section("工作目录") {Text(delivery.text("workingDirectory")).font(.system(.caption, design: .monospaced));if let workspace = delivery["workspace"], workspace != .null {Text(workspace.text("managedBranch"));Text(workspace.text("state")).font(.caption).foregroundStyle(Theme.textSecondary);if !workspace.text("diagnostic").isEmpty {Text(workspace.text("diagnostic")).foregroundStyle(Theme.warning)}}};Section("关联PR") {ForEach(delivery["pullRequests"]?.rvArray ?? [], id: \.self) {pr in VStack(alignment: .leading) {Text(pr.text("title"));Text(pr.text("url")).font(.caption).foregroundStyle(Theme.textSecondary)}}};Section("交付文件") {ForEach(delivery["files"]?.rvArray ?? [], id: \.self) {file in Button(file.text("displayName")) {source = attachment(owner: "message", id: file.text("attachmentId"), message: file.text("messageId"))}}}}
                if tab == 2 {ForEach(history, id: \.self) {activity in GlassRow {VStack(alignment: .leading, spacing: 6) {Text(missionActivityName(activity));Text(activity.text("createdAt")).font(.caption).foregroundStyle(Theme.textSecondary)}}.listRowBackground(Color.clear).listRowSeparator(.hidden)};if more {Button("更早的活动") {Task {await work {var params: [String: JSONValue] = ["missionId": .string(id)];if let before = history.last?["id"] {params["before"] = before};let result = try await model.query(.missionsActivity, params: .object(params)).rvArray;let ids = Set(history.map {$0["id"]});history += result.filter {!ids.contains($0["id"])};more = result.count == 100}}}.disabled(busy)}}
                if tab == 3, let changes {Section {let checkout = changes["checkoutState"];Text("\(checkout?.text("kind") ?? "") · \(checkout?.text("branch") ?? "")").font(.caption);if !changes.text("diffError").isEmpty {Text(changes.text("diffError")).foregroundStyle(Theme.warning)}};ForEach(changes["files"]?.rvArray ?? [], id: \.self) {file in Section {Button(file.text("path")) {Task {await work {let result = try await model.query(.missionsFileDiff, params: .object(["missionId": .string(id), "fileId": file["id"] ?? .null, "viewId": changes["viewId"] ?? .null]));patches[file.text("id")] = result.text("patch")}}}.disabled(busy);if file["binary"]?.rvBool == true {Text("二进制文件")};if let patch = patches[file.text("id")] {if patch.isEmpty {Text("没有文本行差异")} else {DiffView(files: UnifiedDiff.parse(patch) ?? [])}}}}}
            }.scrollContentBackground(.hidden)
        }.navigationTitle(mission.text("title")).navigationBarTitleDisplayMode(.inline).toolbar {ToolbarItem(placement: .cancellationAction) {Button("关闭") {dismiss()}};ToolbarItem(placement: .topBarTrailing) {Button {refresh += 1} label: {Image(systemName: "arrow.clockwise")}.disabled(busy)}}}
        .task(id: "\(id):\(model.resourceVersion):\(refresh)") {let release = model.retainMissionDiff(id);defer {release()};await work {delivery = try await model.query(.missionsDelivery, params: .object(["missionId": .string(id)]));history = try await model.query(.missionsActivity, params: .object(["missionId": .string(id)])).rvArray;more = history.count == 100};do {try await Task.sleep(for: .seconds(31536000))} catch {}}
        .task(id: tab) {if tab == 3 {await work {changes = try await model.query(.missionsChanges, params: .object(["missionId": .string(id)]));patches = [:]}}}
        .sheet(isPresented: Binding(get: {source != nil}, set: {if !$0 {source = nil}})) {if let source {RovaiFilePreview(model: model, source: source)}}
    }
    private func attachment(owner: String, id: String, message: String? = nil) -> JSONValue {var locator: [String: JSONValue] = ["owner": .string(owner), "threadId": mission["threadId"] ?? .null, "attachmentRefId": .string(id)];if owner == "mission" {locator["missionId"] = .string(self.id)};if let message {locator["messageId"] = .string(message)};return .object(["kind": .string("attachment"), "threadId": mission["threadId"] ?? .null, "locator": .object(locator)])}
    private func work(_ block: () async throws -> Void) async {guard !busy else {return};let generation = model.connectionGeneration;busy = true;defer {busy = false};issue = nil;do {try await block();guard generation == model.connectionGeneration else {throw CancellationError()}} catch is CancellationError {} catch {issue = error.localizedDescription}}
}
func missionActivityName(_ value: JSONValue) -> String {switch value.text("kind") {case "created": return "创建了使命";case "started": return "开始了使命";case "status": return "状态改为“\(missionStatusName(value["changes"]?.text("status") ?? ""))”";case "pull_request": return value["changes"]?["removed"]?.rvBool == true ? "移除了PR关联" : "关联了PR";case "members": return "更新了队员";case "lead": return "调整了队长";default: return "更新了使命定义"}}
