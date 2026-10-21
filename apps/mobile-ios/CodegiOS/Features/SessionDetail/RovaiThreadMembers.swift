import SwiftUI

/// Mirrors the original member inspector; the Host's roster and removal preview remain authoritative.
struct RovaiThreadMembers: View {
    let model: RovaiWorkspaceModel
    let threadId: String
    @Environment(\.dismiss) private var dismiss
    @State private var snapshot: JSONValue?, confirmation: JSONValue?, issue: String?, submitted: String?
    @State private var busy = false
    @State private var generation: UInt64?
    @State private var search = ""
    private var blocked: Bool {busy || model.busy || model.pending != nil || generation != model.connectionGeneration}
    private var members: [JSONValue] {(snapshot?["members"]?.rvArray ?? []).filter {$0.text("membershipStatus") == "active"}}
    private var present: Int {members.filter {$0.text("profilePresence") == "present" && ($0["leaveRequestedAt"] == nil || $0["leaveRequestedAt"] == .null)}.count}
    private var candidates: [JSONValue] {let active = Set(members.map {$0.text("agentId")});return model.members.filter {$0.text("presence") == "present" && !active.contains($0.text("agentId")) && (search.isEmpty || ($0.text("displayName") + " " + $0.text("teamRole")).localizedCaseInsensitiveContains(search))}}
    var body: some View {
        NavigationStack {List {
            if let issue {Section {Text(issue).font(.caption).foregroundStyle(Theme.warning)}}
            Section {
                Text("\(present) 位在队 · \(members.count - present) 位暂离").font(.caption).foregroundStyle(Theme.textSecondary)
                Button("重新读取名册") {Task {await load()}}.disabled(blocked)
                ForEach(snapshot?["membershipReconciliations"]?.rvArray ?? [], id: \.self) {value in Text("正在收拢已开始工作 · \(value["settledRunCount"]?.rvInt ?? 0)/\(value["targetRunCount"]?.rvInt ?? 0)；新消息和工具写入已停止。").font(.caption).foregroundStyle(Theme.textSecondary)}
            }
            Section("当前会话队员") {ForEach(members, id: \.self) {member in
                let id = member.text("agentId"), lead = member["isDefaultLead"]?.rvBool == true
                VStack(alignment: .leading, spacing: 8) {
                    HStack {Text(member.text("displayName")).font(.headline);if lead {Text("队长").font(.caption).foregroundStyle(Theme.accent)}}
                    Text(member.text("teamRole")).font(.caption).foregroundStyle(Theme.textSecondary)
                    Text(member["leaveRequestedAt"] != nil && member["leaveRequestedAt"] != .null ? "正在暂离" : member.text("profilePresence") == "present" ? "在队" : "暂离").font(.caption).foregroundStyle(Theme.textSecondary)
                    if let profile = model.members.first(where: {$0.text("agentId") == id}) {Text(profile["runtime"]?.text("adapterKind") ?? "未选择运行程序").font(.caption).foregroundStyle(Theme.textSecondary)}
                    if let fast = member["fast"], fast != .null {
                        Toggle(isOn: Binding(get: {fast["fastOverride"]?.rvBool ?? fast["runtimeDefaultFast"]?.rvBool ?? false}, set: {selected in
                            Task {await model.command(.threadsMembersFastSet, value: .object(["threadId": .string(threadId), "agentId": .string(id), "expectedRuntimeBindingRevision": fast["runtimeBindingRevision"] ?? .null, "fastOverride": .bool(selected)]));await load()}
                        })) {VStack(alignment: .leading, spacing: 4) {Text("Fast");Text("用于之后的新执行；实际档位以运行程序反馈为准。").font(.caption).foregroundStyle(Theme.textSecondary)}}.disabled(blocked || fast["runtimeBindingRevision"] == nil)
                    }
                    HStack {
                        Button("设为队长") {confirmation = prepared("lead", command: .object(["threadId": .string(threadId), "successorAgentId": .string(id), "expectedVersion": snapshot?["thread"]?["version"] ?? .null]), preview: member)}.disabled(blocked || lead || member.text("profilePresence") != "present" || member["leaveRequestedAt"] != nil && member["leaveRequestedAt"] != .null)
                        Spacer()
                        Button(members.count <= 1 ? "至少保留一位队员" : "移出会话", role: .destructive) {Task {await removal(id)}}.disabled(blocked || members.count <= 1)
                    }
                }.padding(.vertical, 4)
            }}
            Section("邀请队员") {
                TextField("搜索姓名或职责", text: $search)
                ForEach(candidates, id: \.self) {candidate in HStack {
                    VStack(alignment: .leading, spacing: 4) {Text(candidate.text("displayName"));Text(candidate.text("teamRole")).font(.caption).foregroundStyle(Theme.textSecondary)};Spacer()
                    Button("邀请") {confirmation = prepared("add", command: .object(["threadId": .string(threadId), "agentId": candidate["agentId"] ?? .null, "expectedMembershipGeneration": snapshot?["thread"]?["membershipGeneration"] ?? .null]), preview: candidate)}.disabled(blocked || snapshot == nil)
                }}
                if candidates.isEmpty {Text("没有符合条件的可邀请队员").foregroundStyle(Theme.textSecondary)}
            } footer: {Text("加入只在后续新执行中生效；不会启动队员或恢复旧执行。")}
        }.navigationTitle("会话队员").navigationBarTitleDisplayMode(.inline).toolbar {Button("关闭") {dismiss()}.disabled(busy)}}
        .task {generation = model.connectionGeneration;await load()}
        .task(id: model.resourceVersion) {if generation != nil {await load()}}
        .onChange(of: model.lastResult) {_, value in
            let receipt = value?["commandResult"] ?? value
            guard let submitted, receipt?.text("commandId") == submitted else {return}
            if ["applied", "accepted"].contains(receipt?.text("status") ?? "") {self.submitted = nil;confirmation = nil;Task {await load()}}
            else if receipt?.text("status") == "rejected" {self.submitted = nil;confirmation = nil;issue = (receipt?.text("code") ?? "") + "；请重新读取名册或移出影响后再确认。"}
        }
        .sheet(isPresented: Binding(get: {confirmation != nil}, set: {if !$0 {confirmation = nil}})) {
            if let confirmation {let kind = confirmation.text("kind"), preview = confirmation["preview"] ?? .null
                NavigationStack {Form {
                    if kind == "remove" {
                        Section("移出影响") {
                            Text("\(preview.text("displayName"))将离开此会话。历史记录保留，旧参与期的执行停止，开放任务回到待分配。")
                            LabeledContent("相关执行", value: "\(preview["nonTerminalAgentRunCount"]?.rvInt ?? 0)")
                            LabeledContent("开放任务", value: "\(preview["openAssignedTaskCount"]?.rvInt ?? 0)")
                            LabeledContent("相关投递", value: "\((preview["pendingDeliveryCount"]?.rvInt ?? 0) + (preview["runningDeliveryCount"]?.rvInt ?? 0))")
                            if preview["isDefaultLead"]?.rvBool == true {let id = preview.text("nextDefaultLeadAgentId");LabeledContent("接任队长", value: members.first {$0.text("agentId") == id}?.text("displayName") ?? "暂缺，等现有成员归队后恢复")}
                            if preview["removable"]?.rvBool != true {Text(preview.text("blockerCode")).foregroundStyle(Theme.warning)}
                        } footer: {Text("名册变更会拒绝本次操作。重新邀请不会恢复旧执行。")}
                    } else {Section {Text(kind == "lead" ? "将\(preview.text("displayName"))设为此会话队长。" : "邀请\(preview.text("displayName"))加入当前会话，不自动发消息或启动运行。")}}
                    if model.pending != nil {Section {Text("原请求结果待核对，可关闭本页后从工作区核对。").font(.caption)}}
                    Section {Button(self.submitted == nil ? "确认" : "原请求待核对", role: kind == "remove" ? .destructive : nil) {Task {await send(confirmation)}}.disabled(blocked || self.submitted != nil || kind == "remove" && preview["removable"]?.rvBool != true)}
                }.navigationTitle(kind == "remove" ? "移出会话？" : kind == "lead" ? "更换队长？" : "邀请队员？").toolbar {Button("关闭") {self.confirmation = nil}}}
            }
        }
    }
    private func prepared(_ kind: String, command: JSONValue, preview: JSONValue) -> JSONValue {.object(["kind": .string(kind), "commandId": .string(UUID().uuidString.lowercased()), "command": command, "preview": preview])}
    private func load() async {
        guard !busy, generation == model.connectionGeneration else {return};busy = true;defer {busy = false}
        do {let value = try await model.query(.threadsOpen, params: .object(["threadId": .string(threadId), "traceId": .string(UUID().uuidString.lowercased())]));guard generation == model.connectionGeneration else {throw CancellationError()};snapshot = value;issue = nil} catch is CancellationError {} catch {issue = error.localizedDescription}
    }
    private func removal(_ id: String) async {
        guard !blocked else {return};busy = true;defer {busy = false}
        do {let value = try await model.query(.threadsMembersRemovalPreview, params: .object(["threadId": .string(threadId), "agentId": .string(id)]));guard generation == model.connectionGeneration else {throw CancellationError()};var command: [String: JSONValue] = ["threadId": .string(threadId), "agentId": .string(id), "expectedMembershipGeneration": value["membershipGeneration"] ?? .null, "expectedMembershipVersion": value["membershipVersion"] ?? .null];if value["isDefaultLead"]?.rvBool == true {command["replacementDefaultLeadAgentId"] = value["nextDefaultLeadAgentId"] ?? .null};confirmation = prepared("remove", command: .object(command), preview: value)} catch is CancellationError {} catch {issue = error.localizedDescription}
    }
    private func send(_ frozen: JSONValue) async {
        guard !blocked, submitted == nil else {return};submitted = frozen.text("commandId")
        let operation: RovaiOperation = frozen.text("kind") == "add" ? .threadsMembersAdd : frozen.text("kind") == "lead" ? .threadsChangeDefaultLead : .threadsMembersRemove
        await model.execute(operation, params: .object(["commandId": frozen["commandId"] ?? .null, "command": frozen["command"] ?? .null]))
    }
}
