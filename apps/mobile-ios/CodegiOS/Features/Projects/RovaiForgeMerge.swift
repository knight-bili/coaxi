import SwiftUI

struct RovaiForgeMerge: View {
    let model: RovaiWorkspaceModel
    let threadId: String, accountId: String
    let number: Int, repository: JSONValue, detail: JSONValue
    let onChanged: () -> Void
    @State private var open = false, busy = false
    @State private var methods: [String] = []
    @State private var method = "merge", head = "", strategy = "", commandId = ""
    @State private var sent: JSONValue?, issue: String?
    private var owner: String {"forge-merge:\(threadId):\(accountId):\(number)"}
    private var record: JSONValue {.object(["repositoryKey": repository["repositoryKey"] ?? .null, "head": .string(head), "method": .string(method), "methods": .array(methods.map(JSONValue.string)), "strategy": .string(strategy), "commandId": .string(commandId), "sent": sent ?? .null])}
    var body: some View {
        Button("合并PR / MR") {Task {await prepare()}}.disabled(busy || detail.text("head_sha").isEmpty || detail.text("state") != "open" || detail["draft"]?.rvBool == true || model.pending != nil)
        .sheet(isPresented: $open) {NavigationStack {Form {
            Section {Text("\(repository.text("ownerRepo")) #\(number)");Text("只合并这里显示的提交。分支更新后必须重新读取。").font(.caption).foregroundStyle(Theme.textSecondary);Text(head).font(.system(.caption, design: .monospaced)).textSelection(.enabled);if let issue {Text(issue).foregroundStyle(Theme.warning)}}
            Section {Picker("合并方式", selection: $method) {ForEach(methods, id: \.self) {value in Text(value == "squash" ? "压缩提交" : value == "rebase" ? "变基合并" : strategy == "fast_forward" ? "快进合并" : strategy == "rebase_merge" ? "变基后合并" : "创建合并提交").tag(value)}}.disabled(busy || sent != nil)}
            if sent != nil {Text("原合并请求已冻结。核对复用原请求编号，不会改为合并新的提交。").font(.caption).foregroundStyle(Theme.textSecondary)}
            Section {Button(sent == nil ? "确认合并" : "核对原合并请求") {Task {await submit()}}.disabled(busy || head.isEmpty || methods.isEmpty)}
        }.navigationTitle("确认合并PR / MR").toolbar {ToolbarItem(placement: .cancellationAction) {Button("关闭") {open = false}.disabled(busy)}}}.interactiveDismissDisabled(busy)}
    }
    private func params(_ action: String) -> [String: JSONValue] {var value: [String: JSONValue] = ["scope": .object(["threadId": .string(threadId)]), "action": .string(action), "number": .number(Double(number))];if !accountId.isEmpty {value["accountId"] = .string(accountId)};return value}
    private func prepare() async {
        guard !busy else {return};let generation = model.connectionGeneration, capturedHead = detail.text("head_sha");busy = true;open = true;defer {busy = false};issue = nil
        do {let previous = try model.readBody(owner)
            if !previous.isEmpty {let value = try JSONDecoder().decode(JSONValue.self, from: Data(previous.utf8));guard value.text("repositoryKey") == repository.text("repositoryKey") else {throw RovaiHostFailure(code: "仓库身份已变化，已保留原合并请求")};head = value.text("head");method = value.text("method");methods = (value["methods"]?.rvArray ?? []).compactMap(\.stringValue);strategy = value.text("strategy");commandId = value.text("commandId");if let frozen = value["sent"], frozen != .null {sent = frozen}}
            else {guard capturedHead.range(of: "^[0-9a-fA-F]{40,64}$", options: .regularExpression) != nil else {throw RovaiHostFailure(code: "请先读取当前PR的提交")};let result = try await model.query(.forgeOperation, params: .object(params("merge_options")));guard result["success"]?.rvBool == true, generation == model.connectionGeneration else {throw RovaiHostFailure(code: "无法读取合并选项")};let options = result["data"];methods = (options?["methods"]?.rvArray ?? []).compactMap(\.stringValue).filter {["merge", "squash", "rebase"].contains($0)};if methods.isEmpty {methods = ["merge"]};let preferred = options?.text("default_method") ?? "merge";method = methods.contains(preferred) ? preferred : methods[0];strategy = options?.text("merge_strategy") ?? "";head = capturedHead;commandId = UUID().uuidString.lowercased()}
        } catch is CancellationError {} catch {issue = error.localizedDescription}
    }
    private func submit() async {
        guard !busy, !head.isEmpty, !methods.isEmpty else {return};let generation = model.connectionGeneration;busy = true;defer {busy = false};issue = nil
        do {if sent == nil {var command = params("merge_pr");command["headSha"] = .string(head);command["mergeMethod"] = .string(method);command["confirmed"] = .bool(true);command["expectedRepositoryKey"] = repository["repositoryKey"];command["commandId"] = .string(commandId);sent = .object(command)}
            let data = try JSONEncoder().encode(record);try model.saveBody(owner, body: String(decoding: data, as: UTF8.self));let result = try await model.query(.forgeOperation, params: sent!);guard result["success"]?.rvBool == true, generation == model.connectionGeneration else {throw RovaiHostFailure(code: "合并结果需核对，原请求仍保留")};try model.saveBody(owner, body: "");open = false;onChanged()
        } catch is CancellationError {} catch {issue = error.localizedDescription}
    }
}
