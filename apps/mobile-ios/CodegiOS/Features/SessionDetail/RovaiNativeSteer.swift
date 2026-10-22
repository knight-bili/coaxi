import SwiftUI

@MainActor func nativeUserCall(_ model: RovaiWorkspaceModel, _ operation: RovaiOperation, _ input: JSONValue) async throws -> JSONValue {
    let generation = model.connectionGeneration
    let value = try await model.query(operation, params: input)
    guard generation == model.connectionGeneration else {throw CancellationError()}
    return value
}
struct RovaiMemberNativeSteerSwitch: View {
    let model: RovaiWorkspaceModel;let agentId: String
    @State private var policy: JSONValue?, original: JSONValue?, issue: String?, generation: UInt64?
    @State private var busy = false
    private var owner: String {"native-steer-policy:\(agentId)"}
    private var enabled: Bool {(policy?["members"]?.rvArray ?? []).contains {$0.text("agentId") == agentId && $0["enabled"]?.rvBool == true}}
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Toggle("原生插话", isOn: Binding(get: {enabled}, set: {selected in
                guard let policy, !busy, original == nil else {return}
                let members = (policy["members"]?.rvArray ?? []).filter {$0.text("agentId") != agentId} + [.object(["agentId": .string(agentId), "enabled": .bool(selected)])]
                let input: JSONValue = .object(["schemaVersion": .number(1), "commandId": .string(UUID().uuidString.lowercased()), "expectedRevision": policy["revision"] ?? .number(0), "members": .array(members)])
                Task {await save(input)}
            })).disabled(busy || policy == nil || original != nil || model.busy || model.pending != nil || generation != model.connectionGeneration)
            Text("默认关闭；仅之后的新任务生效，仍需实际连接支持。").font(.caption).foregroundStyle(Theme.textSecondary)
            if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning)}
            if let original {Button("核对原保存请求") {Task {await reconcile(original)}}.disabled(busy || generation != model.connectionGeneration)}
        }.task(id: agentId) {
            generation = model.connectionGeneration;busy = true;defer {busy = false}
            do {let raw = try model.readBody(owner);if !raw.isEmpty {original = try JSONDecoder().decode(JSONValue.self, from: Data(raw.utf8))};try await read()} catch {issue = error.localizedDescription}
        }
    }
    private func read() async throws {policy = try await nativeUserCall(model, .runtimeSteerPolicyGet, .object(["schemaVersion": .number(1)]))}
    private func accept(_ value: JSONValue, _ input: JSONValue) async throws {
        guard value.text("commandId") == input.text("commandId"), value.text("commandType") == "runtime.steer.policy.set", ["applied", "accepted", "rejected"].contains(value.text("status")), let generation, generation == model.connectionGeneration else {throw RovaiHostFailure(code: "原请求尚无确定回执或归属不同")}
        let raw = String(decoding: try JSONEncoder().encode(input), as: UTF8.self)
        _ = try model.clearBodyIfMatches(owner, expectedBody: raw, expectedGeneration: generation);original = nil
        if value.text("status") == "rejected" {try await read();throw RovaiHostFailure(code: "本次设置未保存：" + value.text("code"))}
        policy = value["payload"]
    }
    private func save(_ input: JSONValue) async {
        guard !busy, generation == model.connectionGeneration else {return};busy = true;defer {busy = false};issue = nil
        do {let raw = String(decoding: try JSONEncoder().encode(input), as: UTF8.self);try model.saveBody(owner, body: raw);original = input;let result = try await nativeUserCall(model, .runtimeSteerPolicySet, input);try await accept(result, input)} catch {issue = error.localizedDescription}
    }
    private func reconcile(_ input: JSONValue) async {
        guard !busy else {return};busy = true;defer {busy = false};issue = nil
        do {let value = try await nativeUserCall(model, .commandsReconcile, .object(["operation": .string("runtime.steer.policy.set"), "params": .object(["commandId": input["commandId"] ?? .null]) ]));guard value.text("state") == "recorded", let result = value["result"] else {throw RovaiHostFailure(code: "原请求尚无确定回执，未重复发送")};try await accept(result, input)} catch {issue = error.localizedDescription}
    }
}

func nativeSteerState(_ receipt: JSONValue) -> String {
    switch receipt.text("status") {
    case "prepared": return "已登记，正在发送原请求"
    case "connection_written": return "已写入原生连接"
    case "native_acknowledged": return "原程序已明确接收"
    case "not_consumed": return "原程序未接收，整份输入保留"
    case "requires_user_review": return "原程序已接收但另开轮次，需要核对"
    case "unknown": return "结果待核对，不重复发送"
    default: return receipt.text("status")
    }
}

