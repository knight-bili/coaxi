import SwiftUI

struct RovaiOfficeCliSettings: View {
    let model: RovaiWorkspaceModel
    @State private var info: JSONValue?, task: JSONValue?, issue: String?
    @State private var busy = false, expanded = false
    @State private var confirm: String?
    private let owner = "office-cli-operation"
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {Text("OfficeCLI").font(.headline);Spacer();Text(info == nil ? "正在检测…" : info?["runnable"]?.rvBool == true ? "可以运行" : info?["installed"]?.rvBool == true ? "已安装，运行异常" : "未安装").font(.caption)}
            HStack {Button("重新检测") {Task {await work {info = try await call("detect")}}};Button(info?["installed"]?.rvBool == true ? "重新安装" : "安装") {confirm = "install"};Button("卸载") {confirm = "uninstall"}.disabled(info?["installed"]?.rvBool != true)}.disabled(busy || task?.text("state") == "running")
            if task?.text("state") == "running" {Button("取消安装") {Task {await work {task = try await call("cancel", extra: ["commandId": task?["commandId"] ?? .null])}}}.disabled(busy)}
            if let version = info?.text("version"), !version.isEmpty {Text(version).font(.caption).foregroundStyle(Theme.textSecondary)}
            if let error = info?.text("runtimeError"), !error.isEmpty {Text(error).font(.caption).foregroundStyle(Theme.warning)}
            if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning)}
            if let task {DisclosureGroup(task.text("state") == "running" ? "正在执行" : task.text("state") == "completed" ? "操作已完成" : task.text("state") == "unknown" ? "结果待核对" : task.text("state") == "not_found" ? "未找到原操作" : "操作未完成", isExpanded: $expanded) {if !task.text("error").isEmpty {Text(task.text("error")).foregroundStyle(Theme.warning)};ScrollView {Text(task.text("log")).font(.system(.caption, design: .monospaced)).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading)}.frame(maxHeight: 260)};if ["unknown", "not_found"].contains(task.text("state")) {Button("核对原操作") {Task {await work {self.task = try await call("status", extra: ["commandId": task["commandId"] ?? .null])}}}.disabled(busy)}}
        }.task(id: model.connectionGeneration) {await work {info = try await call("detect");let previous = try model.readBody(owner);if !previous.isEmpty {let value = try JSONDecoder().decode(JSONValue.self, from: Data(previous.utf8));task = try await call("status", extra: ["commandId": value["commandId"] ?? .null])}}}
        .task(id: "\(task?.text("commandId") ?? ""):\(task?.text("state") ?? "")") {guard let id = task?.text("commandId") else {return};do {while task?.text("state") == "running" {try await Task.sleep(for: .milliseconds(1200));let result = try await call("status", extra: ["commandId": .string(id)]);task = result;if let value = result["info"], value != .null {info = value}}} catch is CancellationError {} catch {issue = error.localizedDescription}}
        .confirmationDialog(confirm == "install" ? "在工作区主机安装OfficeCLI？" : "卸载工作区主机的OfficeCLI？", isPresented: Binding(get: {confirm != nil}, set: {if !$0 {confirm = nil}}), titleVisibility: .visible) {Button("确认执行", role: confirm == "uninstall" ? .destructive : nil) {let operation = confirm;Task {if let operation {await start(operation)}}};Button("取消", role: .cancel) {confirm = nil}} message: {Text(confirm == "install" ? "将在当前工作区所在电脑运行OfficeCLI官方安装脚本，并写入用户程序目录。过程会下载程序，安装日志保留在这里。" : "仅移除主机用户程序目录中的OfficeCLI。技能、任务历史和文档保留。")}
    }
    private func call(_ operation: String, extra: [String: JSONValue] = [:]) async throws -> JSONValue {let generation = model.connectionGeneration;var params = extra;params["operation"] = .string(operation);let result = try await model.query(.officeCli, params: .object(params));guard generation == model.connectionGeneration else {throw CancellationError()};return result}
    private func work(_ action: () async throws -> Void) async {guard !busy else {return};busy = true;defer {busy = false};issue = nil;do {try await action()} catch is CancellationError {} catch {issue = error.localizedDescription}}
    private func start(_ operation: String) async {await work {let id = UUID().uuidString.lowercased();let record = JSONValue.object(["commandId": .string(id)]);let data = try JSONEncoder().encode(record);try model.saveBody(owner, body: String(decoding: data, as: UTF8.self));task = .object(["commandId": .string(id), "state": .string("unknown")]);task = try await call(operation, extra: ["commandId": .string(id), "confirmed": .bool(true), "expectedDigest": info?["digest"] ?? .null]);confirm = nil}}
}
