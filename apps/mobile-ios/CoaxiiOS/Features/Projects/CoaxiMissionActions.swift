import SwiftUI

struct CoaxiMissionActions: View {
    let model: CoaxiWorkspaceModel
    let mission: JSONValue
    let onDeleted: () -> Void
    @State private var editing = false
    @State private var lead = false
    @State private var busy = false
    @State private var cleanup = false
    @State private var confirmation: Confirmation?
    @State private var delivery: JSONValue?
    @State private var issue: String?
    private struct Confirmation: Identifiable {let id = UUID();let kind: String}
    private var blocked: Bool {model.pending != nil || model.busy || busy}
    private var id: String {mission.text("missionId")}
    private var threadId: String {mission.text("threadId")}
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Menu {ForEach(coaxiMissionStatuses, id: \.0) {value, title in Button(title) {Task {await model.command(.missionsStatus, value: .object(["missionId": .string(id), "status": .string(value), "sourceMessageId": .null]))}}}} label: {Label(missionStatusName(mission.text("status")), systemImage: "chevron.down")}.disabled(blocked)
            HStack {Button("编辑使命") {editing = true};Spacer();Button("调整队长") {lead = true}}.disabled(blocked)
            HStack {if mission["cleanupAvailable"]?.rvBool == true || mission["workspaceCleanup"]?.text("state") == "failed" {Button("清理Worktree") {Task {await prepare("cleanup")}}};Spacer();Button("删除使命", role: .destructive) {Task {await prepare("delete")}}}.disabled(blocked)
            if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning)}
        }
        .sheet(isPresented: $editing) {CoaxiMissionEdit(model: model, mission: mission)}
        .sheet(isPresented: $lead) {NavigationStack {List {let ids = Set((mission["memberAgentIds"]?.rvArray ?? []).compactMap(\.stringValue));ForEach(model.members.filter {ids.contains($0.text("agentId"))}.map {CoaxiRow(value: $0)}) {row in Button("\(row.value.text("displayName"))\(row.id == mission.text("defaultLeadAgentId") ? " · 当前队长" : "")") {Task {await changeLead(row.id)}}.disabled(blocked || row.id == mission.text("defaultLeadAgentId"))}}.navigationTitle("调整使命队长").toolbar {Button("关闭") {lead = false}.disabled(busy)}}}
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
