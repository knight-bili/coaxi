import SwiftUI

func conversationSourceReference(_ source: JSONValue) -> String {
    let title = (source.text("title").isEmpty ? "旧讨论" : source.text("title")).replacingOccurrences(of: "[\\r\\n\\t]+", with: " ", options: .regularExpression).replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "[", with: "\\[").replacingOccurrences(of: "]", with: "\\]")
    return "[参考：\(title)](coaxi-source:\(source.text("sourceSessionId")))"
}

private struct ConversationSourceRow: Identifiable {
    let value: JSONValue
    let field: String
    var id: String {value.text(field)}
}

/// 沿原生历史库的 List/Section，不增加输入副本；授权只加入原草稿。
struct CoaxiConversationSources: View {
    let model: CoaxiWorkspaceModel
    let threadId: String
    var conversationId: String? = nil
    var child = false
    var enabled = true
    let onInsert: (String) throws -> Void
    @State private var open = false
    var body: some View {
        Button("参考旧讨论") {open = true}.disabled(!enabled)
            .sheet(isPresented: $open) {
                CoaxiConversationSourcePicker(model: model, scope: conversationScope(threadId, conversationId, child), enabled: enabled, onInsert: onInsert)
                    .id("sources:\(threadId):\(conversationId ?? ""):\(model.connectionGeneration)")
            }
    }
}

