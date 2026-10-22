import SwiftUI

@MainActor private func feedbackCall(_ model: RovaiWorkspaceModel, _ action: String, _ payload: JSONValue = .object([:])) async throws -> JSONValue {
    let generation = model.connectionGeneration
    let value = try await model.query(.feedbackOperation, params: .object(["action": .string(action), "payload": payload]))
    guard generation == model.connectionGeneration else {throw CancellationError()};return value
}
private func feedbackStatus(_ value: JSONValue) -> String {
    switch value.text("status") {case "pending": "等待队员检查反馈";case "response_prepared": "已准备，正在核对写出";case "connection_written": "已送到智能体连接";case "unknown": "结果待核对";case "requires_user_review": "任务已变化，需要重新选择";case "continued_as_message": value["continuedMessageId"] == .null ? "已作为普通消息排队" : "已作为普通消息续办";default: value.text("status")}
}

/// 使用原模型设置的原生表单，保存原请求后才交给 Host。
struct RovaiMemberFeedbackSwitch: View {
    let model: RovaiWorkspaceModel;let agentId: String
    @State private var policy: JSONValue?, original: JSONValue?, issue: String?, generation: UInt64?
    @State private var busy = false
    private var owner: String {"feedback-policy:" + agentId}
    private var enabled: Bool {(policy?["members"]?.rvArray ?? []).first {$0.text("agentId") == agentId}?["enabled"]?.rvBool == true}
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Toggle("工作中接收补充", isOn: Binding(get: {enabled}, set: {selected in
                guard let policy else {return};let members = (policy["members"]?.rvArray ?? []).filter {$0.text("agentId") != agentId} + [.object(["agentId": .string(agentId), "enabled": .bool(selected)])]
                let input = JSONValue.object(["schemaVersion": .number(1), "expectedRevision": policy["revision"] ?? .number(0), "members": .array(members)]);original = input;Task {await save(input)}
            })).disabled(busy || policy == nil || original != nil || model.busy || model.pending != nil || generation != model.connectionGeneration)
            Text("默认关闭；开启后从新任务生效，队员主动检查时才会收到。").font(.caption).foregroundStyle(Theme.textSecondary)
            if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning)}
            if let original {Button("核对原设置") {Task {await save(original)}}.disabled(busy || generation != model.connectionGeneration)}
        }.task(id: agentId) {
            generation = model.connectionGeneration;busy = true;defer {busy = false}
            do {let raw = try model.readBody(owner);if !raw.isEmpty {original = try JSONDecoder().decode(JSONValue.self, from: Data(raw.utf8))};policy = try await feedbackCall(model, "policy.get")} catch {issue = error.localizedDescription}
        }
    }
    private func save(_ input: JSONValue) async {
        busy = true;defer {busy = false}
        do {let raw = String(decoding: try JSONEncoder().encode(input), as: UTF8.self);try model.saveBody(owner, body: raw);policy = try await feedbackCall(model, "policy.set", input);guard let generation, generation == model.connectionGeneration else {throw CancellationError()};_ = try model.clearBodyIfMatches(owner, expectedBody: raw, expectedGeneration: generation);original = nil;issue = nil} catch {issue = error.localizedDescription}
    }
}

