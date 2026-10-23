import SwiftUI

private func customRuntimeEnvironmentText(_ value: JSONValue) -> String {
    switch value["installation"]?["environment"]?.text("status") {
    case "needs_environment": return "本机程序或关联文件需要修复。保留旧任务，在本机重新安装此版本，再为之后的任务选择本机安装。"
    case "unreadable": return "暂时无法核对恢复记录，请先检查主机数据目录，原任务和安装记录已保留。"
    case "artifacts_current": return "本机程序文件与安装记录一致，运行资格仍待实际验证。"
    case "not_installed": return "当前主机尚未安装此版本，可选择安装此版本。"
    default: return "当前主机没有提供恢复环境检查结果。"
    }
}

private func customObject(_ value: JSONValue?) throws -> [String: JSONValue] {
    guard case .object(let object) = value else {throw RovaiHostFailure(code: "配置必须是 JSON 对象")};return object
}
private func customKeys(_ value: [String: JSONValue], _ names: Set<String>) throws {
    guard value.keys.allSatisfy({names.contains($0)}) else {throw RovaiHostFailure(code: "配置含有未知字段，请核对原版格式")}
}
func customRuntimeRef(_ value: JSONValue) -> JSONValue {.object(["id": value["id"] ?? .null, "definitionRevision": value["revision"] ?? .null, "definitionDigest": value["definitionDigest"] ?? .null])}
private func customPackage(_ value: JSONValue?, _ kind: String) throws -> JSONValue {
    let raw = try customObject(value)
    try customKeys(raw, Set(["package", "args", "env", "cmd"] + (kind == "npx" ? ["nodeRequired", "node_required"] : ["uvRequired", "uv_required", "python"])))
    guard !(raw["nodeRequired"] != nil && raw["node_required"] != nil), !(raw["uvRequired"] != nil && raw["uv_required"] != nil) else {throw RovaiHostFailure(code: "版本要求不能重复填写")}
    var item: [String: JSONValue] = ["package": raw["package"] ?? .null, "args": raw["args"] ?? .array([]), "env": raw["env"] ?? .object([:]), "cmd": raw["cmd"] ?? .null]
    if kind == "npx" {item["nodeRequired"] = raw["nodeRequired"] ?? raw["node_required"] ?? .null}
    else {item["uvRequired"] = raw["uvRequired"] ?? raw["uv_required"] ?? .null;item["python"] = raw["python"] ?? .null}
    return .object(item)
}
/// 原版 distribution 或注册表整条记录；保留全部渠道，启动只选指定渠道。
func customRuntimeSpec(_ text: String, _ kind: String) throws -> JSONValue {
    guard text.utf8.count <= 2 * 1024 * 1024, let parsed = JSONValue.parse(text) else {throw RovaiHostFailure(code: "配置 JSON 无效或超过2MB")}
    let raw = try customObject(parsed)
    if raw["distribution"] != nil {try customKeys(raw, ["id", "name", "description", "version", "website", "repository", "license", "icon", "distribution"])}
    let channels = try customObject(raw["distribution"] ?? parsed);try customKeys(channels, ["npx", "uvx", "binary"])
    guard ["npx", "uvx", "binary"].contains(kind) else {throw RovaiHostFailure(code: "请选择分发方式")}
    let selected = try customObject(channels[kind]);guard kind != "binary" || !selected.isEmpty else {throw RovaiHostFailure(code: "缺少平台下载配置")}
    var binary: [String: JSONValue] = [:]
    if let binaries = channels["binary"] {
        let entries = try customObject(binaries)
        for (platform, value) in entries {
            let item = try customObject(value);try customKeys(item, ["archive", "url", "cmd", "args", "env", "sha256"])
            guard !(item["archive"] != nil && item["url"] != nil) else {throw RovaiHostFailure(code: "下载地址不能重复填写")}
            binary[platform] = .object(["archive": item["archive"] ?? item["url"] ?? .null, "cmd": item["cmd"] ?? .string(""), "args": item["args"] ?? .array([]), "env": item["env"] ?? .object([:]), "sha256": item["sha256"] ?? .null])
        }
    }
    return .object(["npx": channels["npx"] != nil && channels["npx"] != .null ? try customPackage(channels["npx"], "npx") : .null, "uvx": channels["uvx"] != nil && channels["uvx"] != .null ? try customPackage(channels["uvx"], "uvx") : .null, "binary": .object(binary)])
}
private func customBlank() -> JSONValue {.object([
    "schemaVersion": .number(1), "id": .string(UUID().uuidString.lowercased()), "registryId": .string(""), "name": .string(""), "description": .string(""), "version": .string("1.0.0"), "distributionKind": .string("npx"),
    "spec": .object(["npx": .object(["package": .string(""), "args": .array([.string("--acp")]), "env": .object([:]), "cmd": .null, "nodeRequired": .null]), "uvx": .null, "binary": .object([:])]),
    "iconUrl": .null, "skillsSharedStore": .bool(false), "skillsDir": .null, "source": .string("manual"), "sourceRef": .null, "versionProbe": .null, "supportsMcp": .bool(true), "enabled": .bool(true)
])}
private func customPreferredDistribution(_ entry: JSONValue, _ platform: String) throws -> String {
    let platforms: [String: [String]] = ["windows-x64": ["windows-x86_64", "x86_64-pc-windows-msvc"], "windows-arm64": ["windows-aarch64", "aarch64-pc-windows-msvc"], "macos-x64": ["darwin-x86_64", "x86_64-apple-darwin"], "macos-arm64": ["darwin-aarch64", "aarch64-apple-darwin"], "linux-x64": ["linux-x86_64", "x86_64-unknown-linux-gnu"], "linux-arm64": ["linux-aarch64", "aarch64-unknown-linux-gnu"]]
    let source = try customObject(entry["distribution"]), binary = try customObject(source["binary"] ?? .object([:]))
    if (platforms[platform] ?? []).contains(where: {binary[$0] != nil && binary[$0] != .null}) {return "binary"}
    if source["npx"] != nil && source["npx"] != .null {return "npx"};if source["uvx"] != nil && source["uvx"] != .null {return "uvx"}
    guard !binary.isEmpty else {throw RovaiHostFailure(code: "这条记录没有可用的分发方式")};return "binary"
}
private func customMasked(_ value: JSONValue) throws -> JSONValue {
    var result = try customObject(value);var spec = try customObject(result["spec"])
    func masked(_ value: JSONValue) throws -> JSONValue {
        if value == .null {return .null};var item = try customObject(value);var env = try customObject(item["env"])
        for (key, entry) in env {if case .string = entry {env[key] = .string("<本次新输入，保存后仅保留秘密引用>")}}
        item["env"] = .object(env);return .object(item)
    }
    for (key, entry) in spec {
        if key == "binary" {var entries = try customObject(entry);for (platform, item) in entries {entries[platform] = try masked(item)};spec[key] = .object(entries)}
        else {spec[key] = try masked(entry)}
    }
    result["spec"] = .object(spec);return .object(result)
}

