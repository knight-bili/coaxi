import SwiftUI
import UIKit

struct RovaiQuoteSelector: View {
    let model: RovaiWorkspaceModel
    let threadId: String
    let messageId: String
    let onChosen: (JSONValue) -> Bool
    var conversationId: String? = nil
    var onSelection: ((JSONValue) -> Bool)? = nil
    @Environment(\.dismiss) private var dismiss
    @State private var projection: JSONValue?
    @State private var selection = NSRange(location: 0, length: 0)
    @State private var busy = true
    @State private var issue: String?
    private var text: String {projection?.text("text") ?? ""}
    private var safeRange: NSRange {let size = (text as NSString).length;let start = min(max(0, selection.location), size);return NSRange(location: start, length: min(max(0, selection.length), size - start))}
    private var chosen: String {(text as NSString).substring(with: safeRange)}
    private var limit: Int {projection?["maxScalars"]?.rvInt ?? 12000}
    var body: some View {
        NavigationStack {VStack(alignment: .leading, spacing: 12) {
            Text("长按选择一段内容。引用保持当时的快照，原消息变化时拒绝捕获。").font(.caption).foregroundStyle(Theme.textSecondary)
            if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning)}
            RovaiQuoteTextSelection(text: text, selection: $selection).disabled(busy)
            HStack {Text("已选 \(chosen.unicodeScalars.count) / \(limit) 字符").font(.caption).foregroundStyle(Theme.textSecondary);Spacer();Button("加入引用") {Task {await capture()}}.buttonStyle(.borderedProminent).disabled(busy || projection == nil || chosen.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || chosen.unicodeScalars.count > limit)}
        }.padding(16).navigationTitle("选择引用内容").navigationBarTitleDisplayMode(.inline).toolbar {ToolbarItem(placement: .cancellationAction) {Button("关闭") {dismiss()}}}}
        .task(id: messageId) {let generation = model.connectionGeneration;busy = true;defer {busy = false};do {
            var params: [String: JSONValue] = ["threadId": .string(threadId), "messageId": .string(messageId)]
            if let conversationId {params["conversationId"] = .string(conversationId)}
            let value = try await model.query(conversationId == nil ? .messageQuotesProjection : .messageQuotesPrivateProjection, params: .object(params))
            guard generation == model.connectionGeneration, !Task.isCancelled else {return}
            projection = value
            if value.text("text").unicodeScalars.count <= (value["maxScalars"]?.rvInt ?? 12000) {selection = NSRange(location: 0, length: (value.text("text") as NSString).length)}
        } catch is CancellationError {} catch {issue = error.localizedDescription}}
    }
    private func capture() async {
        guard !busy, let projection else {return};busy = true;defer {busy = false};issue = nil
        let generation = model.connectionGeneration, range = safeRange
        let prefix = (text as NSString).substring(to: range.location)
        let selected = chosen
        do {
            let input = JSONValue.object(["messageId": .string(messageId), "bodyAtSelection": projection["bodyAtSelection"] ?? .null, "startScalar": .number(Double(prefix.unicodeScalars.count)), "endScalar": .number(Double(prefix.unicodeScalars.count + selected.unicodeScalars.count)), "text": .string(selected)])
            if conversationId != nil {
                guard generation == model.connectionGeneration, !Task.isCancelled else {return}
                guard onSelection?(input) == true else {issue = "输入框当前无法接收引用，请稍后重试。";return}
            } else {
                let quote = try await model.query(.messageQuotesCapture, params: .object(["threadId": .string(threadId), "selection": input]))
                guard generation == model.connectionGeneration, !Task.isCancelled else {return}
                guard onChosen(quote) else {issue = "输入框当前无法接收引用，请稍后重试。";return}
            }
            dismiss()
        } catch is CancellationError {} catch {issue = error.localizedDescription}
    }
}

private struct RovaiQuoteTextSelection: UIViewRepresentable {
    let text: String
    @Binding var selection: NSRange
    func makeCoordinator() -> Coordinator {Coordinator(self)}
    func makeUIView(context: Context) -> UITextView {let view = UITextView();view.isEditable = false;view.isSelectable = true;view.backgroundColor = .clear;view.font = .preferredFont(forTextStyle: .body);view.textColor = UIColor(Theme.textPrimary);view.delegate = context.coordinator;view.adjustsFontForContentSizeCategory = true;return view}
    func updateUIView(_ view: UITextView, context: Context) {context.coordinator.parent = self;if view.text != text {view.text = text};if view.selectedRange != selection {view.selectedRange = selection}}
    final class Coordinator: NSObject, UITextViewDelegate {
        var parent: RovaiQuoteTextSelection
        init(_ parent: RovaiQuoteTextSelection) {self.parent = parent}
        func textViewDidChangeSelection(_ textView: UITextView) {let range = textView.selectedRange;if parent.selection != range {DispatchQueue.main.async {self.parent.selection = range}}}
    }
}

struct RovaiComposerQuotes: View {
    let quotes: [JSONValue]
    let enabled: Bool
    let onRemove: (String) -> Void
    var body: some View {ForEach(quotes, id: \.self) {quote in GlassRow {HStack {VStack(alignment: .leading, spacing: 4) {Text("引用 · \(quote["authorAtCapture"]?.text("displayName") ?? "")").font(.caption.weight(.medium));Text(quote.text("text")).font(.caption).lineLimit(2).foregroundStyle(Theme.textSecondary)};Spacer();Button("移除") {onRemove(quote.text("quoteId"))}.font(.caption).disabled(!enabled)}}}}
}
