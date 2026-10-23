import SwiftUI

struct RovaiRuntimeSettingsView: View {
    let model: RovaiWorkspaceModel
    var body: some View {
        List { NavigationLink("模型服务商") {RovaiPortableProviderSettings(model: model)}; NavigationLink("搬入原来的配置") {RovaiPortableConfigurationMigration(model: model)}; NavigationLink("导出完整配置") {RovaiPortableConfigurationExport(model: model)}; NavigationLink("主机日志输出") {RovaiHostLoggingSettings(model: model)}; NavigationLink("任务模板") {RovaiTaskTemplateSettings(model: model)}; NavigationLink("自定义智能体") {RovaiCustomRuntimeSettings(model: model)}; ForEach(model.members.filter { $0.text("presence") != "removed" }.map { RovaiRow(value: $0) }) { row in
            NavigationLink { NativeRuntimeEditor(model: model, member: row.value) } label: { VStack(alignment: .leading, spacing: 6) { Text(row.value.text("displayName")).font(.headline); Text(row.value["runtimeReadiness"]?.text("status") ?? "").font(.caption).foregroundStyle(Theme.textSecondary) } }
        } }.scrollContentBackground(.hidden).navigationTitle("智能体、模型和权限")
    }
}

private struct NativeRuntimeEditor: View {
    let model: RovaiWorkspaceModel
    let member: JSONValue
    @Environment(\.dismiss) private var dismiss
    @State private var installations: [JSONValue] = []
    @State private var kind = ""
    @State private var selection: JSONValue = .object(["mode": .string("runtime_default")])
    @State private var permissions: JSONValue = .object([:])
    @State private var catalog: JSONValue = .object([:])
    @State private var advanced: [String: String] = [:]
    @State private var baseline = ""
    @State private var issue: String?
    @State private var loading = false
    @State private var initialized = false
    @State private var submitted: String?
    @State private var discard = false
    @State private var clearing = false
    private var disabled: Bool { loading || model.busy || model.pending != nil }
    private var signature: String { kind + ":" + selection.prettyString + ":" + permissions.prettyString + ":" + advanced.keys.sorted().map { $0 + ":" + (advanced[$0] ?? "") }.joined(separator: "\n") }
    private var dirty: Bool { initialized && signature != baseline }
    private var eligible: [JSONValue] { installations.filter { $0.text("installationClass") == "managed_default" && $0.text("authScope") == "default" } }
    private var installation: JSONValue? { eligible.first { $0.text("adapterKind") == kind } }
    private var snapshot: JSONValue { installation?["snapshot"] ?? .object([:]) }
    private var models: [JSONValue] { let fresh = catalog["models"]?.rvArray ?? []; return fresh.isEmpty ? snapshot["models"]?.rvArray ?? [] : fresh }
    private var optionDescriptors: [JSONValue] { models.first { $0.text("id") == selection.text("modelId") }?["options"]?.rvArray ?? [] }
    private var permissionDescriptors: [JSONValue] { installation?["permissionOptions"]?.rvArray ?? snapshot["permissionOptions"]?.rvArray ?? [] }
    private var changed: Bool { model.members.first { $0.text("agentId") == member.text("agentId") }?["version"] != member["version"] }
    var body: some View {
        Form {
            Section("模型服务商") {RovaiPortableMemberConnection(model: model, agentId: member.text("agentId"), disabled: disabled || dirty || changed)}
            Section("工作中补充") {RovaiMemberFeedbackSwitch(model: model, agentId: member.text("agentId"))}
            Section("提问与后续工作") {RovaiConversationToolSettings(model: model, agentId: member.text("agentId"))}
            Section("原生插话") {RovaiMemberNativeSteerSwitch(model: model, agentId: member.text("agentId"))}
            Section("自定义智能体程序") {RovaiMemberCustomRuntime(model: model, member: member)}
            if let issue { Section { Text(issue).font(.caption).foregroundStyle(Theme.warning) } }
            if changed { Section { Text("队员信息已变化，当前更改保留；请返回后重新打开。").foregroundStyle(Theme.warning) } }
            Section("智能体") {
                Picker("智能体", selection: Binding(get: { kind }, set: { choose($0) })) { Text("请选择").tag(""); ForEach(eligible, id: \.self) { item in Text(item.text("adapterKind")).tag(item.text("adapterKind")) } }.disabled(disabled)
                if installation != nil { LabeledContent("环境状态", value: snapshot.text("probeStatus").isEmpty ? "尚未检查" : snapshot.text("probeStatus")); LabeledContent("认证状态", value: snapshot.text("authenticationStatus")); NavigationLink("启动程序与环境变量") { RovaiRuntimeStartupView(model: model, runtimeKind: kind) }; Button("检查运行环境") { Task { await model.execute(.runtimeProductCheck, params: .object(["runtimeKind": .string(kind)])) } }.disabled(disabled) }
            }
            if installation?["memberRuntimeDefaults"] != nil && installation?["memberRuntimeDefaults"] != .null {
                Section("模型") {
                    Picker("模型", selection: Binding(get: { selection.text("mode") == "runtime_default" ? "" : selection.text("modelId") }, set: { id in selection = id.isEmpty ? .object(["mode": .string("runtime_default")]) : .object(["mode": .string("explicit"), "modelId": .string(id), "options": .object([:])]) })) {
                        Text("使用智能体默认模型").tag("")
                        ForEach(models.filter { $0["hidden"]?.rvBool != true }, id: \.self) { item in Text(item.text("displayName").isEmpty ? item.text("id") : item.text("displayName")).tag(item.text("id")) }
                        if selection.text("mode") == "explicit" && !models.contains(where: { $0.text("id") == selection.text("modelId") }) { Text("当前模型（未在列表中）：\(selection.text("modelId"))").tag(selection.text("modelId")) }
                    }.disabled(disabled)
                    if kind == "deepseek-harness" && selection.text("mode") == "explicit" { Picker("模型来源", selection: Binding(get: { selection.text("dshSource").isEmpty ? "native" : selection.text("dshSource") }, set: { source in var fields = selection.rvObject; fields["dshSource"] = .string(source); selection = .object(fields) })) { Text("原生").tag("native"); Text("Web").tag("web") }.disabled(disabled) }
                    Button("更新模型列表") { Task { await refreshCatalog() } }.disabled(disabled)
                    ForEach(optionDescriptors, id: \.self) { option in
                        Picker(option.text("label"), selection: Binding(get: { selection["options"]?.text(option.text("key")) ?? "" }, set: { value in var fields = selection.rvObject; var options = selection["options"]?.rvObject ?? [:]; options[option.text("key")] = .string(value); fields["options"] = .object(options); selection = .object(fields) })) { Text("智能体默认").tag(""); ForEach(option["values"]?.rvArray ?? [], id: \.self) { choice in Text(choice.text("label")).tag(choice.text("value")) } }.disabled(disabled)
                    }
                }
                Section("原生权限") {
                    ForEach(permissionDescriptors, id: \.self) { option in NativePermissionField(option: option, values: Binding(get: { permissions["values"] ?? .object([:]) }, set: { value in var fields = permissions.rvObject; fields["values"] = value; permissions = .object(fields) }), advanced: $advanced).disabled(disabled || option["supported"]?.rvBool == false) }
                }
                Section { Button("保存模型和权限", action: save).disabled(disabled || changed || !dirty) }
            } else { Section { Text("请先选择已配置的智能体；模型和权限以Host实际支持的选项为准。").foregroundStyle(Theme.textSecondary) } }
            if member["runtimeConfiguration"] != nil && member["runtimeConfiguration"] != .null { Section { Button("清除运行配置", role: .destructive) { clearing = true }.disabled(disabled || changed) } }
        }.scrollContentBackground(.hidden).navigationTitle(member.text("displayName"))
        .navigationBarBackButtonHidden(dirty).interactiveDismissDisabled(dirty || disabled)
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("返回") { if dirty { discard = true } else { dismiss() } }.disabled(disabled) }; ToolbarItem(placement: .topBarTrailing) { Button("刷新") { Task { await load() } }.disabled(disabled) } }
        .task { await load() }
        .onChange(of: model.lastResult) { _, value in let receipt = value?["commandResult"] ?? value; if receipt?.text("commandId") == submitted { if receipt?.text("status") == "applied" { dismiss() } else { issue = receipt?.text("code") } }; if model.lastOperation == .runtimeProductCheck { Task { await load() } } }
        .confirmationDialog("放弃未保存的运行参数？", isPresented: $discard, titleVisibility: .visible) { Button("放弃", role: .destructive) { dismiss() } }
        .confirmationDialog("清除这位队员的运行配置？", isPresented: $clearing, titleVisibility: .visible) { Button("清除", role: .destructive) { submit(.membersRuntimeClear, command: ["agentId": member["agentId"] ?? .null, "expectedVersion": member["version"] ?? .number(0)]) } }
    }
    private func choose(_ runtime: String) {
        let saved = member["runtimeConfiguration"] ?? .object([:]); let source = saved.text("adapterKind") == runtime ? saved : eligible.first { $0.text("adapterKind") == runtime }?["memberRuntimeDefaults"] ?? .object([:])
        kind = runtime; selection = source["model"] ?? .object(["mode": .string("runtime_default")]); permissions = source["permissions"] ?? .object([:]); advanced = [:]; catalog = .object([:])
    }
    private func load() async { guard !loading else { return }; loading = true; defer { loading = false }; do { installations = try await model.query(.runtimeInstallationsList).rvArray; if !initialized { choose(member["runtimeConfiguration"]?.text("adapterKind") ?? ""); initialized = true; baseline = signature }; issue = nil } catch is CancellationError {} catch { issue = error.localizedDescription } }
    private func refreshCatalog() async {
        loading = true; defer { loading = false }
        do { var params: [String: JSONValue] = ["runtimeKind": .string(kind), "waitForRefresh": .bool(true)]; if kind == "deepseek-harness", selection.text("mode") == "explicit" { params["modelId"] = selection["modelId"]; params["dshSource"] = selection["dshSource"] }; catalog = try await model.query(.runtimeModelCatalogOpen, params: .object(params)); issue = nil } catch is CancellationError {} catch { issue = error.localizedDescription }
    }
    private func submit(_ operation: RovaiOperation, command: [String: JSONValue]) { let id = UUID().uuidString.lowercased(); submitted = id; Task { await model.execute(operation, params: .object(["commandId": .string(id), "command": .object(command)])) } }
    private func save() {
        do { var fields = permissions.rvObject; var values = permissions["values"]?.rvObject ?? [:]; for descriptor in permissionDescriptors where descriptor.text("valueType") == "rule_list" { if let draft = advanced[descriptor.text("key")] { values[descriptor.text("key")] = try JSONDecoder().decode(JSONValue.self, from: Data(draft.utf8)) } }; fields["values"] = .object(values); permissions = .object(fields); submit(.membersRuntimeSet, command: ["agentId": member["agentId"] ?? .null, "expectedVersion": member["version"] ?? .number(0), "adapterKind": .string(kind), "model": selection, "permissions": permissions]) } catch { issue = "原生权限规则格式不正确，请检查后保存。" }
    }
}

