import SwiftUI

private struct NativeEnvironmentValue: Identifiable { let id = UUID(); var name: String; var value: String }
struct CoaxiRuntimeStartupView: View {
    let model: CoaxiWorkspaceModel
    let runtimeKind: String
    @Environment(\.dismiss) private var dismiss
    @State private var saved: JSONValue?
    @State private var program = ""
    @State private var environment: [NativeEnvironmentValue] = []
    @State private var conflicts: [JSONValue] = []
    @State private var inspection: JSONValue?
    @State private var issue: String?
    @State private var busy = false
    @State private var discard = false
    private var configuration: JSONValue { .object(["programPath": program.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? .null : .string(program.trimmingCharacters(in: .whitespacesAndNewlines)), "environment": .array(environment.map { .object(["name": .string($0.name.trimmingCharacters(in: .whitespacesAndNewlines)), "value": .string($0.value)]) })]) }
    private var changes: [JSONValue] {
        let before = saved?["configuration"] ?? .object([:]); let after = configuration; var result: [JSONValue] = []
        func add(_ path: [String], _ old: JSONValue, _ new: JSONValue, _ title: String) { if old != new { result.append(.object(["path": .array(path.map(JSONValue.string)), "before": old, "after": new, "label": .string(title)])) } }
        add(["programPath"], before["programPath"] ?? .null, after["programPath"] ?? .null, "程序路径")
        let old = variables(before); let new = variables(after)
        for name in Set(old.keys).union(new.keys).sorted() { add(["environment", name], old[name] ?? .null, new[name] ?? .null, "环境变量 " + name) }; return result
    }
    private var dirty: Bool { saved != nil && !changes.isEmpty }
    var body: some View {
        Form {
            if let issue { Section { Text(issue).font(.caption).foregroundStyle(Theme.warning) } }
            Section { LabeledContent("智能体", value: runtimeKind); TextField("Host可执行程序路径", text: $program).textInputAutocapitalization(.never).autocorrectionDisabled().disabled(busy) } footer: { Text("填写Host上的程序路径；留空使用自动发现。") }
            Section("环境变量") {
                ForEach($environment) { $row in VStack(alignment: .leading, spacing: 8) { TextField("变量名", text: $row.name).textInputAutocapitalization(.never).autocorrectionDisabled(); SecureField("变量值", text: $row.value).textInputAutocapitalization(.never).autocorrectionDisabled(); Button("移除变量", role: .destructive) { environment.removeAll { $0.id == row.id } } }.disabled(busy) }
                Button("添加环境变量") { environment.append(NativeEnvironmentValue(name: "", value: "")) }.disabled(busy || environment.count >= 256)
            }
            if !conflicts.isEmpty { Section("处理外部修改") {
                ForEach(conflicts, id: \.self) { conflict in VStack(alignment: .leading, spacing: 8) { Text(conflict.text("label") + "已在外部更新。").foregroundStyle(Theme.warning); HStack { Button("保留我的") { conflicts.removeAll { $0 == conflict } }; Button("使用最新") { accept(apply(configuration, edits: [.object(["path": conflict["path"] ?? .array([]), "after": conflict["current"] ?? .null])])); conflicts.removeAll { $0 == conflict } } }.disabled(busy) } }
            } }
            Section("程序识别") { Button("识别程序") { Task { await inspect(deep: false) } }.disabled(busy); Button("检查草稿") { Task { await inspect(deep: true) } }.disabled(busy); if let inspection { Text(["missing": "未检测到程序", "recognized": "已识别程序", "version_unverified": "已识别程序，版本检查未完成", "authentication_required": "已识别程序，需要登录", "ready": "草稿检查通过", "check_failed": "草稿检查未通过"][inspection.text("status")] ?? "草稿检查未通过"); Text(inspection.text("executablePath")).font(.caption).textSelection(.enabled) } }
        }.scrollContentBackground(.hidden).navigationTitle("启动程序与环境变量")
        .navigationBarBackButtonHidden(dirty || busy).interactiveDismissDisabled(dirty || busy)
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("返回") { if dirty { discard = true } else { dismiss() } }.disabled(busy) }; ToolbarItem(placement: .confirmationAction) { Button("保存") { Task { await save() } }.disabled(busy || !dirty || !conflicts.isEmpty) } }
        .task { await load() }
        .confirmationDialog("放弃未保存的启动设置？", isPresented: $discard, titleVisibility: .visible) { Button("放弃", role: .destructive) { dismiss() } }
    }
    private func variables(_ configuration: JSONValue) -> [String: JSONValue] { var result: [String: JSONValue] = [:]; for variable in configuration["environment"]?.rvArray ?? [] { result[variable.text("name")] = variable["value"] ?? .null }; return result }
    private func apply(_ configuration: JSONValue, edits: [JSONValue]) -> JSONValue {
        var program = configuration["programPath"] ?? .null; var environment = variables(configuration)
        for edit in edits { let parts = (edit["path"]?.rvArray ?? []).compactMap(\.stringValue); let value = edit["after"] ?? .null; if parts == ["programPath"] { program = value } else if parts.count == 2 && parts[0] == "environment" { if value == .null { environment.removeValue(forKey: parts[1]) } else { environment[parts[1]] = value } } }
        return .object(["programPath": program, "environment": .array(environment.keys.sorted().map { .object(["name": .string($0), "value": environment[$0] ?? .null]) })])
    }
    private func accept(_ configuration: JSONValue) { program = configuration.text("programPath"); environment = (configuration["environment"]?.rvArray ?? []).map { NativeEnvironmentValue(name: $0.text("name"), value: $0.text("value")) } }
    private func load() async { busy = true; defer { busy = false }; do { let value = try await model.query(.runtimeStartupGet, params: .object(["runtimeKind": .string(runtimeKind)])); saved = value; accept(value["configuration"] ?? .object([:])); issue = nil } catch is CancellationError {} catch { issue = error.localizedDescription } }
    private func inspect(deep: Bool) async {
        busy = true; defer { busy = false }
        do { inspection = try await model.query(deep ? .runtimeStartupCheck : .runtimeStartupInspect, params: .object(["runtimeKind": .string(runtimeKind), "configuration": configuration])); issue = nil } catch is CancellationError {} catch { issue = error.localizedDescription }
    }
    private func save() async {
        let names = environment.map { $0.name.trimmingCharacters(in: .whitespacesAndNewlines) }
        guard Set(names).count == names.count, names.allSatisfy({ $0.range(of: "^[A-Za-z_][A-Za-z0-9_]{0,255}$", options: .regularExpression) != nil }) else { issue = "变量名需要唯一，只含字母、数字、下划线，且不能以数字开头。"; return }
        let edits = changes; busy = true; defer { busy = false }
        do { let response = try await model.query(.runtimeStartupSave, params: .object(["runtimeKind": .string(runtimeKind), "edits": .array(edits)])); if response.text("status") == "conflict" { saved = response["latest"]; accept(apply(saved?["configuration"] ?? .object([:]), edits: edits)); conflicts = response["conflicts"]?.rvArray ?? [] } else { saved = response; accept(response["configuration"] ?? .object([:])); conflicts = [] }; issue = nil } catch is CancellationError {} catch { issue = (error as? CoaxiHostFailure)?.unknownResult == false ? error.localizedDescription : "保存结果尚未确认，草稿已保留。请核对Host上的设置后重试。" }
    }
}
