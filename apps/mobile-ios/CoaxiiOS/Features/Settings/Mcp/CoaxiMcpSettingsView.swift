import SwiftUI

struct CoaxiMcpSettingsView: View {
    let model: CoaxiWorkspaceModel
    @State private var config: JSONValue = .object([:])
    @State private var search = ""
    @State private var issue: String?
    @State private var loading = false
    @State private var adding = false
    var body: some View {
        List {
            if let issue { Text(issue).font(.caption).foregroundStyle(Theme.warning) }
            if let issue = config["fileIssue"], issue != .null { Text(issue.text("message")).foregroundStyle(Theme.warning) }
            ForEach((config["servers"]?.rvArray ?? []).filter { search.isEmpty || "\($0.text("name")) \($0.text("endpoint"))".localizedCaseInsensitiveContains(search) }.map { McpNativeRow(value: $0) }) { row in
                NavigationLink { CoaxiMcpEditor(model: model, config: config, id: row.id) } label: { VStack(alignment: .leading, spacing: 5) { Text(row.value.text("name")).font(.headline); Text(row.value.text("endpoint")).font(.caption).foregroundStyle(Theme.textSecondary); Text(row.value["enabled"]?.rvBool == true ? "已启用" : "已停用").font(.caption2) } }
            }
        }.scrollContentBackground(.hidden).navigationTitle("MCP连接").searchable(text: $search, prompt: "搜索MCP")
        .toolbar { Button { adding = true } label: { Image(systemName: "plus") }.disabled(loading || model.pending != nil); Button("刷新") { Task { await load() } }.disabled(loading) }
        .sheet(isPresented: $adding) { NavigationStack { CoaxiMcpEditor(model: model, config: config, id: nil) } }
        .task { await load() }.onChange(of: model.lastResult) { _, _ in if model.lastOperation?.rawValue.hasPrefix("mcp.") == true { Task { await load() } } }
    }
    private func load() async { guard !loading else { return }; loading = true; defer { loading = false }; do { config = try await model.query(.mcpConfigGet); issue = nil } catch is CancellationError {} catch { issue = error.localizedDescription } }
}
private struct McpNativeRow: Identifiable { let value: JSONValue; var id: String { value.text("serverId") } }
private struct NativeMcpAction: Identifiable { let operation: CoaxiOperation; let params: JSONValue; let title: String; let detail: String; let id = UUID() }

