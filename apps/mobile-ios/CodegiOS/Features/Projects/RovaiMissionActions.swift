import SwiftUI

struct RovaiMissionActions: View {
    let model: RovaiWorkspaceModel
    let mission: JSONValue
    let onDeleted: () -> Void
    @State private var editing = false, lead = false, busy = false, cleanup = false
    @State private var confirmation: Confirmation?, delivery: JSONValue?, issue: String?
    private struct Confirmation: Identifiable {let id = UUID();let kind: String}
    private var blocked: Bool {model.pending != nil || model.busy || busy}
    private var id: String {mission.text("missionId")}
    private var threadId: String {mission.text("threadId")}
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Menu {ForEach(rovaiMissionStatuses, id: \.0) {value, title in Button(title) {Task {await model.command(.missionsStatus, value: .object(["missionId": .string(id), "status": .string(value), "sourceMessageId": .null]))}}}} label: {Label(missionStatusName(mission.text("status")), systemImage: "chevron.down")}.disabled(blocked)
            HStack {Button("编辑使命") {editing = true};Spacer();Button("调整队长") {lead = true}}.disabled(blocked)
            HStack {if mission["cleanupAvailable"]?.rvBool == true || mission["workspaceCleanup"]?.text("state") == "failed" {Button("清理Worktree") {Task {await prepare("cleanup")}}};Spacer();Button("删除使命", role: .destructive) {Task {await prepare("delete")}}}.disabled(blocked)
            if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning)}
        }
        .sheet(isPresented: $editing) {RovaiMissionEdit(model: model, mission: mission)}
        .sheet(isPresented: $lead) {NavigationStack {List {let ids = Set((mission["memberAgentIds"]?.rvArray ?? []).compactMap(\.stringValue));ForEach(model.members.filter {ids.contains($0.text("agentId"))}.map {RovaiRow(value: $0)}) {row in Button("\(row.value.text("displayName"))\(row.id == mission.text("defaultLeadAgentId") ? " · 当前队长" : "")") {Task {await changeLead(row.id)}}.disabled(blocked || row.id == mission.text("defaultLeadAgentId"))}}.navigationTitle("调整使命队长").toolbar {Button("关闭") {lead = false}.disabled(busy)}}}
        .sheet(item: $confirmation) {target in NavigationStack {Form {
            Section {Text(target.kind == "delete" ? "使命、会话和交付文件将一并删除，正在执行的队员会停止。此操作无法撤销。" : "将清理此使命的Worktree和本地分支。主工程保留。");if let workspace = delivery?["workspace"], workspace != .null {Text(workspace.text("worktreePath")).font(.system(.caption, design: .monospaced));Text(workspace.text("managedBranch")).font(.caption)}}
            if target.kind == "delete", mission["workspaceEverCreated"]?.rvBool == true {Section {Toggle("同时清理Worktree和本地分支", isOn: $cleanup).disabled(mission["workspaceResourcesPresent"]?.rvBool != true)} footer: {Text("未勾选时，Worktree和本地分支保留在原位置。")}}
            if let issue {Text(issue).foregroundStyle(Theme.warning)}
            Section {Button(target.kind == "delete" ? "确认删除使命" : "确认清理", role: .destructive) {Task {await perform(target.kind)}}.disabled(blocked || delivery == nil || target.kind == "cleanup" && (delivery?["workspace"] == nil || delivery?["workspace"] == .null))}
        }.navigationTitle(target.kind == "delete" ? "删除使命" : "清理使命Worktree").toolbar {ToolbarItem(placement: .cancellationAction) {Button("取消") {confirmation = nil}.disabled(busy)}}}.interactiveDismissDisabled(busy)}
    }
    private func prepare(_ kind: String) async {guard !blocked else {return};busy = true;defer {busy = false};let generation = model.connectionGeneration;delivery = nil;cleanup = false;issue = nil;confirmation = Confirmation(kind: kind);do {let result = try await model.query(.missionsDelivery, params: .object(["missionId": .string(id)]));guard generation == model.connectionGeneration else {throw CancellationError()};delivery = result} catch is CancellationError {} catch {issue = error.localizedDescription}}
    private func changeLead(_ successor: String) async {
        guard !blocked else {return};let generation = model.connectionGeneration;busy = true;defer {busy = false};issue = nil
        do {let current = try await model.query(.threadsOpen, params: .object(["threadId": .string(threadId), "traceId": .string(UUID().uuidString.lowercased())]));guard generation == model.connectionGeneration else {throw CancellationError()};await model.command(.threadsChangeDefaultLead, value: .object(["threadId": .string(threadId), "successorAgentId": .string(successor), "expectedVersion": current["thread"]?["version"] ?? .number(0)]));if model.pending == nil {lead = false}} catch is CancellationError {} catch {issue = error.localizedDescription}
    }
    private func perform(_ kind: String) async {
        guard !blocked else {return};let generation = model.connectionGeneration;busy = true;defer {busy = false};issue = nil
        do {if kind == "cleanup" {await model.command(.missionsWorkspaceCleanup, value: .object(["missionId": .string(id)]));if model.pending == nil {confirmation = nil}}
            else {let current = try await model.query(.threadsOpen, params: .object(["threadId": .string(threadId), "traceId": .string(UUID().uuidString.lowercased())]));guard generation == model.connectionGeneration else {throw CancellationError()};await model.command(.threadsDelete, value: .object(["threadId": .string(threadId), "expectedVersion": current["thread"]?["version"] ?? .number(0), "force": .bool(true), "workspaceDisposition": .string(cleanup ? "cleanup" : "retain")]));let receipt = model.lastResult?["commandResult"] ?? model.lastResult;if model.pending == nil, model.lastOperation == .threadsDelete, ["applied", "accepted"].contains(receipt?.text("status") ?? "") {confirmation = nil;onDeleted()}}
        } catch is CancellationError {} catch {issue = error.localizedDescription}
    }
}

