import SwiftUI

struct RovaiForgeLinks: View {
    let model: RovaiWorkspaceModel
    let threadId: String, accountId: String, sourceKind: String
    let number: Int, repositoryKey: String
    @State private var page: JSONValue?, pending: JSONValue?, unlinking: JSONValue?, issue: String?
    @State private var missions: [JSONValue] = [], tasks: [JSONValue] = []
    @State private var kind = "thread", target = "", busy = false
    private var owner: String {"forge-link:\(threadId):\(accountId):\(sourceKind):\(number)"}
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("关联工作").font(.headline);if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning)}
            ForEach(page?["links"]?.rvArray ?? [], id: \.self) {link in HStack {Text(label(link));Spacer();Button("移除关联") {unlinking = link}.disabled(busy || pending != nil)}}
            Picker("关联类型", selection: $kind) {Text("当前对话").tag("thread");Text("使命").tag("mission");Text("任务").tag("task")}.disabled(busy || pending != nil).onChange(of: kind) {_, value in target = value == "mission" ? missions.first?.text("missionId") ?? "" : value == "task" ? tasks.first?.text("taskId") ?? "" : threadId}
            if kind != "thread" {Picker("关联工作", selection: $target) {ForEach(kind == "mission" ? missions : tasks, id: \.self) {row in Text(row.text("title")).tag(row.text(kind == "mission" ? "missionId" : "taskId"))}}.disabled(busy || pending != nil)}
            Button(pending == nil ? "关联" : "核对原关联") {Task {await apply("link")}}.disabled(busy || page == nil || pending == nil && target.isEmpty)
        }.task(id: "\(owner):\(model.connectionGeneration):\(repositoryKey)") {await prepare()}
        .confirmationDialog("移除这项关联？", isPresented: Binding(get: {unlinking != nil}, set: {if !$0 {unlinking = nil}}), titleVisibility: .visible) {Button("移除关联", role: .destructive) {let chosen = unlinking;Task {await apply("unlink", targetKind: chosen?.text("targetKind"), targetId: chosen?.text("targetId"))}};Button("取消", role: .cancel) {unlinking = nil}} message: {Text("保留仓库记录和原工作，只移除来源关系。")}
    }
    private func args(_ operation: String) -> [String: JSONValue] {var value: [String: JSONValue] = ["scope": .object(["threadId": .string(threadId)]), "operation": .string(operation), "sourceKind": .string(sourceKind), "number": .number(Double(number))];if !accountId.isEmpty {value["accountId"] = .string(accountId)};return value}
    private func load() async throws {page = try await model.query(.forgeLinks, params: .object(args("list")))}
    private func prepare() async {
        guard !busy else {return};let generation = model.connectionGeneration;busy = true;defer {busy = false};issue = nil
        do {target = threadId;let previous = try model.readBody(owner);if !previous.isEmpty {let value = try JSONDecoder().decode(JSONValue.self, from: Data(previous.utf8));guard value.text("expectedRepositoryKey") == repositoryKey else {throw RovaiHostFailure(code: "仓库已变化，请在原仓库核对原关联")};pending = value};let thread = try await model.query(.threadsOpen, params: .object(["threadId": .string(threadId), "traceId": .string(UUID().uuidString.lowercased())]))["thread"];let project = thread?.text("projectPath") ?? "";missions = try await model.query(.missionsList).rvArray.filter {$0.text("projectPath") == project};tasks = try await model.query(.tasksList, params: .object(["threadId": .string(threadId), "limit": .number(200)]))["tasks"]?.rvArray ?? [];try await load();guard generation == model.connectionGeneration else {throw CancellationError()}} catch is CancellationError {} catch {issue = error.localizedDescription}
    }
    private func label(_ link: JSONValue) -> String {let id = link.text("targetId");switch link.text("targetKind") {case "mission": return missions.first {$0.text("missionId") == id}?.text("title") ?? "原使命暂不可用";case "task": return tasks.first {$0.text("taskId") == id}?.text("title") ?? "原任务暂不可用";default: return "当前对话"}}
    private func apply(_ operation: String, targetKind: String? = nil, targetId: String? = nil) async {
        guard !busy, let page else {return};let generation = model.connectionGeneration;busy = true;defer {busy = false};issue = nil
        do {if pending == nil {var command = args(operation);command["targetKind"] = .string(targetKind ?? kind);command["targetId"] = .string(targetId ?? target);command["expectedRevision"] = page["revision"];command["expectedRepositoryKey"] = .string(repositoryKey);command["commandId"] = .string(UUID().uuidString.lowercased());pending = .object(command)};let data = try JSONEncoder().encode(pending!);try model.saveBody(owner, body: String(decoding: data, as: UTF8.self));let result = try await model.query(.forgeLinks, params: pending!);guard result["applied"]?.rvBool == true, generation == model.connectionGeneration else {throw RovaiHostFailure(code: "关联结果尚未核对，原请求仍保留")};try model.saveBody(owner, body: "");pending = nil;unlinking = nil;try await load()} catch is CancellationError {} catch {issue = error.localizedDescription}
    }
}
