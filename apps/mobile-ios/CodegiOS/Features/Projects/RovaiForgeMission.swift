import SwiftUI

struct RovaiForgeMission: View {
    let model: RovaiWorkspaceModel
    let threadId: String, accountId: String, sourceKind: String
    let number: Int, repository: JSONValue
    let onSaved: () -> Void
    @State private var open = false
    var body: some View {Button("从此记录创建使命") {open = true}.disabled(model.pending != nil).sheet(isPresented: $open) {RovaiForgeMissionEditor(model: model, threadId: threadId, accountId: accountId, sourceKind: sourceKind, number: number, repository: repository) {open = false;onSaved()}}}
}
private struct RovaiForgeMissionEditor: View {
    let model: RovaiWorkspaceModel
    let threadId: String, accountId: String, sourceKind: String
    let number: Int, repository: JSONValue
    let onSaved: () -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var title = ""
    @State private var description = ""
    @State private var sourceURL = ""
    @State private var project = ""
    @State private var lead = ""
    @State private var created = ""
    @State private var commandId = UUID().uuidString.lowercased()
    @State private var members: [JSONValue] = []
    @State private var selected = Set<String>()
    @State private var sent: JSONValue?
    @State private var link: JSONValue?
    @State private var issue: String?
    @State private var ready = false
    @State private var busy = false
    private var owner: String {"forge-mission:\(threadId):\(accountId):\(sourceKind):\(number)"}
    private var record: JSONValue {.object(["repositoryKey": repository["repositoryKey"] ?? .null, "title": .string(title), "description": .string(description), "sourceUrl": .string(sourceURL), "project": .string(project), "members": .array(members), "selected": .array(selected.sorted().map(JSONValue.string)), "lead": .string(lead), "commandId": .string(commandId), "sent": sent ?? .null, "link": link ?? .null, "createdId": .string(created)])}
    var body: some View {
        NavigationStack {Form {
            if let issue {Text(issue).foregroundStyle(Theme.warning)};Section {Text(sourceURL).font(.caption).foregroundStyle(Theme.textSecondary);TextField("使命名称", text: $title);TextField("任务说明", text: $description, axis: .vertical).lineLimit(5...18);LabeledContent("当前工程", value: project)}.disabled(!ready || busy || sent != nil)
            Section("队员与负责人") {ForEach(members.map {RovaiRow(value: $0)}) {row in Toggle(row.value.text("displayName"), isOn: Binding(get: {selected.contains(row.id)}, set: {value in if value {selected.insert(row.id)} else {selected.remove(row.id)};if !selected.contains(lead) {lead = selected.sorted().first ?? ""}}))};Picker("负责人", selection: $lead) {ForEach(members.filter {selected.contains($0.text("agentId"))}.map {RovaiRow(value: $0)}) {row in Text(row.value.text("displayName")).tag(row.id)}}}.disabled(!ready || busy || sent != nil)
            Section {Text(created.isEmpty ? "先保存使命和来源关系，由你决定何时开始。" : "使命已保存，继续核对原来源关联。").font(.caption).foregroundStyle(Theme.textSecondary);Button(sent == nil ? "确认保存使命" : "核对原创建和关联") {Task {await submit()}}.disabled(!ready || busy || title.isEmpty || selected.isEmpty || !selected.contains(lead))}
        }.navigationTitle("从仓库记录创建使命").toolbar {ToolbarItem(placement: .cancellationAction) {Button("关闭，保留进度") {dismiss()}.disabled(busy)}}}.interactiveDismissDisabled(busy)
        .task {await prepare()}
        .task(id: record) {guard ready, !busy else {return};do {try await Task.sleep(for: .milliseconds(350));try persist()} catch is CancellationError {} catch {issue = error.localizedDescription}}
        .onDisappear {if ready {try? persist()}}
    }
    private func args() -> [String: JSONValue] {var value: [String: JSONValue] = ["scope": .object(["threadId": .string(threadId)]), "sourceKind": .string(sourceKind), "number": .number(Double(number))];if !accountId.isEmpty {value["accountId"] = .string(accountId)};return value}
    private func persist() throws {let data = try JSONEncoder().encode(record);try model.saveBody(owner, body: String(decoding: data, as: UTF8.self))}
    private func prepare() async {
        guard !ready, !busy else {return};let generation = model.connectionGeneration;busy = true;defer {busy = false}
        do {let raw = try model.readBody(owner);if !raw.isEmpty {let value = try JSONDecoder().decode(JSONValue.self, from: Data(raw.utf8));guard value.text("repositoryKey") == repository.text("repositoryKey") else {throw RovaiHostFailure(code: "远程仓库已变化，请在原仓库核对已保留的使命操作")};title = value.text("title");description = value.text("description");sourceURL = value.text("sourceUrl");project = value.text("project");members = value["members"]?.rvArray ?? [];selected = Set((value["selected"]?.rvArray ?? []).compactMap(\.stringValue));lead = value.text("lead");commandId = value.text("commandId");created = value.text("createdId");if let frozen = value["sent"], frozen != .null {sent = frozen};if let frozen = value["link"], frozen != .null {link = frozen}}
            else {let snapshot = try await model.query(.threadsOpen, params: .object(["threadId": .string(threadId), "traceId": .string(UUID().uuidString.lowercased())]));guard let thread = snapshot["thread"], thread.text("projectBindingKind") == "directory", !thread.text("projectPath").isEmpty else {throw RovaiHostFailure(code: "此对话没有可关联的目录工程")};project = thread.text("projectPath");members = snapshot["members"]?.rvArray ?? [];selected = Set(members.map {$0.text("agentId")}.filter {!$0.isEmpty});lead = selected.contains(thread.text("defaultLeadAgentId")) ? thread.text("defaultLeadAgentId") : selected.sorted().first ?? "";var request = args();request.removeValue(forKey: "sourceKind");request["action"] = .string("mission_draft_\(sourceKind)");let source = try await model.query(.forgeOperation, params: .object(request));guard source["success"]?.rvBool == true, source["repository"]?.text("repositoryKey") == repository.text("repositoryKey") else {throw RovaiHostFailure(code: "仓库记录来源发生变化")};title = source["missionDraft"]?.text("title") ?? "";description = source["missionDraft"]?.text("description") ?? "";sourceURL = source["missionDraft"]?.text("sourceUrl") ?? ""};guard generation == model.connectionGeneration else {throw CancellationError()};ready = true
        } catch is CancellationError {} catch {issue = error.localizedDescription}
    }
    private func submit() async {
        guard ready, !busy else {return};let generation = model.connectionGeneration;busy = true;defer {busy = false};issue = nil
        do {if sent == nil {sent = .object(["commandId": .string(commandId), "command": .object(["title": .string(title), "description": .string(description), "projectPath": .string(project), "projectBindingKind": .string("directory"), "memberAgentIds": .array(selected.sorted().map(JSONValue.string)), "defaultLeadAgentId": .string(lead), "tags": .array([.string("forge:\(repository.text("provider"))")])])])};try persist()
            if created.isEmpty {let result = try await model.query(.missionsCreate, params: sent!);let receipt = result["commandResult"] ?? result;guard ["applied", "accepted"].contains(receipt.text("status")), generation == model.connectionGeneration else {throw RovaiHostFailure(code: "使命创建结果需核对，原请求已保留")};created = receipt["payload"]?.text("missionId") ?? "";if created.isEmpty {created = receipt["payload"]?["mission"]?.text("id") ?? ""};guard !created.isEmpty else {throw RovaiHostFailure(code: "使命已返回，但尚未核对到使命编号")};try persist()}
            if link == nil {var list = args();list["operation"] = .string("list");let page = try await model.query(.forgeLinks, params: .object(list));var command = args();command["operation"] = .string("link");command["targetKind"] = .string("mission");command["targetId"] = .string(created);command["expectedRevision"] = page["revision"];command["expectedRepositoryKey"] = repository["repositoryKey"];command["commandId"] = .string(UUID().uuidString.lowercased());link = .object(command)};try persist();let result = try await model.query(.forgeLinks, params: link!);guard result["applied"]?.rvBool == true, generation == model.connectionGeneration else {throw RovaiHostFailure(code: "使命已保留，来源关联仍需核对")};try model.saveBody(owner, body: "");ready = false;await model.refresh();onSaved()
        } catch is CancellationError {} catch {issue = error.localizedDescription}
    }
}
