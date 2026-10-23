import SwiftUI

struct RovaiWebdavSettings: View {
    let model: RovaiWorkspaceModel
    @Environment(\.dismiss) private var dismiss
    @State private var saved: JSONValue = .object([:])
    @State private var endpoint = ""
    @State private var username = ""
    @State private var password = ""
    @State private var clearPassword = false
    @State private var insecure = false
    @State private var dirty = false
    @State private var busy = false
    @State private var leaving = false
    @State private var snapshots: [JSONValue] = []
    @State private var transfers: [JSONValue] = []
    @State private var removals: [JSONValue] = []
    @State private var confirm: JSONValue?
    @State private var issue: String?
    @State private var message: String?
    private let owner = "webdav-remote-removals"
    private var connected: Bool {saved["revision"] != nil && saved["revision"] != .null}
    var body: some View {
        Form {
            Section { NavigationLink("主机配置自动上传") { RovaiConfigurationSyncSettings(model: model) }.disabled(dirty || busy) }
            if let issue {Section {Text(issue).foregroundStyle(Theme.warning)}}
            if let message {Section {Text(message).font(.caption).foregroundStyle(Theme.textSecondary)}}
            account
            Section {HStack {Button("测试连接") {Task {await work {_ = try await remote("probe");message = "连接成功"}}};Spacer();Button("刷新副本和传输") {Task {await work {try await list()}}}}.disabled(busy || dirty || !connected)}
            if connected {RovaiWebdavRetention(model: model, revision: saved.text("revision"), disabled: busy || dirty) {Task {await work {try await list()}}}}
            if !removals.isEmpty {Section("移除结果待核对") {ForEach(removals, id: \.self) {frozen in VStack(alignment: .leading, spacing: 8) {
                Text(frozen.text("snapshotId")).font(.caption).foregroundStyle(Theme.textSecondary)
                HStack {Button("核对原结果") {Task {await work {
                    let value = try await request(.backupWebdavRemote, .object(["operation": .string("removal_status"), "expectedRevision": frozen["expectedRevision"] ?? .null, "snapshotId": frozen["snapshotId"] ?? .null, "expectedDigest": frozen["expectedDigest"] ?? .null]))
                    if value["removed"]?.rvBool == true {try keepRemovals(removals.filter {$0.text("snapshotId") != frozen.text("snapshotId")});message = "原移除已完成"} else {message = "原副本仍在或结果未知，原请求继续保留。"}
                }}};Button("重试原移除") {confirm = frozen}}.disabled(busy)
            }}}}
            Section("加密副本") {ForEach(snapshots, id: \.self) {snapshot in VStack(alignment: .leading, spacing: 8) {
                Text(snapshot.text("kind") == "configuration" ? "配置快照" : "完整备份").font(.headline);Text(snapshot.text("createdAt"));Text("\(snapshot["bytes"]?.rvInt ?? 0) 字节").font(.caption).foregroundStyle(Theme.textSecondary)
                Button("移除远端副本", role: .destructive) {Task {await work {
                    let preview = try await remote("removal_preview", ["snapshotId": snapshot["id"] ?? .null, "expectedDigest": snapshot["sha256"] ?? .null])
                    confirm = .object(["operation": .string("remove"), "expectedRevision": saved["revision"] ?? .null, "snapshotId": snapshot["id"] ?? .null, "expectedDigest": snapshot["sha256"] ?? .null, "expectedIndexEtag": preview["indexEtag"] ?? .null, "confirmed": .bool(true)])
                }}}.disabled(busy || dirty)
            }}}
            Section("主机传输") {ForEach(transfers, id: \.self) {transfer in VStack(alignment: .leading, spacing: 8) {
                Text("\(transfer.text("operation") == "push" ? "上传" : "下载") · \(transfer.text("state"))")
                if !transfer.text("message").isEmpty {Text(transfer.text("message")).font(.caption)}
                HStack {Button("核对原任务") {Task {await work {let result = try await remote("status", ["commandId": transfer["commandId"] ?? .null]);transfers = transfers.map {$0.text("commandId") == transfer.text("commandId") ? result : $0}}}};if ["started", "stopping"].contains(transfer.text("state")) {Button("停止传输", role: .destructive) {confirm = .object(["operation": .string("cancel"), "expectedRevision": saved["revision"] ?? .null, "commandId": transfer["commandId"] ?? .null])}}}.disabled(busy)
            }}}
        }.navigationTitle("主机WebDAV").navigationBarTitleDisplayMode(.inline)
        .navigationBarBackButtonHidden(dirty || busy).interactiveDismissDisabled(dirty || busy)
        .toolbar {if dirty || busy {ToolbarItem(placement: .cancellationAction) {Button("返回") {leaving = true}.disabled(busy)}}}
        .task {await work {accept(try await request(.backupWebdavSettings, .object(["operation": .string("get")])));let raw = try model.readBody(owner);removals = raw.isEmpty ? [] : JSONValue.parse(raw)?.rvArray ?? []}}
        .confirmationDialog(title, isPresented: Binding(get: {confirm != nil}, set: {if !$0 && !busy {confirm = nil}}), titleVisibility: .visible) {
            Button("确认", role: confirm?.text("operation") == "set" ? nil : .destructive) {if let frozen = confirm {Task {await work {
                switch frozen.text("operation") {
                case "set": accept(try await request(.backupWebdavSettings, frozen));message = "主机连接已保存"
                case "remove": try keepRemovals(removals.filter {$0.text("snapshotId") != frozen.text("snapshotId")} + [frozen]);_ = try await request(.backupWebdavRemote, frozen);try keepRemovals(removals.filter {$0.text("snapshotId") != frozen.text("snapshotId")});try await list();message = "远端索引中的副本已移除，本地文件保留。"
                default: _ = try await request(.backupWebdavRemote, frozen);transfers = try await remote("transfers")["items"]?.rvArray ?? []
                };confirm = nil
            }}}}.disabled(busy)
            Button("取消", role: .cancel) {confirm = nil}.disabled(busy)
        } message: {Text(confirm?.text("operation") == "set" ? "保存到连接的主机；账号版本已变化时拒绝覆盖。密码只在本页内存中使用。" : confirm?.text("operation") == "remove" ? "按刚才预览的索引版本移除远端副本，保留本地数据；结果未知时核对同一请求。" : "停止不代表远端操作已经撤销，随后核对原任务。")}
        .confirmationDialog("放弃未保存的连接修改？", isPresented: $leaving, titleVisibility: .visible) {Button("放弃修改", role: .destructive) {password = "";dismiss()};Button("继续编辑", role: .cancel) {}} message: {Text("主机原设置保持原样，密码不会保存在本机草稿。")}
    }
    private var account: some View {Section("连接的主机使用此账号保存加密副本") {
        TextField("WebDAV目录地址", text: changed($endpoint)).textInputAutocapitalization(.never).autocorrectionDisabled().keyboardType(.URL)
        TextField("用户名", text: changed($username)).textInputAutocapitalization(.never).autocorrectionDisabled()
        SecureField(saved["passwordSet"]?.rvBool == true ? "密码已保存；留空保留" : "密码", text: changed($password))
        Toggle("清除保存的密码", isOn: changed($clearPassword));Toggle("允许HTTP连接", isOn: changed($insecure))
        Button("保存主机连接") {confirm = .object(["operation": .string("set"), "expectedRevision": saved["revision"] ?? .null, "endpoint": .string(endpoint), "username": .string(username), "password": .string(password), "clearPassword": .bool(clearPassword), "allowInsecure": .bool(insecure)])}.disabled(!dirty || endpoint.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
    }.disabled(busy)}
    private var title: String {switch confirm?.text("operation") {case "set": "保存到连接的主机？";case "remove": "移除此远端副本？";default: "停止原传输？"}}
    private func changed<T>(_ binding: Binding<T>) -> Binding<T> {Binding(get: {binding.wrappedValue}, set: {binding.wrappedValue = $0;dirty = true})}
    private func accept(_ value: JSONValue) {saved = value;endpoint = value.text("endpoint");username = value.text("username");insecure = value["allowInsecure"]?.rvBool == true;password = "";clearPassword = false;dirty = false}
    private func request(_ operation: RovaiOperation, _ value: JSONValue) async throws -> JSONValue {let generation = model.connectionGeneration;let result = try await model.query(operation, params: value);guard generation == model.connectionGeneration, !Task.isCancelled else {throw CancellationError()};return result}
    private func remote(_ action: String, _ fields: [String: JSONValue] = [:]) async throws -> JSONValue {var params = fields;params["operation"] = .string(action);params["expectedRevision"] = saved["revision"] ?? .null;return try await request(.backupWebdavRemote, .object(params))}
    private func list() async throws {snapshots = try await remote("list")["entries"]?.rvArray ?? [];transfers = try await remote("transfers")["items"]?.rvArray ?? []}
    private func keepRemovals(_ values: [JSONValue]) throws {let raw = String(decoding: try JSONEncoder().encode(JSONValue.array(values)), as: UTF8.self);try model.saveBody(owner, body: raw);removals = values}
    private func work(_ action: () async throws -> Void) async {guard !busy else {return};busy = true;issue = nil;message = nil;defer {busy = false};do {try await action()} catch is CancellationError {} catch {issue = error.localizedDescription}}
}