/// 沿用 Codeg iOS 的 List、Section、原生表单；材料默认不选择。
struct RovaiNativeSteer: View {
    let model: RovaiWorkspaceModel;let scope: JSONValue;let text: String
    var agentId: String? = nil;var materials: [JSONValue] = [];var composerScope: JSONValue = .null
    @Environment(\.dismiss) private var dismiss
    @State private var targets: [JSONValue] = [], records: [JSONValue] = [], run = "", checked: Set<String> = []
    @State private var packet: JSONValue?, receipt: JSONValue?, next: String?, issue: String?, generation: UInt64?
    @State private var busy = false, ready = false
    @State private var normalOpen = false
    private var owner: String {"native-steer:\(scope.prettyString):\(agentId ?? "")"}
    private var disabled: Bool {busy || !ready || model.busy || model.pending != nil || generation != model.connectionGeneration}
    private var preview: JSONValue? {packet?["preview"]}
    private var original: JSONValue? {packet?["original"]}
    var body: some View {
        NavigationStack {List {
            if let issue {Section {Text(issue).font(.caption).foregroundStyle(Theme.warning)}}
            if let preview {
                Section("将发送的完整内容") {
                    Text(preview["input"]?.text("text") ?? "").textSelection(.enabled)
                    ForEach(preview["input"]?["materials"]?.rvArray ?? [], id: \.self) {material in Text(material.text("displayName") + " · \(material["byteSize"]?.rvInt ?? 0) B")}
                    if let receipt {Text(nativeSteerState(receipt)).font(.caption);Text(receipt.text("commandId")).font(.caption2).textSelection(.enabled)
                        if receipt.text("nativeOutcome") == "startedNewTurn" {Text("输入已被原程序消耗，不要再次排队；该连接的直接插话已停用。").font(.caption)}
                        if receipt.text("status") == "not_consumed" {Text("整份输入仍保存在原预览，作为普通下一轮发送须另外确认。").font(.caption)}
                        if receipt.text("status") == "not_consumed" {Button("预览整份普通下一轮") {normalOpen = true}.disabled(disabled)}
                    }
                    Button(original == nil ? "确认发送完整补充" : "核对原提交") {Task {await submit(preview)}}.disabled(disabled || receipt != nil && original == nil)
                    Text("普通输入和其他材料保留。送到连接不代表模型已阅读。").font(.caption).foregroundStyle(Theme.textSecondary)
                }
            } else {
                Section("接收完整补充的当前任务") {
                    Picker("队员和任务", selection: $run) {Text("选择任务").tag("");ForEach(targets, id: \.self) {target in Text(target.text("agentId") + " · " + target.text("agentRunId")).tag(target.text("agentRunId"))}}.disabled(disabled)
                    Text(text).textSelection(.enabled)
                    ForEach(materials, id: \.self) {material in Toggle(material.text("displayName"), isOn: Binding(get: {checked.contains(material.text("id"))}, set: {value in if value {checked.insert(material.text("id"))} else {checked.remove(material.text("id"))}})).disabled(disabled || material.text("kind") != "file" || !["", "unknown", "available"].contains(material.text("availability")))}
                    Button("预览整份文字和材料") {Task {await prepare()}}.disabled(disabled || targets.first {$0.text("agentRunId") == run} == nil || text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && checked.isEmpty || text.unicodeScalars.count > 4096)
                    if targets.isEmpty {Text("当前没有实际支持原生插话且已启用的任务。").font(.caption).foregroundStyle(Theme.textSecondary)}
                }
            }
            Section("原生插话记录") {
                ForEach(records.filter {agentId == nil || $0["preview"]?["target"]?.text("agentId") == agentId}, id: \.self) {row in
                    VStack(alignment: .leading, spacing: 8) {Text(row["preview"]?["input"]?.text("text") ?? "").textSelection(.enabled);Text(nativeSteerState(row["receipt"] ?? .null)).font(.caption);Button("查看完整内容和原回执") {do {
                        var fields: [String: JSONValue] = ["preview": row["preview"] ?? .null]
                        if let previous = row["receipt"], previous != .null {fields["original"] = .object(["schemaVersion": .number(1), "commandId": previous["commandId"] ?? .null, "previewId": row["preview"]?["previewId"] ?? .null, "previewDigest": row["preview"]?["previewDigest"] ?? .null]);receipt = previous} else {receipt = nil}
                        try keep(.object(fields))
                    } catch {issue = error.localizedDescription}}.disabled(disabled)}
                }
                if next != nil {Button("更多记录") {Task {await refresh(more: true)}}.disabled(disabled)}
            }
        }.navigationTitle("完整原生插话").navigationBarTitleDisplayMode(.inline).scrollContentBackground(.hidden).toolbar {Button("关闭并保留") {dismiss()}.disabled(busy)}}
        .interactiveDismissDisabled(busy)
        .sheet(isPresented: $normalOpen) {if let receipt {RovaiNativeNormalContinue(model: model, sourceCommandId: receipt.text("commandId"), composerScope: composerScope)}}
        .task(id: scope.prettyString) {
            generation = model.connectionGeneration;busy = true;defer {busy = false}
            do {
                let raw = try model.readBody(owner);if !raw.isEmpty {packet = try JSONDecoder().decode(JSONValue.self, from: Data(raw.utf8));if let original {try await reconcile(original)}}
                let value = try await nativeUserCall(model, .runtimeSteerTargets, .object(["schemaVersion": .number(1), "scope": scope]));targets = (value["targets"]?.rvArray ?? []).filter {agentId == nil || $0.text("agentId") == agentId};if targets.count == 1 {run = targets[0].text("agentRunId")};try await read();ready = true
            } catch {issue = error.localizedDescription}
        }
        .onChange(of: run) {_, _ in checked = []}
    }
    private func keep(_ value: JSONValue) throws {guard generation == model.connectionGeneration else {throw CancellationError()};try model.saveBody(owner, body: String(decoding: try JSONEncoder().encode(value), as: UTF8.self));packet = value}
    private func read(more: Bool = false) async throws {
        let value = try await nativeUserCall(model, .runtimeSteerPreviews, .object(["schemaVersion": .number(1), "scope": scope, "after": more ? next.map(JSONValue.string) ?? .null : .null, "limit": .number(50)]))
        let rows = value["items"]?.rvArray ?? [];records = more ? records + rows.filter {row in !records.contains {$0["preview"]?.text("previewId") == row["preview"]?.text("previewId")}} : rows;next = value["next"]?.stringValue
    }
    private func refresh(more: Bool = false) async {guard !busy else {return};busy = true;defer {busy = false};do {try await read(more: more)} catch {issue = error.localizedDescription}}
    private func prepare() async {
        guard !disabled, let target = targets.first(where: {$0.text("agentRunId") == run}) else {return};busy = true;defer {busy = false};issue = nil
        do {
            var frozen: JSONValue = .array([])
            if !checked.isEmpty {let value = try await nativeUserCall(model, .runtimeSteerMaterialsPrepare, .object(["schemaVersion": .number(1), "target": target, "sourceIds": .array(checked.sorted().map(JSONValue.string)), "composerScope": composerScope]));frozen = value["materials"] ?? .array([])}
            let preview = try await nativeUserCall(model, .runtimeSteerPreview, .object(["schemaVersion": .number(1), "target": target, "input": .object(["text": .string(text), "materials": frozen]) ]));try keep(.object(["preview": preview]));receipt = nil
        } catch {issue = error.localizedDescription}
    }
    private func reconcile(_ input: JSONValue) async throws {
        let value = try await nativeUserCall(model, .runtimeSteerReceipt, .object(["schemaVersion": .number(1), "commandId": input["commandId"] ?? .null]))
        guard value.text("commandId") == input.text("commandId") else {throw RovaiHostFailure(code: "原回执归属不一致")};receipt = value
    }
    private func submit(_ preview: JSONValue) async {
        guard !disabled else {return};busy = true;defer {busy = false};issue = nil
        do {
            if let original {try await reconcile(original);return}
            let request: JSONValue = .object(["schemaVersion": .number(1), "commandId": .string(UUID().uuidString.lowercased()), "previewId": preview["previewId"] ?? .null, "previewDigest": preview["previewDigest"] ?? .null])
            try keep(.object(["preview": preview, "original": request]))
            let value = try await nativeUserCall(model, .runtimeSteerConfirm, request);guard value.text("commandId") == request.text("commandId") else {throw RovaiHostFailure(code: "原回执归属不一致")};receipt = value
        } catch {issue = error.localizedDescription}
    }
}
