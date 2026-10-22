import SwiftUI
import CryptoKit

struct NativeContinuationSelection: Identifiable {let mode: String;var turn: String? = nil;var id: String {mode + ":" + (turn ?? "")}}
private func nativeForkPoint(_ provider: String, _ transcript: JSONValue, _ selected: String) throws -> JSONValue {
    let turns = transcript["turns"]?.rvArray ?? []
    guard let index = turns.firstIndex(where: {$0.text("id") == selected && $0.text("role") == "assistant"}) else {throw RovaiHostFailure(code: "请重新选择原助手回答")}
    func text(_ turn: JSONValue) -> String {(turn["blocks"]?.rvArray ?? []).filter {$0.text("type") == "text"}.map {$0.text("text")}.joined()}
    func fingerprint(_ text: String) -> String {"sha256:" + SHA256.hash(data: Data(text.utf8)).map {String(format: "%02x", $0)}.joined()}
    let turn = turns[index], content = text(turn), native = turn["agent_message_id"]?.stringValue.flatMap {$0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : $0}
    let digest = content.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : fingerprint(content)
    guard digest != nil || provider != "codex" && native != nil else {throw RovaiHostFailure(code: "这条回答暂时无法准确定位，请另选回答或明确从末尾分叉")}
    let id = (provider == "codex" ? selected : native ?? selected).replacingOccurrences(of: ":segment:[0-9]+$", with: "", options: .regularExpression)
    let occurrence = digest.map {digest in turns.prefix(index).filter {$0.text("role") == "assistant" && fingerprint(text($0)) == digest}.count + 1}
    return .object(["selectedTurnId": .string(selected), "messageId": .string(id), "messageFingerprint": digest.map(JSONValue.string) ?? .null, "messageOccurrence": occurrence.map {JSONValue.number(Double($0))} ?? .null])
}
func nativeCreationState(_ receipt: JSONValue) -> String {
    switch receipt.text("state") {
    case "prepared": return "已登记，正在准备原生会话"
    case "starting": return "原程序正在恢复或分叉"
    case "binding_recorded": return "原生会话已记录，新会话尚未发送消息"
    case "failed": return "未完成创建，原导入快照保留"
    case "unknown": return "创建结果待核对，不重复创建"
    default: return receipt.text("state")
    }
}

