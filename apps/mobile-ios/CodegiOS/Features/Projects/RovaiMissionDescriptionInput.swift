import SwiftUI
import UIKit

private let missionMemberAttribute = NSAttributedString.Key("app.rovai.missionMember")
private struct MissionMentionInsertion {let id: String;let name: String;let nonce = UUID()}

func missionDescriptionMemberIds(_ content: [JSONValue]) -> Set<String> {Set(content.filter {$0.text("kind") == "member_mention"}.map {$0.text("agentId")})}
func missionDescriptionDisplay(_ content: [JSONValue], members: [JSONValue]) -> String {content.map {part in part.text("kind") == "text" ? part.text("text") : "@" + (members.first {$0.text("agentId") == part.text("agentId")}?.text("displayName") ?? "不可用队员")}.joined()}

struct RovaiMissionDescriptionInput: View {
    let content: [JSONValue]
    let members: [JSONValue]
    let disabled: Bool
    let onChange: ([JSONValue]) -> Void
    @State private var picker = false
    @State private var insertion: MissionMentionInsertion?
    @State private var issue: String?
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            MissionDescriptionTextView(content: content, members: members, disabled: disabled, insertion: insertion, onChange: onChange, onError: {issue = $0})
                .frame(minHeight: 120, maxHeight: 260).background(Theme.bgElevated).clipShape(RoundedRectangle(cornerRadius: 6)).overlay(RoundedRectangle(cornerRadius: 6).stroke(Theme.surfaceStroke, lineWidth: 1))
            Button("提及队员", systemImage: "at") {picker = true}.font(.caption).disabled(disabled || issue != nil)
            if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning)}
        }.sheet(isPresented: $picker) {NavigationStack {List {ForEach(members.filter {$0.text("presence") == "present"}.map {RovaiRow(value: $0)}) {member in Button(member.value.text("displayName")) {insertion = MissionMentionInsertion(id: member.id, name: member.value.text("displayName"));picker = false}}}.navigationTitle("提及队员").toolbar {ToolbarItem(placement: .cancellationAction) {Button("关闭") {picker = false}}}}}
    }
}

/// Only an explicit picker or existing Core segment creates member metadata.
/// Clipboard transfers remain plain text, and typing never parses @names as identities.
private final class MissionPlainTextView: UITextView {
    override func copy(_ sender: Any?) {let range = selectedRange;guard range.location <= (text as NSString).length, range.length <= (text as NSString).length - range.location else {return};UIPasteboard.general.string = (text as NSString).substring(with: range)}
    override func cut(_ sender: Any?) {copy(sender);insertText("")}
    override func paste(_ sender: Any?) {if let value = UIPasteboard.general.string {insertText(value)}}
}