private struct CoaxiMcpEditor: View {
    let model: CoaxiWorkspaceModel
    @State var config: JSONValue
    let id: String?
    @Environment(\.dismiss) private var dismiss
    @State private var draft: String?
    @State private var baseline: String?
    @State private var baseDigest = ""
    @State private var issue: String?
    @State private var loading = false
    @State private var action: NativeMcpAction?
    @State private var discard = false
    private var server: JSONValue? { (config["servers"]?.rvArray ?? []).first { $0.text("serverId") == id } }
    private var disabled: Bool { model.pending != nil || model.busy || loading }
    private var dirty: Bool { draft != nil && draft != baseline }
    var body: some View {
        Form {
            if let issue { Section { Text(issue).font(.caption).foregroundStyle(Theme.warning) } }
            if let server {
                Section("连接") {
                    LabeledContent("类型", value: server.text("transport")); LabeledContent("地址", value: server.text("endpoint"))
                    Toggle("启用连接", isOn: Binding(get: { server["enabled"]?.rvBool == true }, set: { enabled in change(.mcpServersSetEnabled, extra: ["enabled": .bool(enabled)], title: "启用高风险MCP", confirmRisk: enabled && server.text("riskLevel") == "high") })).disabled(disabled)
                }
                Section("队员选择") {
                    ForEach(model.members.filter { $0.text("presence") != "removed" }.map { CoaxiRow(value: $0) }) { row in
                        Toggle(row.value.text("displayName"), isOn: Binding(get: { selected.contains(row.id) }, set: { checked in let next = checked ? selected.union([row.id]) : selected.subtracting([row.id]); change(.mcpServersSetMembers, extra: ["agentIds": .array(next.sorted().map(JSONValue.string))], title: "授权高风险MCP", confirmRisk: checked && server.text("riskLevel") == "high") })).disabled(disabled)
                    }
                }
            }
            Section("配置") {
                if let draft {
                    TextEditor(text: Binding(get: { self.draft ?? "" }, set: { self.draft = $0 })).font(.system(.caption, design: .monospaced)).frame(minHeight: 240).disabled(disabled).privacySensitive()
                    Button("保存") { save(draft) }.disabled(disabled || !dirty)
                    if id != nil { Button("放弃编辑") { self.draft = nil; baseline = nil } }
                } else {
                    Text(server?.text("definitionJson") ?? "").font(.system(.caption, design: .monospaced)).textSelection(.enabled)
                    Button("显示敏感配置并编辑") { Task { await reveal() } }.disabled(disabled)
                }
            }
            if id != nil { Section { Button("删除连接", role: .destructive) { action = NativeMcpAction(operation: .mcpServersDelete, params: params([:]), title: "删除MCP", detail: "删除连接及队员授权，已产生的会话历史仍保留。") }.disabled(disabled) } }
        }.scrollContentBackground(.hidden).navigationTitle(server?.text("name") ?? "添加MCP")
        .navigationBarBackButtonHidden(dirty).interactiveDismissDisabled(dirty || disabled)
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("关闭") { if dirty { discard = true } else { dismiss() } } }; ToolbarItem(placement: .topBarTrailing) { Button("刷新") { Task { await reload() } }.disabled(disabled) } }
        .task { baseDigest = config.text("configDigest"); if id == nil { draft = "{\n  \"mcpServers\": {\n    \"my-server\": {\n      \"command\": \"npx\",\n      \"args\": [\"-y\", \"@example/mcp-server\"]\n    }\n  }\n}"; baseline = draft } }
        .onChange(of: model.lastResult) { _, value in
            guard model.lastOperation?.rawValue.hasPrefix("mcp.") == true, let value else { return }
            if value.text("status") == "ok" { config = value["config"] ?? config; baseDigest = config.text("configDigest"); if let operation = model.lastOperation, [CoaxiOperation.mcpServersCreate, .mcpServersUpdate].contains(operation) { baseline = draft }; if model.lastOperation == .mcpServersDelete || model.lastOperation == .mcpServersCreate { dismiss() } }
            else { issue = value.text("status") == "conflict" ? "配置已在其他位置修改。草稿已保留，请重新读取后编辑。" : (value["issues"]?.rvArray.first?.text("message") ?? "配置尚未保存，请刷新后核对。") }
        }
        .confirmationDialog(action?.title ?? "确认操作", isPresented: Binding(get: { action != nil }, set: { if !$0 { action = nil } }), titleVisibility: .visible) {
            if let action { Button("确认") { Task { await model.execute(action.operation, params: action.params) }; self.action = nil } }
        } message: { Text(action?.detail ?? "") }
        .confirmationDialog("放弃未保存的配置？", isPresented: $discard, titleVisibility: .visible) { Button("放弃", role: .destructive) { dismiss() } }
    }
    private var selected: Set<String> { Set((server?["assignedAgentIds"]?.rvArray ?? []).compactMap(\.stringValue)) }
    private func params(_ extra: [String: JSONValue]) -> JSONValue { var value = extra; value["expectedConfigDigest"] = config["configDigest"] ?? .null; if let id { value["serverId"] = .string(id) }; return .object(value) }
    private func change(_ operation: CoaxiOperation, extra: [String: JSONValue], title: String, confirmRisk: Bool) {
        var values = extra; values["acknowledgeHighRisk"] = .bool(confirmRisk)
        if confirmRisk { action = NativeMcpAction(operation: operation, params: params(values), title: title, detail: "该连接可以执行外部命令或访问外部服务。确认后按当前选择启用。") } else { Task { await model.execute(operation, params: params(values)) } }
    }
    private func reload() async { loading = true; defer { loading = false }; do { config = try await model.query(.mcpConfigGet); issue = nil } catch is CancellationError {} catch { issue = error.localizedDescription } }
    private func reveal() async {
        loading = true; defer { loading = false }
        do { let result = try await model.query(.mcpServersReveal, params: params([:])); guard result.text("status") == "ok" else { issue = "配置已变化，请刷新后重新查看。"; return }; draft = result.text("definitionJson"); baseline = draft; baseDigest = result.text("configDigest"); issue = nil } catch is CancellationError {} catch { issue = error.localizedDescription }
    }
    private func save(_ text: String) {
        do { _ = try JSONSerialization.jsonObject(with: Data(text.utf8)); var value: [String: JSONValue] = ["expectedConfigDigest": .string(baseDigest), "definitionJson": .string(text)]; if let id { value["serverId"] = .string(id) }; Task { await model.execute(id == nil ? .mcpServersCreate : .mcpServersUpdate, params: .object(value)) } } catch { issue = "JSON格式不正确，请检查括号、引号和逗号。" }
    }
}