private struct CoaxiConversationSourcePicker: View {
    let model: CoaxiWorkspaceModel
    let scope: JSONValue
    let enabled: Bool
    let onInsert: (String) throws -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var rows: [JSONValue] = []
    @State private var imports: [JSONValue] = []
    @State private var grants: [JSONValue] = []
    @State private var next: String?
    @State private var grantNext: String?
    @State private var selected: JSONValue?
    @State private var preview: JSONValue?
    @State private var confirmed: JSONValue?
    @State private var pending: JSONValue?
    @State private var busy = false
    @State private var ready = false
    @State private var issue: String?
    @State private var generation: UInt64 = 0
    @State private var importLimit = 25
    private var owner: String {"conversation-sources:\(scope.text("threadId")):\(scope.text("conversationId")):\(scope.text("scopeKind"))"}
    var body: some View {
        NavigationStack {
            List {
                Section {
                    Text(scope.text("scopeKind") == "public" ? "团队讨论只列出可公开参考的来源。私聊和私人导入资料请在对应私聊中选择。" : "资料只授权给当前私聊或帮手讨论，不会开放给整个团队。")
                    Text("只读取已保存的可见文字和工具名称。授权加入消息后，再按原方式发送。").font(.caption).foregroundStyle(Theme.textSecondary)
                }
                Section("选择讨论") {
                    ForEach(rows.map {ConversationSourceRow(value: $0, field: "sourceSessionId")}) {row in
                        Button {work {try await choose(row.value)}} label: {Text(row.value.text("title"))}.disabled(busy || !ready || pending != nil)
                    }
                    if next != nil {Button("更多讨论") {work {try await load(more: true)}}.disabled(busy || pending != nil)}
                }
                if scope.text("scopeKind") != "public" {
                    Section("已导入历史") {
                        Button("读取原历史库") {work {let value = try await call(.historyLibrary, .object(["scope": .object(["threadId": scope["threadId"] ?? .null]), "operation": .string("list"), "includeArchived": .bool(false)]));imports = value["entries"]?.rvArray ?? []}}.disabled(busy || pending != nil)
                        ForEach(imports.prefix(importLimit).map {ConversationSourceRow(value: $0, field: "id")}) {row in
                            Button(row.value["summary"]?.text("title") ?? "历史讨论") {work {let entry = row.value;guard !entry.text("linkedThreadId").isEmpty, !entry.text("revision").isEmpty else {throw CoaxiHostFailure(code: "导入缺少原归属或修订")};let value = try await call(.historyLibrary, .object(["scope": .object(["threadId": entry["linkedThreadId"] ?? .null]), "operation": .string("source.prepare"), "commandId": .string(UUID().uuidString.lowercased()), "importId": entry["id"] ?? .null, "expectedRevision": entry["revision"] ?? .null]));guard let source = value["source"] else {throw CoaxiHostFailure(code: "缺少资料描述")};try await choose(source)}}.disabled(busy || pending != nil)
                        }
                        if imports.count > importLimit {Button("更多历史") {importLimit += 25}}
                    }
                }
                if let preview {Section(selected?.text("title") ?? "旧讨论") {Text("预览有效至 \(preview.text("expiresAt"))");Text("授权最多两小时；撤销后不能继续读取。这里只准备引用，不发送消息或启动队员。").font(.caption)}}
                if !grants.isEmpty {Section("当前讨论已授权的资料") {
                    ForEach(grants.map {ConversationSourceRow(value: $0, field: "grantId")}) {row in
                        VStack(alignment: .leading, spacing: 6) {
                            Text(rows.first {$0.text("sourceSessionId") == row.value.text("sourceSessionId")}?.text("title") ?? "旧讨论")
                            Text("有效至 \(row.value.text("expiresAt"))").font(.caption).foregroundStyle(Theme.textSecondary)
                            Button("撤销授权", role: .destructive) {work {let original = try retain("conversationTools.grants.revoke");_ = try await call(.conversationToolsGrantsRevoke, .object(["commandId": original["commandId"] ?? .null, "grantId": row.value["grantId"] ?? .null, "expectedRevision": row.value["revision"] ?? .null]));try clear(original);if confirmed?.text("grantId") == row.id {confirmed = nil};try await load()}}.disabled(busy || pending != nil)
                        }
                    }
                    if let grantNext {Button("更多授权记录") {work {let value = try await call(.conversationToolsGrantsList, .object(["scope": scope, "after": .string(grantNext), "limit": .number(100)]));grants += (value["items"]?.rvArray ?? []).filter {item in !grants.contains {$0.text("grantId") == item.text("grantId")}};self.grantNext = value["next"]?.stringValue}}.disabled(busy || pending != nil)}
                }}
                if let issue {Section {Text(issue).font(.caption).foregroundStyle(Theme.warning)}}
                Section {
                    if pending != nil {Button("核对原授权请求") {work {try await reconcile()}}.disabled(busy)}
                    else if let confirmed {Button("将授权加入输入") {work {try insert(confirmed)}}.disabled(busy || !enabled)}
                    else {Button("授权并加入当前输入") {work {guard let preview else {return};let original = try retain("conversationTools.grants.confirm");let grant = try await call(.conversationToolsGrantsConfirm, .object(["commandId": original["commandId"] ?? .null, "previewId": preview["previewId"] ?? .null, "previewDigest": preview["previewDigest"] ?? .null]));confirmed = grant;try clear(original);try insert(grant)}}.disabled(busy || !ready || preview == nil || !enabled)}
                }
            }.navigationTitle("选择这次参考的资料").toolbar {ToolbarItem(placement: .cancellationAction) {Button("关闭并保留记录") {dismiss()}}}
        }.task {
            generation = model.connectionGeneration;busy = true
            defer {busy = false}
            do {let raw = try model.readBody(owner);if !raw.isEmpty {guard raw.utf8.count <= 1024 else {throw CoaxiHostFailure(code: "原授权请求记录过大")};let value = try JSONDecoder().decode(JSONValue.self, from: Data(raw.utf8));guard Set(value.rvObject.keys) == ["operation", "commandId"], ["conversationTools.grants.confirm", "conversationTools.grants.revoke"].contains(value.text("operation")), UUID(uuidString: value.text("commandId")) != nil else {throw CoaxiHostFailure(code: "原授权请求记录损坏")};pending = value};try await load();ready = true} catch {issue = error.localizedDescription}
        }
    }
    @MainActor private func current() throws {guard generation == model.connectionGeneration else {throw CancellationError()}}
    @MainActor private func call(_ operation: CoaxiOperation, _ input: JSONValue) async throws -> JSONValue {try current();let value = try await nativeUserCall(model, operation, input);try current();return value}
    @MainActor private func work(_ action: @escaping @MainActor () async throws -> Void) {guard !busy else {return};busy = true;issue = nil;Task {defer {if generation == model.connectionGeneration {busy = false}};do {try await action()} catch is CancellationError {} catch {if generation == model.connectionGeneration {issue = error.localizedDescription}}}}
    @MainActor private func load(more: Bool = false) async throws {
        let value = try await call(.historyLibrary, .object(["scope": .object(["threadId": scope["threadId"] ?? .null]), "operation": .string("sources"), "targetScope": scope, "after": more ? next.map(JSONValue.string) ?? .null : .null, "limit": .number(50)]))
        let items = value["items"]?.rvArray ?? [];rows = more ? rows + items.filter {item in !rows.contains {$0.text("sourceSessionId") == item.text("sourceSessionId")}} : items;next = value["next"]?.stringValue
        let page = try await call(.conversationToolsGrantsList, .object(["scope": scope, "after": .null, "limit": .number(100)]));grants = page["items"]?.rvArray ?? [];grantNext = page["next"]?.stringValue
    }
    @MainActor private func choose(_ source: JSONValue) async throws {
        let value = try await call(.conversationToolsGrantsPreview, .object(["scope": scope, "sourceSessionId": source["sourceSessionId"] ?? .null, "sourceDigest": source["sourceDigest"] ?? .null, "throughSequence": source["throughSequence"] ?? .null]));selected = source;preview = value;confirmed = nil
    }
    @MainActor private func retain(_ operation: String) throws -> JSONValue {try current();let value = JSONValue.object(["operation": .string(operation), "commandId": .string(UUID().uuidString.lowercased())]);let raw = String(decoding: try JSONEncoder().encode(value), as: UTF8.self);try model.saveBody(owner, body: raw);pending = value;return value}
    @MainActor private func clear(_ original: JSONValue) throws {let raw = String(decoding: try JSONEncoder().encode(original), as: UTF8.self);guard try model.clearBodyIfMatches(owner, expectedBody: raw, expectedGeneration: generation) else {throw CoaxiHostFailure(code: "原授权请求记录已变化")};pending = nil}
    @MainActor private func insert(_ grant: JSONValue) throws {
        try current();let actual = grant["scope"]
        let formatter = ISO8601DateFormatter();formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        let expiry = formatter.date(from: grant.text("expiresAt")) ?? ISO8601DateFormatter().date(from: grant.text("expiresAt"))
        guard enabled, actual?.text("threadId") == scope.text("threadId"), actual?.text("conversationId") == scope.text("conversationId"), actual?.text("scopeKind") == scope.text("scopeKind"), let expiry, expiry > Date() else {throw CoaxiHostFailure(code: "这份授权已过期或当前输入已变化，请重新选择")}
        let source = selected.flatMap {$0.text("sourceSessionId") == grant.text("sourceSessionId") ? $0 : nil} ?? rows.first {$0.text("sourceSessionId") == grant.text("sourceSessionId")} ?? .object(["title": .string("旧讨论"), "sourceSessionId": grant["sourceSessionId"] ?? .null])
        try onInsert(conversationSourceReference(source));dismiss()
    }
    @MainActor private func reconcile() async throws {
        guard let pending else {return};let value = try await call(.commandsReconcile, .object(["operation": pending["operation"] ?? .null, "params": .object(["commandId": pending["commandId"] ?? .null])]))
        guard value.text("state") == "recorded", let result = value["result"], result.text("commandId") == pending.text("commandId"), result.text("commandType") == pending.text("operation") else {throw CoaxiHostFailure(code: "原授权请求没有确定回执，没有重复授权")}
        try clear(pending);guard result.text("status") != "rejected" else {throw CoaxiHostFailure(code: result.text("code"))};if pending.text("operation") == "conversationTools.grants.confirm" {confirmed = result["payload"]};try await load()
    }
}