/// 独立补充、完整预览和原记录。普通输入中的材料不参与反馈。
struct RovaiUserFeedback: View {
    let model: RovaiWorkspaceModel;let threadId: String
    var conversationId: String? = nil;var child = false;var agentId: String? = nil
    var normalInput: (() async throws -> JSONValue)? = nil
    var nativeMaterials: [JSONValue]? = nil
    @Environment(\.dismiss) private var dismiss
    @State private var text = "", selected = "", run = ""
    @State private var targets: [JSONValue] = [], records: [JSONValue] = [], previews: [JSONValue] = []
    @State private var next: String?, previewNext: String?, packet: JSONValue?, receipt: JSONValue?, issue: String?, generation: UInt64?
    @State private var busy = false, ready = false, uncertain = false
    @State private var nativeOpen = false
    private var scope: JSONValue {.object(["threadId": .string(threadId), "conversationId": conversationId.map(JSONValue.string) ?? .null, "scopeKind": .string(conversationId == nil ? "public" : child ? "delegation_child" : "single_chat")])}
    private var owner: String {"feedback:\(threadId):\(conversationId ?? "public"):\(child ? "child" : "member")"}
    private var disabled: Bool {busy || !ready || model.busy || model.pending != nil || generation != model.connectionGeneration}
    private var available: [JSONValue] {targets.filter {selected.isEmpty || $0.text("agentId") == selected}}
    var body: some View {
        NavigationStack {List {
            if let issue {Section {Text(issue).font(.caption).foregroundStyle(Theme.warning)}}
            if let packet {previewSection(packet)} else {
                Section("补充说明") {
                    if agentId == nil {Picker("查看哪位队员的反馈", selection: $selected) {Text("选择队员").tag("");ForEach((model.thread?["members"]?.rvArray ?? []).map {RovaiRow(value: $0)}) {row in Text(row.value.text("displayName")).tag(row.value.text("agentId"))}}}
                    TextField("例如：按钮用蓝色，先不要加动画。", text: $text, axis: .vertical).lineLimit(3...8).disabled(disabled)
                    Text("\(text.unicodeScalars.count)/4096").font(.caption).foregroundStyle(Theme.textSecondary)
                    Picker("接收补充的当前任务", selection: $run) {Text("选择任务").tag("");ForEach(available, id: \.self) {target in Text(target.text("displayName") + " · " + target.text("agentRunId")).tag(target.text("agentRunId"))}}.disabled(disabled)
                    Button("查看完整内容") {Task {await prepare()}}.disabled(disabled || targets.first {$0.text("agentRunId") == run}?["feedbackEnabled"]?.rvBool != true || targets.first {$0.text("agentRunId") == run}?["nativeBindingReady"]?.rvBool != true || text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || text.unicodeScalars.count > 4096)
                    Button("核对记录") {Task {await refresh()}}.disabled(disabled)
                    Button("预览原生插话及当前材料") {nativeOpen = true}.disabled(disabled)
                    if targets.isEmpty {Text("当前没有可接收反馈的运行任务。").font(.caption).foregroundStyle(Theme.textSecondary)}
                } footer: {Text("先预览全文，再交给队员。送到连接不代表模型已阅读。")}
                Section("原预览") {ForEach(previews.filter {$0["receipt"] == nil || $0["receipt"] == .null}, id: \.self) {row in
                    VStack(alignment: .leading, spacing: 8) {Text(row["preview"]?.text("text") ?? "").textSelection(.enabled);Button("继续核对原预览") {do {try keep(.object(["kind": .string("feedback"), "preview": row["preview"] ?? .null]));receipt = nil;uncertain = false} catch {issue = error.localizedDescription}}.disabled(disabled)}
                }}
                Section("反馈记录") {ForEach(records, id: \.self) {row in historyRow(row)};if next != nil || previewNext != nil {Button("更多记录") {Task {await refresh(more: true)}}.disabled(disabled)}}
            }
        }.navigationTitle("工作中补充和反馈记录").navigationBarTitleDisplayMode(.inline).scrollContentBackground(.hidden).toolbar {Button("关闭") {dismiss()}.disabled(busy)}}
        .interactiveDismissDisabled(busy)
        .sheet(isPresented: $nativeOpen) {RovaiNativeSteer(model: model, scope: scope, text: text, agentId: agentId, materials: nativeMaterials ?? (conversationId == nil ? model.thread?["composerDraft"]?["attachments"]?.rvArray ?? [] : []))}
        .task {
            generation = model.connectionGeneration;selected = agentId ?? "";busy = true;defer {busy = false}
            do {text = try model.readBody(owner + ":text");let raw = try model.readBody(owner + ":request");if !raw.isEmpty {packet = try JSONDecoder().decode(JSONValue.self, from: Data(raw.utf8));uncertain = packet?["original"] != nil};try await read();ready = true} catch {issue = error.localizedDescription}
        }
        .onChange(of: text) {_, value in guard ready else {return};do {try model.saveBody(owner + ":text", body: value)} catch {issue = error.localizedDescription}}
        .onChange(of: selected) {_, _ in guard ready else {return};run = "";Task {await refresh()}}
    }
    private func historyRow(_ row: JSONValue) -> some View {
        let record = row["receipt"] ?? .null
        return VStack(alignment: .leading, spacing: 8) {
            Text(feedbackStatus(record)).font(.headline);Text(row.text("text")).textSelection(.enabled);Text(record.text("feedbackId")).font(.caption2).foregroundStyle(Theme.textSecondary)
            if ["pending", "requires_user_review"].contains(record.text("status")), record.text("deliveryChannel") == "none", conversationId == nil || normalInput != nil {Button("预览普通消息续办") {Task {await prepareNormal(record)}}.disabled(disabled)}
            if ["unknown", "response_prepared"].contains(record.text("status")) {Text("原结果尚未确定，不重复发送或改投其他任务。").font(.caption).foregroundStyle(Theme.textSecondary)}
        }.padding(.vertical, 4)
    }
    private func previewSection(_ frozen: JSONValue) -> some View {
        let preview = frozen["preview"] ?? .null, normal = frozen.text("kind") == "normal"
        return Section(normal ? "确认按普通消息继续" : "将交给队员的完整内容") {
            Text(preview.text("text")).textSelection(.enabled);Text(preview.text("agentRunId")).font(.caption2).foregroundStyle(Theme.textSecondary)
            if let receipt {Text(feedbackStatus(receipt)).font(.caption)}
            if uncertain {Text("提交结果待核对；原文和编号保留。").font(.caption).foregroundStyle(Theme.textSecondary)}
            Button(uncertain ? "核对原请求" : normal ? "确认作为普通消息发送" : "确认交给队员") {Task {await submit(frozen)}}.disabled(disabled || receipt != nil)
            Button("返回并保留原文") {do {try keep(nil);receipt = nil;uncertain = false;Task {await refresh()}} catch {issue = error.localizedDescription}}.disabled(disabled)
        }
    }
    private func keep(_ value: JSONValue?) throws {try model.saveBody(owner + ":request", body: value.map {String(decoding: try JSONEncoder().encode($0), as: UTF8.self)} ?? "");packet = value}
    private func read(more: Bool = false) async throws {
        let response = try await feedbackCall(model, "targets", .object(["scope": scope]));targets = (response["targets"]?.rvArray ?? []).filter {agentId == nil || $0.text("agentId") == agentId}
        if run.isEmpty, targets.count == 1 {run = targets[0].text("agentRunId");selected = targets[0].text("agentId")}
        guard !selected.isEmpty else {return}
        let params: [String: JSONValue] = ["scope": scope, "agentId": .string(selected)]
        var first = params;first["after"] = more ? next.map(JSONValue.string) ?? .null : .null
        var second = params;second["after"] = more ? previewNext.map(JSONValue.string) ?? .null : .null
        let a = try await feedbackCall(model, "list", .object(first)), b = try await feedbackCall(model, "previews.list", .object(second))
        let rows = a["items"]?.rvArray ?? [], views = b["items"]?.rvArray ?? [];records = more ? records + rows.filter {row in !records.contains {$0["receipt"]?.text("feedbackId") == row["receipt"]?.text("feedbackId")}} : rows;previews = more ? previews + views.filter {row in !previews.contains {$0["preview"]?.text("previewId") == row["preview"]?.text("previewId")}} : views
        next = a["next"]?.stringValue;previewNext = b["next"]?.stringValue
    }
    private func refresh(more: Bool = false) async {guard !busy else {return};busy = true;defer {busy = false};do {try await read(more: more);issue = nil} catch {issue = error.localizedDescription}}
    private func prepare() async {
        guard let target = targets.first(where: {$0.text("agentRunId") == run}), !disabled else {return};busy = true;defer {busy = false}
        do {let value = try await feedbackCall(model, "preview", .object(["schemaVersion": .number(1), "scope": scope, "agentId": target["agentId"] ?? .null, "agentRunId": target["agentRunId"] ?? .null, "executionEpoch": target["executionEpoch"] ?? .null, "source": .object(["kind": .string("live_note")]), "text": .string(text), "title": .string(""), "fields": .array([])]));try keep(.object(["kind": .string("feedback"), "preview": value]));receipt = nil;uncertain = false;issue = nil} catch {issue = error.localizedDescription}
    }
    private func prepareNormal(_ record: JSONValue) async {
        busy = true;defer {busy = false};do {let result = try await feedbackCall(model, "normal.preview", .object(["scope": scope, "agentId": record["agentId"] ?? .null, "feedbackId": record["feedbackId"] ?? .null]));try keep(.object(["kind": .string("normal"), "preview": result["preview"] ?? .null, "reference": result["reference"] ?? .null]));receipt = nil;uncertain = false;issue = nil} catch {issue = error.localizedDescription}
    }
    private func submit(_ frozen: JSONValue) async {
        guard !disabled else {return};busy = true;defer {busy = false};let preview = frozen["preview"] ?? .null, normal = frozen.text("kind") == "normal"
        do {
            let prior = frozen["original"];var original = prior
            if original == nil {
                if normal {
                    var input: [String: JSONValue] = ["commandId": preview["previewId"] ?? .null, "reference": frozen["reference"] ?? .null, "scope": scope, "agentId": preview["agentId"] ?? .null, "body": preview["text"] ?? .null]
                    if conversationId != nil {guard let normalInput else {throw RovaiHostFailure(code: "请先处理普通输入中的草稿和材料")};let value = try await normalInput();if case .object(let fields) = value {input.merge(fields) {_, new in new}} else {throw RovaiHostFailure(code: "普通输入状态无法确认")}}
                    original = .object(input)
                } else {original = .object(["schemaVersion": .number(1), "commandId": preview["previewId"] ?? .null, "previewId": preview["previewId"] ?? .null, "previewDigest": preview["previewDigest"] ?? .null])}
            }
            guard let original else {return};var saved: [String: JSONValue] = [:];if case .object(let fields) = frozen {saved = fields};saved["original"] = original;try keep(.object(saved));uncertain = true
            let result = try await feedbackCall(model, normal ? prior == nil ? "normal.send" : "normal.reconcile" : "submit", original)
            if normal {guard result.text("state") == "recorded" else {throw RovaiHostFailure(code: "还没有查到原请求结果，请稍后继续核对")};guard result["commandResult"]?.text("status") != "rejected" else {throw RovaiHostFailure(code: result["commandResult"]?.text("code") ?? "续办未发送")};receipt = result["receipt"]} else {receipt = result;if let generation, preview.text("text") == text {_ = try model.clearBodyIfMatches(owner + ":text", expectedBody: text, expectedGeneration: generation);text = ""}}
            uncertain = false;try keep(nil);try await read();issue = nil
        } catch {issue = error.localizedDescription}
    }
}
