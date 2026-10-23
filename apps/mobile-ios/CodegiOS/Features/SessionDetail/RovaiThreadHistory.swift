import SwiftUI

struct RovaiThreadHistory: View {
    let model: RovaiWorkspaceModel
    let threadId: String
    var anchor: String? = nil
    @Environment(\.dismiss) private var dismiss
    @State private var rows: [JSONValue] = []
    @State private var before: Int?
    @State private var watermark = 0
    @State private var query = ""
    @State private var frozenQuery = ""
    @State private var matchIndex = 0
    @State private var total = 0
    @State private var focused: String?
    @State private var busy = false
    @State private var issue: String?
    var body: some View {
        NavigationStack {VStack(spacing: 8) {
            HStack {TextField("搜索消息", text: $query).textFieldStyle(.roundedBorder).onSubmit {search(query, index: 0)};Button("搜索") {search(query, index: 0)}.disabled(busy || query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)}.padding(.horizontal, 12)
            if !frozenQuery.isEmpty {HStack {Text("\(total == 0 ? 0 : matchIndex + 1) / \(total)").font(.caption);Spacer();Button("上一处") {search(frozenQuery, index: matchIndex - 1)}.disabled(busy || matchIndex <= 0);Button("下一处") {search(frozenQuery, index: matchIndex + 1)}.disabled(busy || matchIndex + 1 >= total)}.padding(.horizontal, 12)}
            if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning).padding(.horizontal, 12)}
            ScrollViewReader {proxy in List {
                if let before, before > 0 {Button("更早的消息") {Task {await work {try await page(before)}}}.disabled(busy)}
                ForEach(rows, id: \.self) {message in GlassRow(isSelected: message.text("id") == focused) {VStack(alignment: .leading, spacing: 8) {Text(message.text("authorDisplayName").isEmpty ? message.text("authorType") : message.text("authorDisplayName")).font(.caption.weight(.semibold));Text(message.text("createdAt")).font(.caption2).foregroundStyle(Theme.textSecondary);RovaiMessageBody(model: model, threadId: threadId, message: message)}}.id(message.text("id")).listRowBackground(Color.clear).listRowSeparator(.hidden)}
                if !busy && rows.isEmpty {Text("没有消息记录").foregroundStyle(Theme.textSecondary)}
            }.scrollContentBackground(.hidden).onChange(of: focused) {_, id in if let id {proxy.scrollTo(id, anchor: .center)}}.onChange(of: rows) {_, _ in if let focused {proxy.scrollTo(focused, anchor: .center)}}}
        }.navigationTitle("消息记录").navigationBarTitleDisplayMode(.inline).toolbar {
            ToolbarItem(placement: .cancellationAction) {Button("关闭") {dismiss()}}
            ToolbarItem(placement: .topBarTrailing) {Button("最新") {Task {await work {rows = [];before = nil;focused = nil;frozenQuery = "";watermark = 0;try await page(9007199254740991)}}}.disabled(busy)}
        }}.task(id: "\(threadId):\(anchor ?? "latest")") {await work {if let anchor {try await around(anchor)} else {try await page(9007199254740991)}}}
    }
    private func page(_ cursor: Int) async throws {
        let generation = model.connectionGeneration
        let value = try await model.query(.threadMessagesPage, params: .object(["threadId": .string(threadId), "beforeSequence": .number(Double(cursor)), "throughGlobalSequence": .number(Double(watermark)), "limit": .number(50)]))
        guard generation == model.connectionGeneration, !Task.isCancelled else {throw CancellationError()}
        var seen = Set<String>();rows = Array(((value["messages"]?.rvArray ?? []) + rows).filter {seen.insert($0.text("id")).inserted}.sorted {($0["sequence"]?.rvInt ?? 0) < ($1["sequence"]?.rvInt ?? 0)}.prefix(200))
        before = value["nextBeforeSequence"]?.rvInt;watermark = value["throughGlobalSequence"]?.rvInt ?? 0
    }
    private func around(_ id: String) async throws {
        let generation = model.connectionGeneration
        let value = try await model.query(.threadMessagesAround, params: .object(["threadId": .string(threadId), "messageId": .string(id)]))
        guard generation == model.connectionGeneration, !Task.isCancelled else {throw CancellationError()}
        rows = value["messages"]?.rvArray ?? [];before = rows.compactMap {$0["sequence"]?.rvInt}.min();watermark = value["throughGlobalSequence"]?.rvInt ?? 0;focused = id
        if value["sourceAvailable"]?.rvBool != true {issue = "原消息已不可用，已有引用快照仍保留。"}
    }
    private func search(_ text: String, index: Int) {Task {await work {
        let generation = model.connectionGeneration
        let value = try await model.query(.threadMessagesFind, params: .object(["threadId": .string(threadId), "query": .string(text), "selectedMatchIndex": .number(Double(index))]))
        guard generation == model.connectionGeneration, !Task.isCancelled else {throw CancellationError()}
        frozenQuery = text;total = value["totalMatchCount"]?.rvInt ?? 0;matchIndex = value["selectedMatchIndex"]?.rvInt ?? 0
        if let match = value["match"], !match.text("messageId").isEmpty {try await around(match.text("messageId"))} else {rows = [];before = nil;focused = nil}
    }}}
    private func work(_ action: () async throws -> Void) async {guard !busy else {return};busy = true;issue = nil;defer {busy = false};do {try await action()} catch is CancellationError {} catch {issue = error.localizedDescription}}
}

struct RovaiPublicMessageOrigin: View {
    let model: RovaiWorkspaceModel
    let threadId: String
    let messageId: String
    let label: String
    @State private var open = false
    var body: some View {Button(label) {open = true}.font(.caption).sheet(isPresented: $open) {RovaiThreadHistory(model: model, threadId: threadId, anchor: messageId)}}
}