/// 原 AgentsSettingsView 的原生列表与设置详情，连接端始终使用 Core 为真源。
@MainActor struct RovaiCustomRuntimeSettings: View {
    let model: RovaiWorkspaceModel
    @State private var rows: [JSONValue] = []
    @State private var next: String?
    @State private var search = ""
    @State private var busy = false
    @State private var issue: String?
    @State private var original: JSONValue?
    @State private var editing: JSONValue?
    @State private var removing: JSONValue?
    @State private var inspection: JSONValue?
    @State private var receipt: JSONValue?
    @State private var registry = false
    @State private var generation: UInt64?
    private let owner = "custom-runtime-commands"
    private var disabled: Bool {busy || model.busy || model.pending != nil || generation != model.connectionGeneration}
    private var filtered: [JSONValue] {rows.filter {search.isEmpty || [$0.text("name"), $0.text("registryId"), $0.text("description")].contains {$0.localizedCaseInsensitiveContains(search)}}}
    var body: some View {
        List {
            Section {
                Text("沿用原版分发配置。保存后再明确安装；已有任务保留原版本。").font(.caption).foregroundStyle(Theme.textSecondary)
                Button("从注册表添加") {registry = true}.disabled(disabled || original != nil)
                Button("手动添加") {editing = customBlank()}.disabled(disabled || original != nil)
                TextField("搜索名称或标识", text: $search)
            }
            if let original {
                Section("原请求待核对") {Text(original.text("commandId")).font(.caption).textSelection(.enabled);Text("只保存编号和程序归属，环境明文仅在当前表单中。").font(.caption);Button("读取原请求回执") {work {try await reconcile()}}.disabled(disabled)}
            }
            if let issue {Section {Text(issue).foregroundStyle(Theme.warning)}}
            ForEach(filtered, id: \.self) {definition in
                Section {
                    Text(definition.text("name")).font(.headline)
                    Text(definition.text("registryId") + " · " + definition.text("version") + " · r" + String(definition["revision"]?.rvInt ?? 0)).font(.caption).foregroundStyle(Theme.textSecondary)
                    Text(definition.text("description"))
                    Text(definition["enabled"]?.rvBool == false ? "已停用" : "尚未实测运行").font(.caption)
                    Button("状态") {work {inspection = try await call(.runtimeCustomInspect, ["reference": customRuntimeRef(definition)])}}.disabled(disabled)
                    Button("编辑") {editing = definition}.disabled(disabled || original != nil)
                    Button("安装此版本") {work {let info = try await call(.runtimeCustomInspect, ["reference": customRuntimeRef(definition)]);try await write(.runtimeCustomInstall, id: definition.text("id"), payload: ["reference": customRuntimeRef(definition), "platform": info["platform"] ?? .null])}}.disabled(disabled || original != nil || definition["enabled"]?.rvBool != true)
                    Button("移除", role: .destructive) {removing = definition}.disabled(disabled || original != nil)
                }
            }
            if !busy && rows.isEmpty {Section {Text("还没有自定义智能体。添加已有 ACP 程序后，可在队员设置中选择。")}}
            if next != nil {Button("更多程序") {work {try await load(more: true)}}.disabled(disabled)}
            if let inspection {Section("安装与启动来源") {Text(customRuntimeEnvironmentText(inspection));DisclosureGroup("安装与启动来源") {Text(inspection.prettyString).font(.caption.monospaced()).textSelection(.enabled)};Button("关闭状态") {self.inspection = nil}}}
            if let receipt {Section {DisclosureGroup("最近原请求回执") {Text(receipt.prettyString).font(.caption.monospaced()).textSelection(.enabled)}}}
        }
        .scrollContentBackground(.hidden).navigationTitle("自定义智能体")
        .toolbar {Button("刷新列表") {work {try await load()}}.disabled(disabled)}
        .task {generation = model.connectionGeneration;busy = true;defer {busy = false};do {let raw = try model.readBody(owner);if !raw.isEmpty {guard let pending = JSONValue.parse(raw) else {throw RovaiHostFailure(code: "原请求记录无法读取")};original = pending};try await load()} catch {issue = error.localizedDescription}}
        .sheet(isPresented: $registry) {NavigationStack {RovaiCustomRuntimeRegistry(model: model) {definition in registry = false;editing = definition}}}
        .sheet(isPresented: Binding(get: {editing != nil}, set: {if !$0 {editing = nil}})) {
            if let value = editing {NavigationStack {RovaiCustomRuntimeEditor(value: value, disabled: disabled, pending: original != nil, issue: issue, reconcile: {work {try await reconcile()}}, save: {definition in work {try await write(.runtimeCustomUpsert, id: definition.text("id"), payload: ["expectedRevision": value["revision"] ?? .number(0), "definition": definition])}})}}
        }
        .alert("移除这个程序？", isPresented: Binding(get: {removing != nil}, set: {if !$0 {removing = nil}})) {
            Button("取消", role: .cancel) {removing = nil}
            Button("移除", role: .destructive) {if let removing {work {try await write(.runtimeCustomRemove, id: removing.text("id"), payload: ["id": removing["id"] ?? .null, "expectedRevision": removing["revision"] ?? .number(0)])}}}.disabled(disabled || original != nil)
        } message: {Text("之后的任务不能再选择此定义，原任务仍保留原安装、记录和秘密引用。")}
    }
    private func call(_ operation: RovaiOperation, _ input: [String: JSONValue] = [:]) async throws -> JSONValue {
        guard generation == model.connectionGeneration else {throw CancellationError()};let value = try await model.query(operation, params: .object(input));guard generation == model.connectionGeneration else {throw CancellationError()};return value
    }
    private func load(more: Bool = false) async throws {
        let page = try await call(.runtimeCustomList, ["after": more ? next.map(JSONValue.string) ?? .null : .null, "limit": .number(100)])
        let incoming = page["items"]?.rvArray ?? [];rows = more ? rows + incoming.filter {item in !rows.contains {$0.text("id") == item.text("id")}} : incoming
        next = page["next"]?.stringValue
    }
    private func work(_ block: @escaping () async throws -> Void) {
        guard !disabled else {return};busy = true;Task {defer {busy = false};do {try await block();issue = nil} catch is CancellationError {} catch {if generation == model.connectionGeneration {issue = error.localizedDescription}}}
    }
    private func accept(_ pending: JSONValue, _ result: JSONValue) async throws {
        receipt = result
        if pending.text("operation") == RovaiOperation.runtimeCustomInstall.rawValue {
            if ["running", "unknown"].contains(result.text("state")) {return}
            guard ["completed", "failed"].contains(result.text("state")) else {throw RovaiHostFailure(code: "安装结果尚未确定")}
        } else {guard ["applied", "rejected"].contains(result.text("status")) else {throw RovaiHostFailure(code: "保存结果尚未确定")}}
        guard let generation else {throw CancellationError()};_ = try model.clearBodyIfMatches(owner, expectedBody: pending.prettyString, expectedGeneration: generation);original = nil
        if result.text("state") == "failed" {throw RovaiHostFailure(code: result.text("error"))}
        if result.text("status") == "rejected" {throw RovaiHostFailure(code: result.text("code"))}
        if pending.text("operation") == RovaiOperation.runtimeCustomUpsert.rawValue {editing = nil}
        removing = nil;try await load();await model.refresh()
    }
    private func write(_ operation: RovaiOperation, id: String, payload: [String: JSONValue]) async throws {
        guard original == nil else {throw RovaiHostFailure(code: "请先核对原请求")}
        let commandId = UUID().uuidString.lowercased();let pending = JSONValue.object(["operation": .string(operation.rawValue), "commandId": .string(commandId), "definitionId": .string(id)])
        try model.saveBody(owner, body: pending.prettyString);original = pending;var input = payload;input["commandId"] = .string(commandId)
        let result = try await call(operation, input);try await accept(pending, result)
    }
    private func reconcile() async throws {
        guard let pending = original else {return}
        let result = try await call(.commandsReconcile, ["operation": pending["operation"] ?? .null, "params": .object(["commandId": pending["commandId"] ?? .null])])
        guard result.text("state") == "recorded", let value = result["result"], value != .null else {throw RovaiHostFailure(code: "原请求尚无确定回执，未重复发送")}
        let id = pending.text("operation") == RovaiOperation.runtimeCustomInstall.rawValue ? value["reference"]?.text("id") ?? "" : value["payload"]?.text("id") ?? ""
        guard id.isEmpty || id == pending.text("definitionId") else {throw RovaiHostFailure(code: "原回执归属不同，请保留编号核对")};try await accept(pending, value)
    }
}