private struct RovaiMissionEdit: View {
    let model: RovaiWorkspaceModel
    let mission: JSONValue
    @Environment(\.dismiss) private var dismiss
    @State private var title = "", description = "", tags = "", commandId = UUID().uuidString.lowercased()
    @State private var baseline: JSONValue?, sent: JSONValue?, issue: String?
    @State private var ready = false, busy = false
    private var owner: String {"mission-edit:\(mission.text("missionId"))"}
    private var record: JSONValue {.object(["title": .string(title), "description": .string(description), "tags": .string(tags), "baseline": baseline ?? mission, "commandId": .string(commandId), "sent": sent ?? .null])}
    var body: some View {
        NavigationStack {Form {if let issue {Text(issue).foregroundStyle(Theme.warning)};Section {TextField("使命名称", text: $title);TextField("使命描述", text: $description, axis: .vertical).lineLimit(4...12);TextField("标签，逗号分隔", text: $tags)}.disabled(!ready || busy || sent != nil);if sent != nil {Text("原保存请求已保留，请先核对原操作。").font(.caption).foregroundStyle(Theme.textSecondary)}}.navigationTitle("编辑使命").toolbar {ToolbarItem(placement: .cancellationAction) {Button("关闭，保留草稿") {dismiss()}.disabled(busy)};ToolbarItem(placement: .confirmationAction) {Button(sent == nil ? "保存" : "核对原保存") {Task {await submit()}}.disabled(!ready || busy || model.pending != nil || title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)}}}
        .interactiveDismissDisabled(busy)
        .task {restore()}
        .task(id: record) {guard ready, !busy else {return};do {try await Task.sleep(for: .milliseconds(350));try persist()} catch is CancellationError {} catch {issue = error.localizedDescription}}
        .onDisappear {if ready {try? persist()}}
        .onChange(of: model.lastResult) {_, result in let receipt = result?["commandResult"] ?? result;if sent != nil, receipt?.text("commandId") == commandId, ["applied", "accepted"].contains(receipt?.text("status") ?? "") {ready = false;try? model.saveBody(owner, body: "");dismiss()}}
    }
    private func restore() {guard !ready else {return};do {title = mission.text("title");description = mission.text("description");tags = (mission["tags"]?.rvArray ?? []).compactMap(\.stringValue).joined(separator: ", ");baseline = mission;let previous = try model.readBody(owner);if !previous.isEmpty {let value = try JSONDecoder().decode(JSONValue.self, from: Data(previous.utf8));title = value.text("title");description = value.text("description");tags = value.text("tags");baseline = value["baseline"];commandId = value.text("commandId");if let frozen = value["sent"], frozen != .null {sent = frozen}};ready = true} catch {issue = error.localizedDescription}}
    private func persist() throws {let data = try JSONEncoder().encode(record);try model.saveBody(owner, body: String(decoding: data, as: UTF8.self))}
    private func submit() async {
        guard ready, !busy, model.pending == nil else {return};let generation = model.connectionGeneration;busy = true;defer {busy = false};issue = nil
        do {if sent == nil {var command: [String: JSONValue] = ["missionId": mission["missionId"] ?? .null, "expectedDetailsVersion": baseline?["detailsVersion"] ?? .number(0), "tags": .array(Array(Set(tags.components(separatedBy: CharacterSet(charactersIn: ",，")).map {$0.trimmingCharacters(in: .whitespacesAndNewlines)}.filter {!$0.isEmpty})).sorted().map(JSONValue.string))];if title != baseline?.text("title") {command["title"] = .string(title)};if description != baseline?.text("description") {command["description"] = .string(description)};sent = .object(["commandId": .string(commandId), "command": .object(command)])};try persist();guard generation == model.connectionGeneration else {throw CancellationError()};await model.execute(.missionsUpdate, params: sent!)} catch is CancellationError {} catch {issue = error.localizedDescription}
    }
}
