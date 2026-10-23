import SwiftUI

/// 原 WebDAV 设置页中的主机任务控制，沿现有 Form 与原回执，不另建手机调度器。
struct RovaiConfigurationSyncSettings: View {
    let model: RovaiWorkspaceModel
    @Environment(\.dismiss) private var dismiss
    @State private var saved: JSONValue = .object([:])
    @State private var draft: JSONValue = .object([:])
    @State private var identity = ""
    @State private var passphrase = ""
    @State private var message = ""
    @State private var pending: JSONValue?
    @State private var issue: String?
    @State private var clear = false
    @State private var dirty = false
    @State private var busy = false
    @State private var leaving = false
    @State private var archiving = false
    private var owner: String { "portable-sync/" + identity }
    private var active: String { saved["state"]?.text("activeCommandId") ?? "" }
    private var phase: String { saved["state"]?.text("phase") ?? "" }
    var body: some View {
        Form {
            Section { Text("默认仅上传界面偏好。完整配置和密钥分别选择；另一台主机下载后仍须逐项选择，不自动工作。").foregroundStyle(Theme.textSecondary) }
            Section("上传范围") {
                Toggle("启用这台主机的配置上传", isOn: toggle("enabled"))
                Toggle("自动上传有变化的配置", isOn: toggle("autoSync"))
                Toggle("同时上传完整的模型、智能体和模板配置", isOn: toggle("fullConfiguration"))
                Toggle("完整加密副本中包含密钥", isOn: toggle("includeSecrets")).disabled(draft["fullConfiguration"] != .bool(true))
                TextField("检查间隔（1至1440分钟）", text: Binding(get: { if let value = draft["intervalMinutes"]?.rvInt { String(value) } else { draft.text("intervalMinutes") } }, set: { field("intervalMinutes", Double($0).map(JSONValue.number) ?? .string($0)) })).keyboardType(.numberPad)
                SecureField(saved["passphraseSet"] == .bool(true) ? "新加密口令（留空保留原口令）" : "配置加密口令（至少8个字符）", text: $passphrase).textInputAutocapitalization(.never).autocorrectionDisabled().onChange(of: passphrase) { _, _ in dirty = true }
                Toggle("清除已存的加密口令", isOn: $clear).onChange(of: clear) { _, _ in dirty = true }
                Button("保存设置") { work { try await save() } }.disabled(!dirty || !(1...1440).contains(draft["intervalMinutes"]?.rvInt ?? 0))
            }.disabled(busy || !active.isEmpty)
            if dirty { Section { Button("放弃未保存修改") { accept(saved) }.disabled(busy) } }
            Section("主机任务") {
                Text(active.isEmpty ? "主机当前没有活动上传" : "原上传 · " + phase)
                if let date = saved["state"]?.text("lastSyncAt"), !date.isEmpty { LabeledContent("上次实际上传", value: date) }
                Button("刷新主机状态") { work { try await refresh() } }
                if pending != nil { Text("本次原请求已保留，不自动另建上传。").font(.caption); Button("核对原上传") { work { try await reconcile() } } }
                Button(pending == nil ? "立即上传当前配置" : "继续原编号上传") { work { try await upload() } }.disabled(dirty || !active.isEmpty || saved["enabled"] != .bool(true) || saved["passphraseSet"] != .bool(true))
                if !active.isEmpty {
                    Button("停止原任务", role: .destructive) { work { _ = try await request("cancel", ["expectedRevision": saved["revision"] ?? .null, "commandId": .string(active)]); try await refresh() } }
                    if phase == "unknown" { Button("恢复原上传") { work { _ = try await request("resume", ["expectedRevision": saved["revision"] ?? .null, "commandId": .string(active)]); try await refresh() } } }
                }
                if (phase == "unknown" || pending != nil) && !dirty { Button("保留未决记录后另建上传…") { archiving = true } }
                if let error = saved["state"]?.text("lastError"), !error.isEmpty { Text(error).font(.caption).foregroundStyle(.red) }
            }.disabled(busy || identity.isEmpty)
            if !message.isEmpty { Section { Text(message) } }; if let issue { Section { Text(issue).foregroundStyle(.red) } }; if busy { ProgressView() }
        }.navigationTitle("主机配置自动上传").navigationBarTitleDisplayMode(.inline)
        .navigationBarBackButtonHidden(dirty || busy).interactiveDismissDisabled(dirty || busy)
        .toolbar { if dirty || busy { ToolbarItem(placement: .cancellationAction) { Button("返回") { leaving = true }.disabled(busy) } } }
        .task(id: model.connected) { await start() }.onDisappear { passphrase = "" }
        .confirmationDialog("放弃未保存修改？", isPresented: $leaving, titleVisibility: .visible) { Button("放弃并返回", role: .destructive) { dismiss() }; Button("继续编辑", role: .cancel) {} }
        .confirmationDialog("保留原记录并继续新上传", isPresented: $archiving, titleVisibility: .visible) {
            Button("确认保留记录并继续") { work { try await archiveOriginal() } }; Button("返回", role: .cancel) {}
        } message: { Text("会保留原任务的未决记录，之后的上传使用新编号。远端可能已有原副本；正在运行的任务不能直接跳过。") }
    }
    private func current() async throws { guard !identity.isEmpty, try await model.presentationIdentity() == identity else { throw CancellationError() } }
    private func request(_ operation: String, _ extra: [String: JSONValue] = [:]) async throws -> JSONValue {
        var fields = extra; fields["operation"] = .string(operation); let result = try await model.privateConfiguration("sync", params: .object(fields)); try await current(); return result
    }
    private func work(_ action: @escaping () async throws -> Void) { guard !busy else { return }; busy = true; issue = nil; Task { do { try await current(); try await action() } catch is CancellationError {} catch { issue = error.localizedDescription }; busy = false } }
    private func field(_ key: String, _ next: JSONValue) {
        guard case .object(var fields) = draft else { return }; fields[key] = next
        if key == "fullConfiguration" && next == .bool(false) { fields["includeSecrets"] = .bool(false) }; draft = .object(fields); dirty = true
    }
    private func toggle(_ key: String) -> Binding<Bool> { Binding(get: { draft[key] == .bool(true) }, set: { field(key, .bool($0)) }) }
    private func accept(_ value: JSONValue) { saved = value; draft = value; passphrase = ""; clear = false; dirty = false }
    private func refresh() async throws { let value = try await request("get"); saved = value; if !dirty { draft = value } }
    private func start() async {
        identity = ""; pending = nil; passphrase = ""; busy = true; issue = nil
        do {
            identity = try await model.presentationIdentity(); let raw = try model.readBody(owner)
            if !raw.isEmpty { guard let value = JSONValue.parse(raw), case .object(let fields) = value, Set(fields.keys) == Set(["commandId", "expectedRevision"]), UUID(uuidString: value.text("commandId")) != nil else { throw RovaiHostFailure(code: "原上传恢复记录无效，已保留") }; pending = value }
            accept(try await request("get")); if pending != nil { try await reconcile() }
        } catch is CancellationError {} catch { issue = error.localizedDescription }; busy = false
    }
    private func save() async throws {
        var fields: [String: JSONValue] = ["expectedRevision": draft["revision"] ?? .null, "passphrase": .string(passphrase), "clearPassphrase": .bool(clear)]
        for key in ["enabled", "autoSync", "fullConfiguration", "includeSecrets", "intervalMinutes"] { fields[key] = draft[key] ?? (key == "intervalMinutes" ? .number(5) : .bool(false)) }
        accept(try await request("set", fields)); message = "配置同步设置已保存"
    }
    private func upload() async throws {
        let original = pending ?? .object(["commandId": .string(UUID().uuidString.lowercased()), "expectedRevision": saved["revision"] ?? .null])
        if pending == nil { try model.saveBody(owner, body: original.prettyString); pending = original }
        _ = try await request("upload", ["commandId": original["commandId"] ?? .null, "expectedRevision": original["expectedRevision"] ?? .null]); try await refresh()
    }
    private func reconcile() async throws {
        guard let pending else { return }; let result = try await request("receipt", ["commandId": pending["commandId"] ?? .null])
        guard result["commandId"] == pending["commandId"] else { throw RovaiHostFailure(code: "原上传编号不同") }
        if result.text("state") == "completed" { try model.saveBody(owner, body: ""); self.pending = nil; message = "原配置上传已完成" }
        else { message = result.text("state") == "preparing" ? "主机正在准备原副本" : "原上传尚待核对，未新建任务" }; try await refresh()
    }
    private func archiveOriginal() async throws {
        if !active.isEmpty { _ = try await request("archive", ["expectedRevision": saved["revision"] ?? .null, "commandId": .string(active), "confirmed": .bool(true)]) }
        if let pending { try model.saveBody(owner + "/archived/" + pending.text("commandId"), body: pending.prettyString) }
        try model.saveBody(owner, body: ""); pending = nil; accept(try await request("get")); message = "未决记录已保留，可明确开始下一次上传"
    }
}
