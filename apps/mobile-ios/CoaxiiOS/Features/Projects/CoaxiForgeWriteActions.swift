import SwiftUI

struct CoaxiForgeWriteActions: View {
    let model: CoaxiWorkspaceModel
    let threadId: String, accountId: String, kind: String
    let repository: JSONValue?, selected: JSONValue?
    let onChanged: () -> Void
    @State private var editor: Editor?
    private struct Editor: Identifiable {let id = UUID();let action: String}
    var body: some View {
        HStack {if selected == nil {Button(kind == "pulls" ? "新建PR / MR" : "新建议题") {editor = Editor(action: kind == "pulls" ? "create_pr" : "create_issue")}} else {Button("发表评论") {editor = Editor(action: kind == "pulls" ? "comment_pr" : "comment_issue")};Spacer();Button("编辑") {editor = Editor(action: kind == "pulls" ? "update_pr" : "update_issue")}}}.disabled(repository == nil || model.pending != nil)
        .sheet(item: $editor) {target in if let repository {CoaxiForgeWriteEditor(model: model, threadId: threadId, accountId: accountId, action: target.action, repository: repository, selected: selected) {editor = nil;onChanged()}}}
    }
}

private struct CoaxiForgeWriteEditor: View {
    let model: CoaxiWorkspaceModel
    let threadId: String, accountId: String, action: String
    let repository: JSONValue, selected: JSONValue?
    let onSaved: () -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var title = ""
    @State private var bodyText = ""
    @State private var head = ""
    @State private var base = ""
    @State private var status = "open"
    @State private var commandId = UUID().uuidString.lowercased()
    @State private var draft = false
    @State private var ready = false
    @State private var busy = false
    @State private var confirming = false
    @State private var sent: JSONValue?
    @State private var issue: String?
    private var owner: String {"forge-write:\(threadId):\(accountId):\(action):\(selected?.forgeNumber ?? 0)"}
    private var record: JSONValue {.object(["title": .string(title), "body": .string(bodyText), "head": .string(head), "base": .string(base), "draft": .bool(draft), "state": .string(status), "commandId": .string(commandId), "sent": sent ?? .null, "repositoryKey": repository["repositoryKey"] ?? .null])}
    private var enabled: Bool {ready && !busy && (sent != nil || (action.hasPrefix("comment_") ? !bodyText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty : !title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && (action != "create_pr" || !head.isEmpty && !base.isEmpty)))}
    var body: some View {
        NavigationStack {Form {
            Section {Text(repository.text("webUrl")).font(.caption).foregroundStyle(Theme.textSecondary);if let issue {Text(issue).foregroundStyle(Theme.warning)};if !ready {ProgressView("正在恢复草稿…")}}
            Section {if !action.hasPrefix("comment_") {TextField("标题", text: $title)};TextField("正文", text: $bodyText, axis: .vertical).lineLimit(6...20);if action == "create_pr" {TextField("来源分支", text: $head).textInputAutocapitalization(.never).autocorrectionDisabled();TextField("目标分支", text: $base).textInputAutocapitalization(.never).autocorrectionDisabled();Toggle("草稿", isOn: $draft)};if action.hasPrefix("update_") {Picker("状态", selection: $status) {Text("开放").tag("open");Text("关闭").tag("closed")}}}.disabled(!ready || busy || sent != nil)
            if sent != nil {Section {Text("原请求已冻结。核对时复用同一请求编号；结果未知时请先到远端核对，关闭会保留原请求。").font(.caption).foregroundStyle(Theme.textSecondary)}}
            Section {Button(sent == nil ? "确认提交" : "核对原操作") {confirming = true}.disabled(!enabled)}
        }.navigationTitle(action.hasPrefix("comment_") ? "发表评论" : action.hasPrefix("update_") ? "编辑远端记录" : "新建远端记录").toolbar {ToolbarItem(placement: .cancellationAction) {Button("关闭，保留草稿") {dismiss()}.disabled(busy)}}}
        .interactiveDismissDisabled(busy)
        .task {await restore()}
        .task(id: record) {guard ready, !busy else {return};do {try await Task.sleep(for: .milliseconds(350));try persist()} catch is CancellationError {} catch {issue = error.localizedDescription}}
        .onDisappear {if ready {try? persist()}}
        .confirmationDialog(sent == nil ? "提交到远程仓库？" : "核对这次原操作？", isPresented: $confirming, titleVisibility: .visible) {Button("确认") {Task {await submit()}};Button("取消", role: .cancel) {}} message: {Text("\(repository.text("ownerRepo"))\(selected.map {" #\($0.forgeNumber)"} ?? "")\n将以所选账号提交。请求提交后冻结；失败或断线保留原请求。")}
    }
    private func restore() async {
        guard !ready else {return}
        do {
            if action.hasPrefix("update_") {title = selected?.text("title") ?? "";bodyText = selected?.forgeBody ?? "";status = selected?.text("state") == "closed" ? "closed" : "open"}
            let raw = try model.readBody(owner)
            if !raw.isEmpty {let value = try JSONDecoder().decode(JSONValue.self, from: Data(raw.utf8));guard value.text("repositoryKey") == repository.text("repositoryKey") else {throw CoaxiHostFailure(code: "工程远程仓库已变化，保留旧草稿；请在原仓库核对原操作")};title = value.text("title");bodyText = value.text("body");head = value.text("head");base = value.text("base");draft = value["draft"]?.rvBool == true;status = value.text("state");if !value.text("commandId").isEmpty {commandId = value.text("commandId")};if let frozen = value["sent"], frozen != .null {sent = frozen}}
            ready = true
        } catch {issue = error.localizedDescription}
    }
    private func persist() throws {let data = try JSONEncoder().encode(record);guard let text = String(data: data, encoding: .utf8) else {throw CoaxiHostFailure(code: "草稿编码失败")};try model.saveBody(owner, body: text)}
    private func submit() async {
        guard enabled else {return};let generation = model.connectionGeneration;busy = true;defer {busy = false};issue = nil
        do {
            if sent == nil {var params: [String: JSONValue] = ["scope": .object(["threadId": .string(threadId)]), "action": .string(action), "title": .string(title), "body": .string(bodyText), "commandId": .string(commandId), "confirmed": .bool(true), "expectedRepositoryKey": repository["repositoryKey"] ?? .null];if !accountId.isEmpty {params["accountId"] = .string(accountId)};if let selected {params["number"] = .number(Double(selected.forgeNumber))};if action.hasPrefix("update_") {params["state"] = .string(status)};if action == "create_pr" {params["head"] = .string(head);params["base"] = .string(base);params["draft"] = .bool(draft)};sent = .object(params)}
            try persist();let result = try await model.query(.forgeOperation, params: sent!);guard generation == model.connectionGeneration else {throw CoaxiHostFailure(code: "工作区已切换，原请求已保留")};guard result["success"]?.rvBool == true else {throw CoaxiHostFailure(code: "远端返回状态 \(result["status"]?.rvInt ?? 0)，原请求已保留，请核对远端结果")};ready = false;try model.saveBody(owner, body: "");onSaved()
        } catch is CancellationError {} catch {issue = error.localizedDescription}
    }
}
