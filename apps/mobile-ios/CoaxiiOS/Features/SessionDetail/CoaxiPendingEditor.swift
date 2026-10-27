import SwiftUI

/// Native counterpart of Coaxi PrivatePendingEditor, retaining the original edit token.
struct CoaxiPendingEditor: View {
    let model: CoaxiWorkspaceModel
    let threadId: String
    let conversationId: String
    let child: Bool
    let item: JSONValue
    let session: JSONValue
    let messages: [JSONValue]
    @State private var text = ""
    @State private var ready = false
    @State private var issue: String?
    @State private var confirm: String?
    @State private var quote: CoaxiRow?
    @State private var removedQuote: String?
    private var token: String {session.text("editToken")}
    private var owner: String {"pending-edit:\(threadId):\(conversationId):\(item.text("id")):\(token)"}
    private var operation: CoaxiOperation {child ? .delegationsPendingInputsEdit : .singleChatPendingInputsEdit}
    private var recovery: Bool {session["recoveryRequired"]?.rvBool == true}
    private var disabled: Bool {!ready || recovery || model.busy || model.pending != nil}
    var body: some View {
        GlassRow {VStack(alignment: .leading, spacing: 10) {
            Text("编辑排队消息").font(.headline)
            if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning)}
            if recovery {Text("编辑已中断，可从已保存版本接管或放弃本次编辑。").font(.caption).foregroundStyle(Theme.textSecondary)}
            TextField("消息内容", text: Binding(get: {text}, set: {text = $0;persist()}), axis: .vertical).lineLimit(3...8).textFieldStyle(.roundedBorder).disabled(disabled)
            let attachments = session["workingAttachments"]?.rvArray ?? []
            ForEach(Array(attachments.enumerated()), id: \.element) {index, attachment in HStack {Text(attachment.text("displayName")).font(.caption);Spacer();Button("上移") {var ids = attachments.compactMap {$0["id"]};let value = ids.remove(at: index);ids.insert(value, at: index - 1);perform(.object(["type": .string("reorder_attachments"), "attachmentRefIds": .array(ids)]))}.disabled(disabled || index == 0);Button("移除") {perform(.object(["type": .string("remove_attachment"), "attachmentRefId": attachment["id"] ?? .null]))}.disabled(disabled)}}
            CoaxiAttachmentPicker(model: model, threadId: threadId, target: .object(["kind": .string(child ? "delegation_child_pending" : "single_chat_pending"), "conversationId": .string(conversationId), "pendingInputId": item["id"] ?? .null, "editToken": .string(token), "composerScope": .null]), revision: item["revision"]?.rvInt ?? 0, disabled: disabled)
            CoaxiComposerQuotes(quotes: session["workingQuotes"]?.rvArray ?? [], enabled: !disabled) {id in removedQuote = id;perform(.object(["type": .string("quote"), "action": .object(["type": .string("remove"), "quoteId": .string(id)])]))}
            HStack {Menu("引用一段消息") {ForEach(messages.map {CoaxiRow(value: $0)}) {message in Button(String(message.value.text("body").prefix(90))) {quote = message}}}.disabled(disabled || messages.isEmpty);if let removedQuote {Button("撤销移除引用") {perform(.object(["type": .string("quote"), "action": .object(["type": .string("restore"), "quoteId": .string(removedQuote)])]));self.removedQuote = nil}.disabled(disabled)}}.font(.caption)
            HStack {
                if recovery {Button("从已保存版本接管") {confirm = "takeover"}.disabled(!ready || model.busy || model.pending != nil)}
                else {Button("保存排队消息") {perform(.object(["type": .string("save"), "body": .string(text)]), settle: true)}.buttonStyle(.borderedProminent).disabled(disabled || text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || text.unicodeScalars.count > 100000)}
                Button("放弃本次编辑", role: .destructive) {confirm = "cancel"}.disabled(!ready || model.busy || model.pending != nil)
            }
        }}
        .task(id: owner) {ready = false;let generation = model.connectionGeneration;do {let raw = try model.readBody(owner);guard generation == model.connectionGeneration else {return};if raw.isEmpty {text = session.text("workingBody")} else {guard let value = JSONValue.parse(raw), value.text("editToken") == token, value.text("pendingInputId") == item.text("id") else {throw CoaxiHostFailure(code: "原编辑草稿归属变化")};text = value.text("body")};ready = true} catch {issue = error.localizedDescription}}
        .confirmationDialog(confirm == "takeover" ? "从已保存版本接管？" : "放弃本次编辑？", isPresented: Binding(get: {confirm != nil}, set: {if !$0 {confirm = nil}}), titleVisibility: .visible) {
            Button("确认", role: .destructive) {if let confirm {perform(.object(["type": .string(confirm)]), settle: confirm == "cancel")};confirm = nil}.disabled(!ready || model.busy || model.pending != nil)
            Button("继续编辑", role: .cancel) {confirm = nil}
        } message: {Text(confirm == "takeover" ? "从原排队消息的已保存正文、附件和引用接管；尚未保存的本机正文草稿保留。" : "保留原排队消息，放弃编辑中的附件与引用修改；正文草稿只在原请求确认成功后清除。")}
        .sheet(item: $quote) {message in CoaxiQuoteSelector(model: model, threadId: threadId, messageId: message.id, onChosen: {_ in false}, conversationId: conversationId, onSelection: {selection in guard !disabled else {return false};perform(.object(["type": .string("quote"), "action": .object(["type": .string("add"), "selection": selection])]));return true})}
    }
    private func document() throws -> String {let value = JSONValue.object(["version": .number(1), "body": .string(text), "editToken": .string(token), "pendingInputId": item["id"] ?? .null]);let encoder = JSONEncoder();encoder.outputFormatting = [.sortedKeys];return String(decoding: try encoder.encode(value), as: UTF8.self)}
    private func persist() {guard ready else {return};do {try model.saveBody(owner, body: document());issue = nil} catch {issue = error.localizedDescription}}
    private func perform(_ action: JSONValue, settle: Bool = false) {
        let value = JSONValue.object(["conversationId": .string(conversationId), "composerScope": .null, "pendingInputId": item["id"] ?? .null, "expectedRevision": session["basePendingRevision"] ?? .null, "editToken": .string(token), "action": action])
        let command: JSONValue
        if !child, case .object(var fields) = value {fields["threadId"] = .string(threadId);command = .object(fields)} else {command = value}
        do {let saved = try document();try model.saveBody(owner, body: saved);let generation = model.connectionGeneration;Task {guard generation == model.connectionGeneration else {return};if settle {await model.executePrivate(operation, command: command, owner: owner, body: saved)} else {await model.command(operation, value: command)}}} catch {issue = error.localizedDescription}
    }
}
