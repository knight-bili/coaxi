import SwiftUI

/// canvas-conversation.ts resolves only the saved identity; no singleChat.open or latest-session fallback.
struct RovaiCanvasConversationNode: View {
    let model: RovaiWorkspaceModel
    let boardId: String
    let node: JSONValue
    let onOpenThread: (String) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var ready = false, retry = 0
    @State private var issue: String?
    private var data: JSONValue {node["data"] ?? .null}
    private var reference: JSONValue {data["conversation"] ?? .null}
    private var threadId: String {reference.text("threadId")}
    private var conversationId: String? {reference["conversationId"]?.stringValue}
    var body: some View {
        Group {
            if ready, let conversationId {RovaiPrivateConversationView(model: model, threadId: threadId, initialConversationId: conversationId)}
            else {NavigationStack {VStack(spacing: 16) {if let issue {ContentUnavailableView("原会话暂不可用", systemImage: "bubble.left.and.exclamationmark.bubble.right", description: Text(issue));Button("重新读取") {retry += 1}} else {ProgressView("正在核对卡片绑定和原会话…")}}.toolbar {Button("关闭") {dismiss()}}}}
        }.task(id: retry) {await resolve()}
    }
    private func resolve() async {
        ready = false;issue = nil;let generation = model.connectionGeneration
        do {
            guard data.text("kind") == "conversation", data["objectReference"] == nil || data["objectReference"] == .null, !threadId.isEmpty else {throw RovaiHostFailure(code: "卡片不属于普通会话引用")}
            let graph = try await model.query(.canvasNodesSnapshot, params: .object(["boardId": .string(boardId)]))
            let saved = (graph["nodes"]?.rvArray ?? []).first {$0.text("id") == node.text("id") }?["data"]
            guard saved?.text("kind") == "conversation", saved?["conversation"] == reference, saved?["objectReference"] == nil || saved?["objectReference"] == .null else {throw RovaiHostFailure(code: "原卡片已删除或绑定已变化；引用保留")}
            if let conversationId {
                let value = try await model.query(.singleChatGet, params: .object(["conversationId": .string(conversationId), "composerScope": .null])), conversation = value["conversation"]
                guard conversation?.text("id") == conversationId, conversation?.text("threadId") == threadId, conversation?.text("status") == "active" else {throw RovaiHostFailure(code: "原私聊已结束、删除或归属已变化；卡片引用保留")}
                let anchor = try await model.query(.threadsOpen, params: .object(["threadId": .string(threadId), "traceId": .string(UUID().uuidString.lowercased())]))
                guard anchor["thread"]?.text("id") == threadId else {throw RovaiHostFailure(code: "原私聊的公开会话锚点已不可用")}
                guard generation == model.connectionGeneration else {throw CancellationError()};ready = true
            } else {
                let value = try await model.query(.threadsOpen, params: .object(["threadId": .string(threadId), "traceId": .string(UUID().uuidString.lowercased())]))
                guard value["thread"]?.text("id") == threadId, generation == model.connectionGeneration else {throw RovaiHostFailure(code: "原公开会话已不可用")};dismiss();onOpenThread(threadId)
            }
        } catch is CancellationError {} catch {issue = error.localizedDescription}
    }
}
