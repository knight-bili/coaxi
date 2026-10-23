import SwiftUI

struct RovaiNativeNormalContinue: View {
    let model: RovaiWorkspaceModel;let sourceCommandId: String;var composerScope: JSONValue = .null
    @Environment(\.dismiss) private var dismiss
    @State private var packet: JSONValue?
    @State private var receipt: JSONValue?
    @State private var issue: String?
    @State private var generation: UInt64?
    @State private var busy = false
    @State private var ready = false
    private var owner: String {"native-normal:\(sourceCommandId)"}
    private var preview: JSONValue? {packet?["preview"]}
    private var original: JSONValue? {packet?["original"]}
    private var disabled: Bool {busy || !ready || model.busy || model.pending != nil || generation != model.connectionGeneration}
    var body: some View {
        NavigationStack {List {
            if let issue {Section {Text(issue).font(.caption).foregroundStyle(Theme.warning)}}
            Section {Text("原程序已明确未接收。本次另行确认后按原会话的普通发送和排队规则处理。").font(.caption).foregroundStyle(Theme.textSecondary)}
            if let preview {Section("将发送的整份内容") {
                Text(preview["input"]?.text("text") ?? "").textSelection(.enabled)
                ForEach(preview["input"]?["materials"]?.rvArray ?? [], id: \.self) {material in Text(material.text("displayName") + " · \(material["byteSize"]?.rvInt ?? 0) B")}
                Text("当前普通输入框的正文、材料和引用保持原状。本次只发送以上整份输入。").font(.caption).foregroundStyle(Theme.textSecondary)
            }}
            if let receipt {Section("原普通提交结果") {
                let result = receipt["commandResult"] ?? .null
                Text(receipt.text("state") == "recorded" ? result.text("status") == "rejected" ? "本次未发送，完整输入保留" : "已按普通消息提交或排队" : receipt.text("state") == "failed" ? "未完成提交，完整输入保留" : receipt.text("state") == "prepared" ? "已登记，正在处理原请求" : "原结果尚未确定，只核对同一编号")
                Text(receipt.text("commandId")).font(.caption2).textSelection(.enabled)
                if result.text("status") == "rejected" {Text(result.text("code")).font(.caption).foregroundStyle(Theme.warning)}
                if let error = receipt["error"]?.stringValue {Text(error).font(.caption).foregroundStyle(Theme.warning)}
            }}
            if let preview, !["recorded", "failed"].contains(receipt?.text("state") ?? "") {Section {Button(original == nil ? "确认发送整份输入" : "核对原普通提交") {Task {await submit(preview)}}.disabled(disabled)}}
        }.navigationTitle("普通下一轮发送").navigationBarTitleDisplayMode(.inline).scrollContentBackground(.hidden).toolbar {Button("关闭并保留") {dismiss()}.disabled(busy)}}
        .interactiveDismissDisabled(busy)
        .task(id: sourceCommandId) {
            generation = model.connectionGeneration;busy = true;defer {busy = false}
            do {
                let raw = try model.readBody(owner)
                if !raw.isEmpty {let saved = try JSONDecoder().decode(JSONValue.self, from: Data(raw.utf8));guard saved["preview"]?.text("sourceCommandId") == sourceCommandId else {throw RovaiHostFailure(code: "原请求记录归属不一致")};packet = saved;if let original {try await reconcile(original)}}
                if packet == nil {let value = try await nativeUserCall(model, .runtimeSteerNormalPreview, .object(["schemaVersion": .number(1), "sourceCommandId": .string(sourceCommandId)]));try keep(.object(["preview": value]))};ready = true
            } catch {issue = error.localizedDescription}
        }
    }
    private func keep(_ value: JSONValue) throws {guard generation == model.connectionGeneration else {throw CancellationError()};try model.saveBody(owner, body: String(decoding: try JSONEncoder().encode(value), as: UTF8.self));packet = value}
    private func reconcile(_ input: JSONValue) async throws {let value = try await nativeUserCall(model, .runtimeSteerNormalReceipt, .object(["schemaVersion": .number(1), "commandId": input["commandId"] ?? .null]));guard value.text("commandId") == input.text("commandId") else {throw RovaiHostFailure(code: "原回执归属不一致")};receipt = value}
    private func submit(_ preview: JSONValue) async {
        guard !disabled else {return};busy = true;defer {busy = false};issue = nil
        do {
            if let original {try await reconcile(original);return}
            var fields: [String: JSONValue] = ["schemaVersion": .number(1), "commandId": .string(UUID().uuidString.lowercased()), "reference": .object(["sourceCommandId": .string(sourceCommandId), "previewId": preview["previewId"] ?? .null, "previewDigest": preview["previewDigest"] ?? .null])]
            if let scope = preview["scope"], scope.text("scopeKind") != "public" {
                let operation: RovaiOperation = scope.text("scopeKind") == "delegation_child" ? .delegationsConversationGet : .singleChatGet
                let current = try await nativeUserCall(model, operation, .object(["conversationId": scope["conversationId"] ?? .null, "composerScope": composerScope]))
                guard current["conversation"]?.text("id") == scope.text("conversationId") else {throw RovaiHostFailure(code: "当前私聊已变化，完整输入保留")};fields["draftRevision"] = current["draft"]?["revision"] ?? .number(0);fields["composerScope"] = composerScope
            }
            let input: JSONValue = .object(fields);try keep(.object(["preview": preview, "original": input]))
            let value = try await nativeUserCall(model, .runtimeSteerNormalConfirm, input);guard value.text("commandId") == input.text("commandId") else {throw RovaiHostFailure(code: "原回执归属不一致")};receipt = value
        } catch {issue = error.localizedDescription}
    }
}
