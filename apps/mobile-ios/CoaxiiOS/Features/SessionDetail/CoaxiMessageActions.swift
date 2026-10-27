import SwiftUI
import UIKit

struct CoaxiMessageActions: View {
    let model: CoaxiWorkspaceModel
    let threadId: String
    let message: JSONValue
    let enabled: Bool
    let onQuote: (JSONValue) -> Bool
    let onReply: (JSONValue) -> Void
    @State private var withdrawal: JSONValue?
    @State private var quote = false
    var body: some View {
        Menu {
            Button("回复", systemImage: "arrowshape.turn.up.left") {
                onReply(.object(["id": message["id"] ?? .null, "authorId": message["authorId"] ?? .null, "authorDisplayName": message["authorDisplayName"] ?? .string("原消息"), "excerpt": .string(String(message.text("body").prefix(320)))]))
            }.disabled(!enabled)
            Button("引用一段", systemImage: "text.quote") {quote = true}.disabled(!enabled)
            Button("复制消息", systemImage: "doc.on.doc") {UIPasteboard.general.string = message.text("body")}
            if message["canWithdraw"]?.rvBool == true {Button("撤回", role: .destructive) {withdrawal = .object(["threadId": .string(threadId), "messageId": message["id"] ?? .null, "expectedVersion": message["version"] ?? .null])}.disabled(!enabled)}
        } label: {Image(systemName: "ellipsis").padding(6)}.accessibilityLabel("消息操作")
        .sheet(isPresented: $quote) {CoaxiQuoteSelector(model: model, threadId: threadId, messageId: message.text("id"), onChosen: onQuote)}
        .confirmationDialog("撤回这条消息？", isPresented: Binding(get: {withdrawal != nil}, set: {if !$0 {withdrawal = nil}}), titleVisibility: .visible) {
            Button("撤回", role: .destructive) {if let frozen = withdrawal {Task {await model.command(.threadMessagesWithdraw, value: frozen)}};withdrawal = nil}.disabled(model.pending != nil || model.busy)
            Button("取消", role: .cancel) {withdrawal = nil}
        } message: {Text("消息版本一致且仍允许撤回时才执行；已开始的工作请另外停止。")}
    }
}
struct CoaxiReplyPreview: View {
    let reply: JSONValue
    let enabled: Bool
    let onRemove: () -> Void
    var body: some View {
        GlassRow {HStack {VStack(alignment: .leading, spacing: 4) {Text("回复 · \(reply.text("authorDisplayName").isEmpty ? "原消息" : reply.text("authorDisplayName"))").font(.caption.weight(.medium));Text(reply.text("excerpt")).font(.caption).foregroundStyle(Theme.textSecondary).lineLimit(2)};Spacer();Button("取消回复", action: onRemove).font(.caption).disabled(!enabled)}}
    }
}