/// 原版添加程序的字段分组；新环境值仅存当前视图内存，预览始终掩码。
@MainActor private struct RovaiCustomRuntimeEditor: View {
    let value: JSONValue, disabled: Bool, pending: Bool, issue: String?
    let reconcile: () -> Void, save: (JSONValue) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var registryId = ""
    @State private var description = ""
    @State private var version = ""
    @State private var kind = "npx"
    @State private var distribution = ""
    @State private var icon = ""
    @State private var skillsDir = ""
    @State private var versionProbe = ""
    @State private var shared = false
    @State private var mcp = true
    @State private var enabled = true
    @State private var initialized = false
    @State private var discard = false
    @State private var preview: JSONValue?
    @State private var error: String?
    private var fields: [String] {[name, registryId, description, version, kind, distribution, icon, skillsDir, versionProbe, String(shared), String(mcp), String(enabled)]}
    var body: some View {
        Form {
            Section("基本信息") {TextField("名称", text: $name);TextField("注册标识", text: $registryId);TextField("描述", text: $description, axis: .vertical);TextField("版本", text: $version)}.disabled(disabled || pending)
            Section("分发配置") {Picker("分发方式", selection: $kind) {ForEach(["npx", "uvx", "binary"], id: \.self) {Text($0).tag($0)}};TextEditor(text: $distribution).font(.body.monospaced()).frame(minHeight: 200).accessibilityLabel("原版 distribution 配置 JSON");Text("环境新值仅保留在当前页面；保存后回显秘密引用。").font(.caption).foregroundStyle(Theme.textSecondary)}.disabled(disabled || pending)
            Section("技能与工具") {TextField("图标 URL 或内嵌图片", text: $icon);Toggle("声明支持 MCP", isOn: $mcp);Toggle("共享技能目录", isOn: $shared);TextField("技能绝对目录（可选）", text: $skillsDir);TextField("版本检查命令（可选）", text: $versionProbe);Toggle("允许新任务使用", isOn: $enabled)}.disabled(disabled || pending)
            if let error {Section {Text(error).foregroundStyle(Theme.warning)}}
            if let issue {Section {Text(issue).foregroundStyle(Theme.warning)}}
            Section {Button("核对完整配置") {do {preview = try customMasked(definition());error = nil} catch {self.error = error.localizedDescription}}.disabled(disabled || pending)}
            if let preview {Section("配置预览") {Text(preview.prettyString).font(.caption.monospaced()).textSelection(.enabled);Button("保存配置") {do {save(try definition())} catch {self.error = error.localizedDescription}}.disabled(disabled || pending)}}
            if pending {Section {Text("原请求待核对，不重复保存。");Button("读取原请求回执", action: reconcile).disabled(disabled)}}
        }
        .navigationTitle((value["revision"]?.rvInt ?? 0) > 0 ? "编辑自定义智能体" : "添加自定义智能体")
        .toolbar {ToolbarItem(placement: .cancellationAction) {Button("取消") {discard = true}.disabled(disabled)}}
        .interactiveDismissDisabled()
        .task {guard !initialized else {return};name = value.text("name");registryId = value.text("registryId");description = value.text("description");version = value.text("version");kind = value.text("distributionKind");distribution = value["spec"]?.prettyString ?? "{}";icon = value.text("iconUrl");skillsDir = value.text("skillsDir");versionProbe = value.text("versionProbe");shared = value["skillsSharedStore"]?.rvBool == true;mcp = value["supportsMcp"]?.rvBool == true;enabled = value["enabled"]?.rvBool == true;initialized = true}
        .onChange(of: fields) {_, _ in preview = nil}
        .confirmationDialog("离开当前表单？", isPresented: $discard) {Button("离开", role: .destructive) {dismiss()};Button("继续编辑", role: .cancel) {}} message: {Text("当前输入未保存；新环境明文不会写入草稿。已发送请求仍保留编号供核对。")}
    }
    private func definition() throws -> JSONValue {
        var result = try customObject(value);for key in ["revision", "definitionDigest", "createdAt", "updatedAt"] {result.removeValue(forKey: key)}
        result["name"] = .string(name);result["registryId"] = .string(registryId);result["description"] = .string(description);result["version"] = .string(version);result["distributionKind"] = .string(kind);result["spec"] = try customRuntimeSpec(distribution, kind)
        result["iconUrl"] = icon.isEmpty ? .null : .string(icon);result["skillsDir"] = skillsDir.isEmpty ? .null : .string(skillsDir);result["versionProbe"] = versionProbe.isEmpty ? .null : .string(versionProbe);result["skillsSharedStore"] = .bool(shared);result["supportsMcp"] = .bool(mcp);result["enabled"] = .bool(enabled);return .object(result)
    }
}

