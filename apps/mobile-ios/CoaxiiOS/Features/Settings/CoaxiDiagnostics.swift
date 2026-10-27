import SwiftUI
import UniformTypeIdentifiers

/// Presents the source DiagnosticsCenter report and exports only Core's centrally redacted v5 bundle.
struct CoaxiDiagnostics: View {
    let model: CoaxiWorkspaceModel
    @State private var report: JSONValue?
    @State private var issue: String?
    @State private var filter = ""
    @State private var destination = ""
    @State private var busy = false
    @State private var repair = false
    @State private var settings = false
    @State private var exporting = false
    @State private var generation: UInt64?
    @State private var exportDocument: CoaxiDiagnosticDocument?
    private var blocked: Bool {busy || model.busy || model.pending != nil || generation != model.connectionGeneration}
    private let groups = [("local_dependencies", "本地依赖"), ("managed_content", "受管内容"), ("agent_runtimes", "智能体")]
    var body: some View {
        List {
            Section {
                Text("读取主机的依赖、受管内容和智能体状态。导出由Core统一脱敏，不包含消息、记忆正文、附件正文或工具输出。").font(.caption).foregroundStyle(Theme.textSecondary)
                if let report {let summary = report["summary"];Text("正常 \(summary?["ok"]?.rvInt ?? 0) · 需要处理 \(summary?["attention"]?.rvInt ?? 0) · 无法确认 \(summary?["unknown"]?.rvInt ?? 0)");Text("读取时间 · \(report.text("checkedAt"))").font(.caption).foregroundStyle(Theme.textSecondary)}
                Picker("筛选", selection: $filter) {Text("全部").tag("");Text("需要处理").tag("attention");Text("无法确认").tag("unknown");Text("正常").tag("ok")}
            }
            if let issue {Section {Text(issue + (report == nil ? "" : "；保留上次读取结果")).foregroundStyle(Theme.warning)}}
            if busy, report == nil {ProgressView("正在读取诊断…")}
            ForEach(groups, id: \.0) {key, title in
                let rows = (report?["checks"]?.rvArray ?? []).filter {$0.text("group") == key && (filter.isEmpty || $0.text("status") == filter)}
                if !rows.isEmpty {Section(title) {ForEach(rows, id: \.self) {check in
                    VStack(alignment: .leading, spacing: 10) {
                        HStack {Text(check.text("label")).font(.headline);Spacer();Text(status(check.text("status"))).font(.caption).foregroundStyle(check.text("status") == "attention" ? Theme.warning : Theme.textSecondary)}
                        Text(check.text("detail"))
                        if check["stale"]?.rvBool == true {Text("这是上次成功记录，当前状态尚未重新确认。").font(.caption).foregroundStyle(Theme.textSecondary)}
                        DisclosureGroup("查看详情") {VStack(alignment: .leading, spacing: 6) {Text(check.text("code"));Text(check.text("observedAt"));ForEach(check["facts"]?.rvArray ?? [], id: \.self) {fact in Text(fact.text("key") + " · " + fact.text("value"))}}.font(.caption).foregroundStyle(Theme.textSecondary)}
                        if check.text("status") != "ok" {actions(check)}
                    }.padding(.vertical, 4)
                }}}
            }
        }.navigationTitle("诊断中心").navigationBarTitleDisplayMode(.inline)
        .toolbar {
            Button {Task {await load()}} label: {Image(systemName: "arrow.clockwise")}.disabled(blocked).accessibilityLabel("重新读取诊断")
            Button {Task {await export()}} label: {Image(systemName: "square.and.arrow.up")}.disabled(blocked || exporting).accessibilityLabel("导出脱敏诊断")
        }
        .task {generation = model.connectionGeneration;await load()}
        .sheet(isPresented: $settings, onDismiss: {Task {await load()}}) {NavigationStack {Group {if destination == "mcp" {CoaxiMcpSettingsView(model: model)} else {CoaxiRuntimeSettingsView(model: model)}}.toolbar {Button("返回诊断") {settings = false}}}}
        .confirmationDialog("修复主机MCP配置文件权限？", isPresented: $repair, titleVisibility: .visible) {
            Button("修复权限") {Task {await repairPermissions()}}.disabled(blocked)
            Button("取消", role: .cancel) {}
        } message: {Text("仅让原Core修复受管配置文件访问权限，不更改服务器定义、队员选择或模型输入。完成后重新读取诊断，不把调用完成当作问题已恢复。")}
        .fileExporter(isPresented: $exporting, document: exportDocument, contentType: .json, defaultFilename: "Coaxi-Coaxi-diagnostics") {result in if case .failure(let error) = result {issue = error.localizedDescription}}
    }
    private func status(_ value: String) -> String {value == "ok" ? "正常" : value == "attention" ? "需要处理" : "暂时无法确认"}
    @ViewBuilder private func actions(_ check: JSONValue) -> some View {
        if check.text("id") == "mcp-config", check.text("code") == "mcp_config_permissions_too_broad" {Button("修复MCP文件权限") {repair = true}.disabled(blocked)}
        else if check.text("id") == "mcp-config" {Button("前往MCP设置") {destination = "mcp";settings = true}}
        else if check.text("subjectKind") == "runtime", check.text("status") == "unknown", !check.text("subjectId").isEmpty {Button("重新检测智能体") {Task {await checkRuntime(check.text("subjectId"))}}.disabled(blocked)}
        else if check.text("subjectKind") == "runtime" {Button("前往智能体设置") {destination = "runtime";settings = true}}
        else if check.text("id") == "legacy-skill-entries" {Text("旧入口需在主机桌面的诊断中心核对归属后清理。").font(.caption).foregroundStyle(Theme.textSecondary)}
        else {Button("导出诊断JSON") {Task {await export()}}.disabled(blocked)}
    }
    private func load() async {
        guard !busy, generation == model.connectionGeneration else {return};busy = true;defer {busy = false}
        do {let value = try await model.query(.diagnosticsCheck);guard generation == model.connectionGeneration else {throw CancellationError()};guard value["schemaVersion"]?.rvInt == 1 else {throw CoaxiHostFailure(code: "诊断格式不受支持")};report = value;issue = nil} catch is CancellationError {} catch {issue = error.localizedDescription}
    }
    private func export() async {
        guard !blocked else {return};busy = true;defer {busy = false}
        do {let value = try await model.query(.diagnosticsExport);guard generation == model.connectionGeneration else {throw CancellationError()};guard value.text("format") == "coaxi-diagnostics-v5" else {throw CoaxiHostFailure(code: "诊断导出格式不受支持")};let encoder = JSONEncoder();encoder.outputFormatting = [.sortedKeys, .prettyPrinted];exportDocument = CoaxiDiagnosticDocument(data: try encoder.encode(value));exporting = true} catch is CancellationError {} catch {issue = error.localizedDescription}
    }
    private func repairPermissions() async {
        guard !blocked else {return};busy = true
        do {_ = try await model.query(.mcpConfigRepairPermissions);guard generation == model.connectionGeneration else {throw CancellationError()};busy = false;await load()} catch {busy = false;issue = error.localizedDescription}
    }
    private func checkRuntime(_ kind: String) async {
        guard !blocked else {return};busy = true
        do {_ = try await model.query(.runtimeProductCheck, params: .object(["runtimeKind": .string(kind)]));guard generation == model.connectionGeneration else {throw CancellationError()};busy = false;await load()} catch {busy = false;issue = error.localizedDescription}
    }
}

private struct CoaxiDiagnosticDocument: FileDocument {
    static var readableContentTypes: [UTType] {[.json]}
    var data: Data
    init(data: Data) {self.data = data}
    init(configuration: ReadConfiguration) throws {data = configuration.file.regularFileContents ?? Data()}
    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {FileWrapper(regularFileWithContents: data)}
}