private struct MissionDescriptionTextView: UIViewRepresentable {
    let content: [JSONValue]
    let members: [JSONValue]
    let disabled: Bool
    let insertion: MissionMentionInsertion?
    let onChange: ([JSONValue]) -> Void
    let onError: (String?) -> Void
    private static var normal: [NSAttributedString.Key: Any] {[.font: UIFont.preferredFont(forTextStyle: .body), .foregroundColor: UIColor(Theme.textPrimary)]}
    func makeCoordinator() -> Coordinator {Coordinator(self)}
    func makeUIView(context: Context) -> MissionPlainTextView {let view = MissionPlainTextView();view.delegate = context.coordinator;view.backgroundColor = .clear;view.adjustsFontForContentSizeCategory = true;view.allowsEditingTextAttributes = false;view.isScrollEnabled = true;view.showsVerticalScrollIndicator = false;view.textContainerInset = UIEdgeInsets(top: 10, left: 8, bottom: 10, right: 8);view.accessibilityLabel = "使命描述";return view}
    func updateUIView(_ view: MissionPlainTextView, context: Context) {
        let coordinator = context.coordinator;coordinator.parent = self;view.isEditable = !disabled;view.isSelectable = true
        let names = members.map {$0.text("agentId") + ":" + $0.text("displayName")}.joined(separator: "\n")
        if (coordinator.published != content || coordinator.names != names) && view.markedTextRange == nil {
            do {
                let rendered = NSMutableAttributedString(string: "")
                for part in content {switch part.text("kind") {
                case "text": rendered.append(NSAttributedString(string: part.text("text"), attributes: Self.normal))
                case "member_mention": let id = part.text("agentId");guard !id.isEmpty else {throw RovaiHostFailure(code: "使命描述中的队员引用无效")};var attributes = Self.normal;attributes[missionMemberAttribute] = id;attributes[.foregroundColor] = UIColor(Theme.accent);rendered.append(NSAttributedString(string: "@" + (members.first {$0.text("agentId") == id}?.text("displayName") ?? "不可用队员"), attributes: attributes))
                default: throw RovaiHostFailure(code: "使命描述包含当前编辑器不支持的内容，原草稿保留")
                }}
                let previous = view.selectedRange;coordinator.assigning = true;view.attributedText = rendered;view.typingAttributes = Self.normal;view.selectedRange = NSRange(location: min(previous.location, rendered.length), length: 0);coordinator.assigning = false;coordinator.published = content;coordinator.names = names
                DispatchQueue.main.async {onError(nil)}
            } catch {view.isEditable = false;DispatchQueue.main.async {onError(error.localizedDescription)}}
        }
        if let insertion, insertion.nonce != coordinator.insertion, !disabled {
            coordinator.insertion = insertion.nonce
            DispatchQueue.main.async {guard !coordinator.parent.disabled else {return};coordinator.insert(insertion, into: view)}
        }
    }
    final class Coordinator: NSObject, UITextViewDelegate {
        var parent: MissionDescriptionTextView
        var published: [JSONValue]?
        var names = ""
        var insertion: UUID?
        var assigning = false
        init(_ parent: MissionDescriptionTextView) {self.parent = parent}
        private func tokenRanges(_ view: UITextView) -> [NSRange] {var result: [NSRange] = [];view.attributedText.enumerateAttribute(missionMemberAttribute, in: NSRange(location: 0, length: view.attributedText.length)) {id, range, _ in if id is String {result.append(range)}};return result}
        private func snap(_ offset: Int, in view: UITextView) -> Int {let position = min(max(0, offset), view.attributedText.length);guard let token = tokenRanges(view).first(where: {position > $0.location && position < NSMaxRange($0)}) else {return position};return position - token.location < token.length / 2 ? token.location : NSMaxRange(token)}
        func textViewDidChangeSelection(_ view: UITextView) {guard !assigning, view.markedTextRange == nil else {return};view.typingAttributes = MissionDescriptionTextView.normal;let start = snap(view.selectedRange.location, in: view), end = snap(NSMaxRange(view.selectedRange), in: view);let range = NSRange(location: min(start, end), length: abs(end - start));if range != view.selectedRange {assigning = true;view.selectedRange = range;assigning = false}}
        func textView(_ view: UITextView, shouldChangeTextIn range: NSRange, replacementText text: String) -> Bool {
            guard !parent.disabled else {return false};view.typingAttributes = MissionDescriptionTextView.normal
            let touched = tokenRanges(view).filter {range.length == 0 ? range.location > $0.location && range.location < NSMaxRange($0) : NSIntersectionRange(range, $0).length > 0}
            guard !touched.isEmpty else {return true}
            let start = min(range.location, touched.map(\.location).min() ?? range.location), end = max(NSMaxRange(range), touched.map(NSMaxRange).max() ?? NSMaxRange(range))
            let next = NSMutableAttributedString(attributedString: view.attributedText);next.replaceCharacters(in: NSRange(location: start, length: end - start), with: NSAttributedString(string: text, attributes: MissionDescriptionTextView.normal))
            assigning = true;view.attributedText = next;view.typingAttributes = MissionDescriptionTextView.normal;view.selectedRange = NSRange(location: start + (text as NSString).length, length: 0);assigning = false;publish(view);return false
        }
        func textViewDidChange(_ view: UITextView) {guard !assigning else {return};publish(view)}
        private func publish(_ view: UITextView) {
            var parts: [JSONValue] = []
            view.attributedText.enumerateAttribute(missionMemberAttribute, in: NSRange(location: 0, length: view.attributedText.length)) {id, range, _ in
                if let id = id as? String, !id.isEmpty {parts.append(.object(["kind": .string("member_mention"), "agentId": .string(id)]))}
                else {parts.append(.object(["kind": .string("text"), "text": .string((view.text as NSString).substring(with: range))]))}
            }
            if parts.isEmpty {parts = [.object(["kind": .string("text"), "text": .string("")])]};published = parts;parent.onChange(parts)
        }
        func insert(_ member: MissionMentionInsertion, into view: UITextView) {
            guard !parent.disabled else {return};view.unmarkText();let start = snap(view.selectedRange.location, in: view), end = snap(NSMaxRange(view.selectedRange), in: view)
            var attributes = MissionDescriptionTextView.normal;attributes[missionMemberAttribute] = member.id;attributes[.foregroundColor] = UIColor(Theme.accent)
            let label = "@" + member.name, next = NSMutableAttributedString(attributedString: view.attributedText)
            next.replaceCharacters(in: NSRange(location: min(start, end), length: abs(end - start)), with: NSAttributedString(string: label, attributes: attributes))
            assigning = true;view.attributedText = next;view.typingAttributes = MissionDescriptionTextView.normal;view.selectedRange = NSRange(location: min(start, end) + (label as NSString).length, length: 0);assigning = false;publish(view);view.becomeFirstResponder()
        }
    }
}