@MainActor private struct RovaiCustomRuntimeRegistry: View {
    let model: RovaiWorkspaceModel, choose: (JSONValue) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var catalog: JSONValue?
    @State private var search = ""
    @State private var limit = 100
    @State private var busy = false
    @State private var issue: String?
    @State private var generation: UInt64?
    private var entries: [JSONValue] {(catalog?["entries"]?.rvArray ?? []).filter {search.isEmpty || [$0.text("id"), $0.text("name"), $0.text("description")].contains {$0.localizedCaseInsensitiveContains(search)}}}
    var body: some View {
        List {
            Section {TextField("搜索注册表", text: $search);Text(catalog?.text("registryUrl") ?? "").font(.caption);Text(catalog?.text("fetchedAt") ?? "").font(.caption);Button("刷新来源") {Task {await load(true)}}.disabled(busy || generation != model.connectionGeneration)}
            if let issue {Section {Text(issue).foregroundStyle(Theme.warning)}}
            ForEach(Array(entries.prefix(limit)), id: \.self) {entry in
                let registered = (catalog?["registeredIds"]?.rvArray ?? []).contains(.string(entry.text("id")))
                Section {Text(entry.text("name")).font(.headline);Text(entry.text("description"));Text(entry.text("id")).font(.caption).foregroundStyle(Theme.textSecondary);Button(registered ? "已添加" : "带入表单") {select(entry)}.disabled(registered || busy || generation != model.connectionGeneration)}
            }
            if entries.count > limit {Button("更多程序") {limit += 100}}
            if catalog != nil && !busy && entries.isEmpty {Text("没有符合搜索条件的程序")}
        }.navigationTitle("从 ACP 注册表添加").toolbar {Button("返回") {dismiss()}}
        .task {generation = model.connectionGeneration;await load(false)}.onChange(of: search) {_, _ in limit = 100}
    }
    private func load(_ refresh: Bool) async {
        busy = true;defer {busy = false};do {let value = try await model.query(.runtimeCustomRegistry, params: .object(["refresh": .bool(refresh)]));guard generation == model.connectionGeneration else {throw CancellationError()};catalog = value;issue = nil} catch is CancellationError {} catch {issue = error.localizedDescription}
    }
    private func select(_ entry: JSONValue) {
        do {
            let kind = try customPreferredDistribution(entry, catalog?.text("platform") ?? "")
            var result = try customObject(customBlank());result["registryId"] = entry["id"];result["name"] = entry["name"];result["description"] = entry["description"] ?? .string("");result["version"] = entry.text("version").isEmpty ? .string("1.0.0") : entry["version"];result["iconUrl"] = entry["icon"] ?? .null
            result["distributionKind"] = .string(kind);result["spec"] = try customRuntimeSpec(entry.prettyString, kind);result["source"] = .string("registry")
            let id = entry.text("id").addingPercentEncoding(withAllowedCharacters: .alphanumerics) ?? "";result["sourceRef"] = .string((catalog?.text("registryUrl") ?? "") + "#" + id);choose(.object(result))
        } catch {issue = error.localizedDescription}
    }
}
