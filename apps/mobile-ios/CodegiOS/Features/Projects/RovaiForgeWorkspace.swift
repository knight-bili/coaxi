import SwiftUI

extension JSONValue {
    var forgeNumber: Int {let iid = self["iid"]?.rvInt ?? 0, number = self["number"]?.rvInt ?? 0;return iid > 0 ? iid : number > 0 ? number : self["index"]?.rvInt ?? 0}
    var forgeBody: String {text("body").isEmpty ? text("description") : text("body")}
}

/// Codeg Forge issue rows/detail sheet using the existing native controls.
struct RovaiForgeWorkspace: View {
    let model: RovaiWorkspaceModel
    let threadId: String
    @State private var accounts: [JSONValue] = []
    @State private var rows: [JSONValue] = []
    @State private var comments: [JSONValue] = []
    @State private var files: [JSONValue] = []
    @State private var account = ""
    @State private var kind = "issues"
    @State private var status = "open"
    @State private var search = ""
    @State private var sort = "newest"
    @State private var labels = ""
    @State private var assigned = false
    @State private var busy = false
    @State private var incomplete = false
    @State private var repository: JSONValue?
    @State private var selected: JSONValue?
    @State private var detail: JSONValue?
    @State private var filters: JSONValue?
    @State private var page = 1
    @State private var next: Int?
    @State private var commentNext: Int?
    @State private var fileNext: Int? = 1
    @State private var total: Int?
    @State private var reachable: Int?
    @State private var refreshRevision = 0
    @State private var issue: String?
    private var identity: String {"\(threadId):\(model.connectionGeneration):\(account):\(kind):\(refreshRevision)"}
    var body: some View {
        List {
            if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning)}
            Section {Picker("仓库账号", selection: $account) {Text("按远程仓库匹配").tag("");ForEach(accounts.map {RovaiRow(value: $0)}) {row in Text("\(row.value.text("provider")) · \(row.value.text("baseUrl"))").tag(row.id)}}.disabled(busy)} header: {Text(repository?.text("ownerRepo") ?? "仓库面板")}
            Section {RovaiForgeAccounts(model: model) {refreshRevision += 1}}
            Section {RovaiForgeWriteActions(model: model, threadId: threadId, accountId: account, kind: kind, repository: repository, selected: selected) {refreshRevision += 1}}
            if let selected {
                Section {Button("返回列表") {self.selected = nil;detail = nil;files = [];fileNext = 1}.disabled(busy);Text("#\(selected.forgeNumber) \(selected.text("title"))").font(.headline);MarkdownContent(raw: selected.forgeBody)}
                if let detail {
                    if let repository {Section {RovaiForgeMerge(model: model, threadId: threadId, accountId: account, number: selected.forgeNumber, repository: repository, detail: detail) {refreshRevision += 1}}}
                    Section {Text("\(detail.text("head_ref")) → \(detail.text("base_ref"))").font(.system(.caption, design: .monospaced));Text("\(detail.text("state")) · \(detail.text("head_sha").prefix(12))").font(.caption).foregroundStyle(Theme.textSecondary)}
                    Section("检查") {let checks = detail["checks"]
                        if checks?["available"]?.rvBool != true {Text("未能读取检查状态").foregroundStyle(Theme.textSecondary)}
                        if checks?["partial"]?.rvBool == true {Text("仅取得部分检查状态").foregroundStyle(Theme.textSecondary)}
                        ForEach((checks?["checks"]?.rvArray ?? []).map {RovaiRow(value: $0)}) {row in GlassRow {VStack(alignment: .leading, spacing: 6) {Text(row.value.text("name")).font(.subheadline);Text("\(row.value.text("state"))\(row.value["allow_failure"]?.rvBool == true ? " · 允许失败" : "")").font(.caption).foregroundStyle(Theme.textSecondary);if !row.value.text("summary").isEmpty {Text(row.value.text("summary")).font(.caption)}}}}
                    }
                    Section("变更文件") {ForEach(files, id: \.self) {file in DisclosureGroup {if file["binary"]?.rvBool == true {Text("二进制文件")} else if file.text("patch").isEmpty {Text("平台未提供文本差异")} else {DiffView(files: UnifiedDiff.parse("diff --git a/\(file.text("previous_path").isEmpty ? file.text("path") : file.text("previous_path")) b/\(file.text("path"))\n--- a/\(file.text("previous_path").isEmpty ? file.text("path") : file.text("previous_path"))\n+++ b/\(file.text("path"))\n\(file.text("patch"))") ?? [])}} label: {Text(file.text("path")).font(.system(.caption, design: .monospaced))}}
                        if let fileNext {Button(files.isEmpty ? "读取变更文件" : "更多变更文件") {Task {await work {let result = try await call("change_files", extra: ["number": .number(Double(selected.forgeNumber)), "page": .number(Double(fileNext))]);let values = result["data"]?["files"]?.rvArray ?? [];files = fileNext == 1 ? values : files + values;self.fileNext = result["nextPage"]?.rvInt}}}.disabled(busy)}
                    }
                }
                if let repository {Section {RovaiForgeMission(model: model, threadId: threadId, accountId: account, sourceKind: kind == "pulls" ? "pr" : "issue", number: selected.forgeNumber, repository: repository) {refreshRevision += 1}}}
                if let repository {Section {RovaiForgeLinks(model: model, threadId: threadId, accountId: account, sourceKind: kind == "pulls" ? "pr" : "issue", number: selected.forgeNumber, repositoryKey: repository.text("repositoryKey"))}}
                Section("评论") {ForEach(Array(comments.enumerated()), id: \.offset) {_, row in GlassRow {VStack(alignment: .leading, spacing: 8) {Text((row["user"]?.text("login") ?? "").isEmpty ? row["author"]?.text("username") ?? "" : row["user"]!.text("login")).font(.caption).foregroundStyle(Theme.textSecondary);MarkdownContent(raw: row.forgeBody)}}};if let commentNext {Button("更多评论") {Task {await work {try await loadComments(selected.forgeNumber, page: commentNext)}}}.disabled(busy)}}
            } else {
                Section {Picker("记录类型", selection: $kind) {Text("议题").tag("issues");Text("PR / MR").tag("pulls")}.pickerStyle(.segmented);Picker("状态", selection: $status) {Text("开放").tag("open");Text("关闭").tag("closed");Text("全部").tag("all")};TextField("搜索标题和正文", text: $search);Picker("排序", selection: $sort) {Text("最新创建").tag("newest");Text("最早创建").tag("oldest");Text("最近更新").tag("recently_updated");Text("最早更新").tag("least_recently_updated")}.disabled(["gitea", "forgejo"].contains(repository?.text("provider") ?? ""));Toggle("指派给我", isOn: $assigned);TextField("标签，逗号分隔", text: $labels);Button("应用筛选") {Task {await work {try await load()}}}}.disabled(busy)
                if total != nil || incomplete {Text("\(total.map {"匹配 \($0) 项"} ?? "")\(reachable.map {" · 可翻阅前 \($0) 项"} ?? "")\(incomplete ? " · 平台返回部分结果" : "")").font(.caption).foregroundStyle(Theme.textSecondary)}
                ForEach(rows, id: \.self) {row in Button {Task {await work {let result = try await call(kind == "pulls" ? "pr" : "issue", extra: ["number": .number(Double(row.forgeNumber))]);selected = result["data"];try await loadComments(row.forgeNumber);if kind == "pulls" {detail = try await call("change_detail", extra: ["number": .number(Double(row.forgeNumber))])["data"]}}}} label: {GlassRow {VStack(alignment: .leading, spacing: 8) {Text(row.text("title")).font(.headline);Text("#\(row.forgeNumber) · \(row.text("state"))").font(.caption).foregroundStyle(Theme.textSecondary)}}}.buttonStyle(.plain).disabled(busy).listRowBackground(Color.clear).listRowSeparator(.hidden)}
                HStack {Button("上一页") {Task {await work {try await load(page: page - 1)}}}.disabled(busy || page <= 1);Spacer();Text("\(page)");Spacer();Button("下一页") {if let next {Task {await work {try await load(page: next)}}}}.disabled(busy || next == nil)}
            }
        }.scrollContentBackground(.hidden).task(id: identity) {await work {accounts = try await model.query(.forgeAccountsGet)["accounts"]?.rvArray ?? [];try await load()}}
    }
    private func work(_ action: () async throws -> Void) async {guard !busy else {return};busy = true;defer {busy = false};issue = nil;do {try await action()} catch is CancellationError {} catch {issue = error.localizedDescription}}
    private func call(_ action: String, extra: [String: JSONValue] = [:]) async throws -> JSONValue {
        let generation = model.connectionGeneration
        var params = extra;params["scope"] = .object(["threadId": .string(threadId)]);params["action"] = .string(action);if !account.isEmpty {params["accountId"] = .string(account)}
        let result = try await model.query(.forgeOperation, params: .object(params));guard generation == model.connectionGeneration else {throw CancellationError()};guard result["success"]?.rvBool == true else {throw RovaiHostFailure(code: "仓库服务返回状态 \(result["status"]?.rvInt ?? 0)，请核对远端结果和权限")};repository = result["repository"];return result
    }
    private func load(page: Int = 1) async throws {
        let selectedFilters: JSONValue = page == 1 ? .object(["state": .string(status), "search": .string(search), "sort": .string(sort), "assignedMe": .bool(assigned), "labels": .array(Array(Set(labels.components(separatedBy: CharacterSet(charactersIn: ",，")).map {$0.trimmingCharacters(in: .whitespacesAndNewlines)}.filter {!$0.isEmpty})).sorted().map(JSONValue.string))]) : filters ?? .object([:])
        if page == 1 {filters = selectedFilters};var extra = selectedFilters.rvObject;extra["page"] = .number(Double(page));let result = try await call(kind, extra: extra);rows = result["data"]?.rvArray ?? [];self.page = page;next = result["nextPage"]?.rvInt;total = result["totalCount"]?.rvInt;reachable = result["reachableCount"]?.rvInt;incomplete = result["incomplete"]?.rvBool == true;selected = nil;detail = nil;comments = [];files = [];fileNext = 1
    }
    private func loadComments(_ number: Int, page: Int = 1) async throws {let result = try await call(kind == "pulls" ? "comments_pr" : "comments_issue", extra: ["number": .number(Double(number)), "page": .number(Double(page))]);comments = page == 1 ? result["data"]?.rvArray ?? [] : comments + (result["data"]?.rvArray ?? []);commentNext = result["nextPage"]?.rvInt}
}