private struct NativePermissionField: View {
    let option: JSONValue
    @Binding var values: JSONValue
    @Binding var advanced: [String: String]
    private var key: String { option.text("key") }
    private var value: JSONValue { values[key] ?? option["recommendedValue"] ?? .null }
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(option.text("label")).font(.headline); Text(option.text("description")).font(.caption).foregroundStyle(Theme.textSecondary)
            switch option.text("valueType") {
            case "boolean": Toggle(option.text("label"), isOn: Binding(get: { value.rvBool }, set: { set(.bool($0)) }))
            case "enum": Picker(option.text("label"), selection: Binding(get: { value.stringValue ?? "" }, set: { set(.string($0)) })) { ForEach(option["choices"]?.rvArray ?? [], id: \.self) { choice in Text(choice.text("label")).tag(choice.text("value")) } }
            case "string_list": TextField("每行一项", text: Binding(get: { advanced[key] ?? value.rvArray.compactMap(\.stringValue).joined(separator: "\n") }, set: { text in advanced[key] = text; set(.array(text.components(separatedBy: .newlines).map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }.map(JSONValue.string))) }), axis: .vertical).lineLimit(2...8)
            case "rule_list": TextEditor(text: Binding(get: { advanced[key] ?? value.prettyString }, set: { advanced[key] = $0 })).font(.system(.caption, design: .monospaced)).frame(minHeight: 120)
            default: Text("此选项在当前智能体中不可编辑。").font(.caption).foregroundStyle(Theme.textSecondary)
            }
            if option["supported"]?.rvBool == false { Text(option.text("unsupportedReason")).font(.caption).foregroundStyle(Theme.textSecondary) }
        }
    }
    private func set(_ value: JSONValue) { var fields = values.rvObject; fields[key] = value; values = .object(fields) }
}