/// 延用原会话库和队员设置的原生 Section / Picker；没有替代主题或工程切换。
struct RovaiNativeContinuation: View {
    let model: RovaiWorkspaceModel;let entry: JSONValue;let transcript: JSONValue;let selection: NativeContinuationSelection
    @Environment(\.dismiss) private var dismiss
    @State private var members: [JSONValue] = [], member = "", title = ""
    @State private var packet: JSONValue?, receipt: JSONValue?, issue: String?, generation: UInt64?
    @State private var ready = false, busy = false
    private var owner: String {"native-continuation:\(entry.text("linkedThreadId")):\(entry.text("id"))"}
    private var preview: JSONValue? {packet?["preview"]}
    private var original: JSONValue? {packet?["original"]}
    private var disabled: Bool {busy || !ready || model.busy || model.pending != nil || generation != model.connectionGeneration}
    var body: some View {
        NavigationStack {List {
            if let issue {Section {Text(issue).font(.caption).foregroundStyle(Theme.warning)}}
            if let preview {
                Section("新工作会话") {
                    LabeledContent("名称", value: preview.text("title"));LabeledContent("来源程序", value: preview["source"]?.text("provider") ?? "")
                    LabeledContent("历史消息", value: String(preview["historyMessageCount"]?.rvInt ?? 0));LabeledContent("队员", value: members.first {$0.text("agentId") == preview.text("agentId")}?.text("displayName") ?? preview.text("agentId"))
                    if let point = preview["point"], point != .null {Text("从指定助手回答 \(point.text("selectedTurnId")) 分叉；无法准确定位时不会自动改为末尾分叉。").font(.caption)}
                    if let reason = preview["reason"]?.stringValue {Text(reason).font(.caption).foregroundStyle(Theme.warning)}
                    Text("不会重置 Git 或改写工程文件。原程序可能继续写它自己的日志；导入快照保持只读。").font(.caption).foregroundStyle(Theme.textSecondary)
                }
                if let receipt {Section("原创建结果") {
                    Text(nativeCreationState(receipt));Text(receipt.text("commandId")).font(.caption2).textSelection(.enabled)
                    if let error = receipt["error"]?.stringValue {Text(error).font(.caption).foregroundStyle(Theme.warning)}
                    if receipt.text("state") == "binding_recorded" {Button("打开新会话") {Task {await model.openThread(receipt.text("threadId"));dismiss()}}.disabled(disabled)}
                }}
                Section {
                    Button(original == nil ? "确认创建，不发送消息" : "核对原创建结果") {Task {await submit(preview)}}.disabled(disabled || original == nil && preview.text("capabilityStatus") != "available")
                    Text("创建结果未知时只核对同一编号，不重复创建，也不将只读记录伪装为普通消息。").font(.caption).foregroundStyle(Theme.textSecondary)
                }
            } else {
                Section("新工作会话") {
                    Picker("使用哪位队员", selection: $member) {Text("选择队员").tag("");ForEach(members, id: \.self) {candidate in Text(candidate.text("displayName")).tag(candidate.text("agentId"))}}.disabled(disabled)
                    TextField("新会话名称", text: $title).disabled(disabled)
                    Text("创建只有所选队员的独立工作会话，原记录保留；不会隐式发送消息。").font(.caption).foregroundStyle(Theme.textSecondary)
                    Button("预览新会话") {Task {await prepare()}}.disabled(disabled || member.isEmpty || title.unicodeScalars.count > 80)
                }
            }
        }.navigationTitle(selection.mode == "continue" ? "从原生会话继续" : selection.mode == "fork_point" ? "从这条回答分叉" : "从会话末尾分叉").navigationBarTitleDisplayMode(.inline).scrollContentBackground(.hidden).toolbar {Button("关闭并保留") {dismiss()}.disabled(busy)}}
        .interactiveDismissDisabled(busy)
        .task(id: owner) {
            generation = model.connectionGeneration;busy = true;defer {busy = false}
            do {
                let sourceTitle = entry["summary"]?.text("title") ?? "";title = sourceTitle.isEmpty ? entry.text("sourceName") : sourceTitle
                let raw = try model.readBody(owner);if !raw.isEmpty {packet = try JSONDecoder().decode(JSONValue.self, from: Data(raw.utf8));if let original {try await reconcile(original)}}
                let list = try await nativeUserCall(model, .membersList, .object([:]));members = (list.rvArray).filter {$0.text("presence") == "present" && $0["runtimeConfiguration"] != nil && $0["runtimeConfiguration"] != .null};if members.count == 1 {member = members[0].text("agentId")};ready = true
            } catch {issue = error.localizedDescription}
        }
    }
    private func keep(_ value: JSONValue) throws {guard generation == model.connectionGeneration else {throw CancellationError()};try model.saveBody(owner, body: String(decoding: try JSONEncoder().encode(value), as: UTF8.self));packet = value}
    private func prepare() async {
        guard !disabled, let selected = members.first(where: {$0.text("agentId") == member}) else {return};busy = true;defer {busy = false};issue = nil
        do {
            let point = selection.mode == "fork_point" ? try nativeForkPoint(entry.text("provider"), transcript, selection.turn ?? "") : .null
            let value = try await nativeUserCall(model, .historyExecutionPreview, .object(["schemaVersion": .number(1), "sourceThreadId": entry["linkedThreadId"] ?? .null, "importId": entry["id"] ?? .null, "expectedImportRevision": entry["revision"] ?? .null, "agentId": .string(member), "expectedAgentVersion": selected["version"] ?? .null, "mode": .string(selection.mode), "point": point, "title": .string(title)]));try keep(.object(["preview": value]))
        } catch {issue = error.localizedDescription}
    }
    private func reconcile(_ input: JSONValue) async throws {let value = try await nativeUserCall(model, .historyExecutionReceipt, .object(["schemaVersion": .number(1), "commandId": input["commandId"] ?? .null]));guard value.text("commandId") == input.text("commandId") else {throw RovaiHostFailure(code: "原回执归属不一致")};receipt = value}
    private func submit(_ preview: JSONValue) async {
        guard !disabled else {return};busy = true;defer {busy = false};issue = nil
        do {
            if let original {try await reconcile(original);return}
            let request: JSONValue = .object(["schemaVersion": .number(1), "commandId": .string(UUID().uuidString.lowercased()), "previewId": preview["previewId"] ?? .null, "previewDigest": preview["previewDigest"] ?? .null]);try keep(.object(["preview": preview, "original": request]))
            let value = try await nativeUserCall(model, .historyExecutionConfirm, request);guard value.text("commandId") == request.text("commandId") else {throw RovaiHostFailure(code: "原回执归属不一致")};receipt = value
        } catch {issue = error.localizedDescription}
    }
}
