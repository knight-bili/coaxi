import SwiftUI

struct RovaiNativeEvidence: View {
    let model: RovaiWorkspaceModel;let commandId: String
    @State private var value: JSONValue?, issue: String?
    @State private var open = false, busy = false
    var body: some View {
        DisclosureGroup("原投递与普通续办记录", isExpanded: $open) {
            Text("这里只读取原记录，不会重发。送到连接或取得原程序回执，不代表模型已经阅读。").font(.caption).foregroundStyle(Theme.textSecondary)
            if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning)}
            Button(busy ? "正在读取…" : "读取原投递记录") {Task {await read()}}.disabled(busy)
            if let value {Text(value.prettyString).font(.caption.monospaced()).textSelection(.enabled)}
        }
    }
    private func read() async {
        guard !busy else {return};busy = true;defer {busy = false};issue = nil
        do {let next = try await nativeUserCall(model, .runtimeSteerEvidence, .object(["schemaVersion": .number(1), "commandId": .string(commandId)]));guard next["receipt"]?.text("commandId") == commandId else {throw RovaiHostFailure(code: "原回执归属不一致")};value = next} catch {issue = error.localizedDescription}
    }
}

struct RovaiNativeCreationRecords: View {
    let model: RovaiWorkspaceModel;let entry: JSONValue
    @Environment(\.dismiss) private var dismiss
    @State private var rows: [JSONValue] = [], overrides: [String: JSONValue] = [:]
    @State private var after: String?, issue: String?, working: JSONValue?, generation: UInt64?
    @State private var busy = false
    var body: some View {
        NavigationStack {List {
            if let issue {Section {Text(issue).font(.caption).foregroundStyle(Theme.warning)}}
            Section {Button("刷新原记录") {Task {await refresh()}}.disabled(busy)}
            ForEach(rows, id: \.self) {row in
                let preview = row["preview"] ?? .null, id = preview.text("previewId"), receipt = overrides[id] ?? row["receipt"] ?? .null
                Section {
                    Text(preview.text("title")).font(.headline)
                    Text(preview.text("mode") == "continue" ? "继续原生会话" : preview.text("mode") == "fork_point" ? "从指定回答分叉" : "从会话末尾分叉").font(.caption).foregroundStyle(Theme.textSecondary)
                    Text(LocalizedStringKey(receipt == .null ? "只有预览，尚未确认创建" : nativeCreationState(receipt)))
                    if let reason = preview["reason"]?.stringValue {Text(reason).font(.caption).foregroundStyle(Theme.textSecondary)}
                    if receipt != .null {
                        Text(receipt.text("commandId")).font(.caption2).textSelection(.enabled)
                        if let session = receipt["reportedSessionId"]?.stringValue {LabeledContent("原程序会话编号", value: session).font(.caption).textSelection(.enabled)}
                        if let error = receipt["error"]?.stringValue {Text(error).font(.caption).foregroundStyle(Theme.warning)}
                        Button("核对原创建结果") {Task {await reconcile(receipt, id)}}.disabled(busy || generation != model.connectionGeneration)
                        if receipt.text("state") == "binding_recorded" {Button("打开新工作私聊") {working = receipt}.disabled(busy || model.busy || model.pending != nil || generation != model.connectionGeneration)}
                    }
                    DisclosureGroup("来源与准备记录") {Text(JSONValue.object(["source": preview["source"] ?? .null, "previewId": preview["previewId"] ?? .null, "receipt": receipt, "preparation": row["preparation"] ?? .null]).prettyString).font(.caption.monospaced()).textSelection(.enabled)}
                }
            }
            if rows.isEmpty && !busy {Text("这份来源还没有继续或分叉记录。").font(.caption).foregroundStyle(Theme.textSecondary)}
            if after != nil {Button("更多原记录") {Task {await refresh(more: true)}}.disabled(busy)}
        }.navigationTitle("原生继续与分叉原记录").navigationBarTitleDisplayMode(.inline).scrollContentBackground(.hidden).toolbar {Button("关闭") {dismiss()}.disabled(busy)}}
        .interactiveDismissDisabled(busy)
        .task(id: entry.text("id")) {generation = model.connectionGeneration;await refresh()}
        .sheet(isPresented: Binding(get: {working != nil}, set: {if !$0 {working = nil}})) {if let working {RovaiPrivateConversationView(model: model, threadId: working.text("threadId"), initialConversationId: working.text("conversationId"))}}
    }
    private func refresh(more: Bool = false) async {
        guard !busy else {return};busy = true;defer {busy = false};issue = nil
        do {
            let value = try await nativeUserCall(model, .historyExecutionRecords, .object(["schemaVersion": .number(1), "sourceThreadId": entry["linkedThreadId"] ?? .null, "importId": entry["id"] ?? .null, "after": more ? after.map(JSONValue.string) ?? .null : .null, "limit": .number(50)]))
            let next = value["items"]?.rvArray ?? [];rows = more ? rows + next.filter {row in !rows.contains {$0["preview"]?.text("previewId") == row["preview"]?.text("previewId")}} : next;after = value["next"]?.stringValue;if !more {overrides = [:]}
        } catch {issue = error.localizedDescription}
    }
    private func reconcile(_ receipt: JSONValue, _ previewId: String) async {
        guard !busy else {return};busy = true;defer {busy = false};issue = nil
        do {let value = try await nativeUserCall(model, .historyExecutionReceipt, .object(["schemaVersion": .number(1), "commandId": receipt["commandId"] ?? .null]));guard value.text("commandId") == receipt.text("commandId") else {throw RovaiHostFailure(code: "原回执归属不一致")};overrides[previewId] = value} catch {issue = error.localizedDescription}
    }
}
