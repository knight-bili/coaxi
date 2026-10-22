import SwiftUI
import Observation
import UniformTypeIdentifiers
import CryptoKit

private let migrationDomainNames = ["modelProviders": "模型服务商", "agentSettings": "智能体设置", "customAgents": "自定义程序", "quickMessages": "快捷语", "taskTemplates": "任务模板", "preferences": "可迁移偏好"]
private func migrationField(_ value: JSONValue, _ key: String, _ next: JSONValue) -> JSONValue { guard case .object(var fields) = value else { return value }; fields[key] = next; return .object(fields) }

@MainActor @Observable
private final class PortableMigrationController {
    let model: RovaiWorkspaceModel
    var identity = "", busy = false, issue: String?, active = true
    var source: Data?, sourceName = "", sourceDigest = "", passphrase = ""
    var archives: [JSONValue] = [], archiveNext: JSONValue = .null, archive: JSONValue?
    var rows: [JSONValue] = [], visible = 50, choices: [String: JSONValue] = [:]
    var targets: [String: [JSONValue]] = [:], targetNext: [String: JSONValue] = [:], mappings: [String: JSONValue] = [:]
    var preview: JSONValue?, undo = false, pending: JSONValue?, last: JSONValue?, receipt: JSONValue?
    init(_ model: RovaiWorkspaceModel) { self.model = model }
    private var owner: String { "portable-migration/" + identity }
    var selected: [JSONValue] { choices.keys.sorted().compactMap { choices[$0] }.filter { $0["decision"]?.text("action") != "skip" } }
    var canPreview: Bool { !busy && pending == nil && archive != nil && !selected.isEmpty && !selected.contains { $0["decision"]?.text("action") == "replace" && $0["decision"]?["targetId"] == .null } && !mappings.values.contains { $0.text("providerEntryId").isEmpty } }
    func finishFiles() async throws { guard let original = last else { return }; _ = try await model.privateConfiguration("files-finish", params: .object(["commandId": original["commandId"] ?? .null])); try await current(); try await readReceipt(original) }
    func stop() { active = false; source = nil; passphrase = "" }
    private func current() async throws { guard active, !identity.isEmpty, try await model.presentationIdentity() == identity else { throw CancellationError() } }
    func work(_ action: () async throws -> Void) async {
        guard !busy else { return }; busy = true; issue = nil
        do { try await current(); try await action() } catch is CancellationError {} catch { issue = error.localizedDescription }; busy = false
    }
    private func decoded(_ raw: String) throws -> JSONValue? {
        if raw.isEmpty { return nil }
        guard let value = JSONValue.parse(raw), case .object(let fields) = value,
              Set(fields.keys).isSubset(of: Set(["operation", "commandId", "sourceDigest", "archiveId", "previewId", "previewDigest"])),
              ["configuration.archive.register", "configuration.migration.confirm", "configuration.migration.undoConfirm"].contains(value.text("operation")), UUID(uuidString: value.text("commandId")) != nil,
              fields.values.allSatisfy({ if case .string(let text) = $0 { return text.utf8.count <= 256 }; return false }) else { throw RovaiHostFailure(code: "原迁移请求记录无效，已保留") }
        return value
    }
    func start() async {
        active = true; identity = ""; source = nil; passphrase = ""; archive = nil; rows = []; choices = [:]; mappings = [:]; preview = nil; receipt = nil; pending = nil; last = nil
        guard model.connected else { return }
        busy = true; issue = nil
        do { identity = try await model.presentationIdentity(); pending = try decoded(model.readBody(owner)); last = try decoded(model.readBody(owner + "/last")); try await loadArchives(); if let original = pending ?? last, original.text("operation") != "configuration.archive.register" { try await readReceipt(original) } }
        catch is CancellationError {} catch { issue = error.localizedDescription }; busy = false
    }
    private func remember(_ value: JSONValue?) async throws { try await current(); try model.saveBody(owner, body: value?.prettyString ?? ""); pending = value }
    func loadArchives(more: Bool = false) async throws {
        let page = try await model.query(.configurationMigrationArchives, params: .object(["after": more ? archiveNext : .null, "limit": .number(100)])); try await current()
        archives = more ? archives + (page["items"]?.rvArray ?? []) : page["items"]?.rvArray ?? []; archiveNext = page["next"] ?? .null
    }
    func chooseFile(_ url: URL) async throws {
        let access = url.startAccessingSecurityScopedResource(); defer { if access { url.stopAccessingSecurityScopedResource() } }
        let result = try await Task.detached(priority: .userInitiated) { () -> (Data, String) in
            let handle = try FileHandle(forReadingFrom: url); defer { try? handle.close() }
            var bytes = Data(), hash = SHA256()
            while let chunk = try handle.read(upToCount: 64 * 1024), !chunk.isEmpty { guard bytes.count + chunk.count <= 64 * 1024 * 1024 else { throw RovaiHostFailure(code: "配置文件超过64MB") }; bytes.append(chunk); hash.update(data: chunk) }
            return (bytes, "sha256:" + hash.finalize().map { String(format: "%02x", $0) }.joined())
        }.value
        try await current(); source = result.0; sourceDigest = result.1; sourceName = url.lastPathComponent
    }
    func open(_ value: JSONValue) async throws {
        let result = try await model.query(.configurationMigrationPreview, params: .object(["schemaVersion": .number(1), "archiveId": value["archiveId"] ?? .null, "sourceDigest": value["sourceDigest"] ?? .null, "selections": .array([]), "members": .array([])])); try await current()
        archive = value; rows = result["rows"]?.rvArray ?? []; choices = [:]; mappings = [:]; preview = nil; undo = false; targets = [:]; targetNext = [:]; visible = 50
    }
    private func registered(_ value: JSONValue, original: JSONValue) async throws {
        guard value["archiveId"] == original["commandId"], value["sourceDigest"] == original["sourceDigest"] else { throw RovaiHostFailure(code: "候选文件与原请求不同") }
        try await remember(nil); source = nil; passphrase = ""; try await loadArchives(); try await open(value)
    }
    func registerSource() async throws {
        guard let source else { throw RovaiHostFailure(code: "请先选择文件") }
        let original = pending ?? .object(["operation": .string("configuration.archive.register"), "commandId": .string(UUID().uuidString.lowercased()), "sourceDigest": .string(sourceDigest)])
        guard original.text("operation") == "configuration.archive.register", original.text("sourceDigest") == sourceDigest else { throw RovaiHostFailure(code: "继续原请求时须选择同一份文件") }
        if pending == nil { try await remember(original) }
        let value = try await model.configurationArchive(metadata: .object(["operationId": original["commandId"] ?? .null, "sourceDigest": .string(sourceDigest), "byteSize": .number(Double(source.count)), "passphrase": .string(passphrase), "manifest": .null]), data: source)
        try await current(); try await registered(value, original: original)
    }
    func reconcile() async throws {
        guard let pending else { return }
        if pending.text("operation") == "configuration.archive.register" {
            let result = try await model.privateConfiguration("receipt", params: .object(["operationId": pending["commandId"] ?? .null])); try await current()
            guard let archive = result["archive"], archive != .null else { throw RovaiHostFailure(code: "尚未查到原文件登记，请稍后核对") }; try await registered(archive, original: pending)
        } else { try await readReceipt(pending) }
    }
    func decision(_ row: JSONValue, action: String, target: JSONValue? = nil) {
        let id = row.text("entryId")
        choices[id] = .object(["entryId": .string(id), "domain": row["domain"] ?? .null, "decision": .object(["entryId": .string(id), "action": .string(action), "targetId": action == "replace" ? target?["id"] ?? .null : .null, "expectedRevision": action == "replace" ? target?["revision"] ?? .null : .null])]); preview = nil; undo = false
    }
    func loadTargets(_ domain: String, more: Bool = false) async throws {
        let page = try await model.query(.configurationMigrationTargets, params: .object(["domain": .string(domain), "after": more ? targetNext[domain] ?? .null : .null, "limit": .number(100)])); try await current()
        targets[domain] = more ? (targets[domain] ?? []) + (page["items"]?.rvArray ?? []) : page["items"]?.rvArray ?? []; targetNext[domain] = page["next"] ?? .null
    }
    func addMember(_ member: JSONValue) async throws {
        let id = member.text("agentId"), actual = try await model.query(.configurationMemberConnectionGet, params: .object(["schemaVersion": .number(1), "agentId": member["agentId"] ?? .null])); try await current()
        guard let runtime = actual["runtime"], runtime != .null else { throw RovaiHostFailure(code: "请先为这位队员选择可用的程序") }
        let selection = runtime["modelSelection"] ?? .object(["mode": .string("runtime_default")])
        mappings[id] = .object(["agentId": .string(id), "expectedVersion": actual["agentVersion"] ?? .null, "installationId": runtime["installationId"] ?? .null, "expectedInstallationGeneration": runtime["installationGeneration"] ?? .null, "providerEntryId": .string(""), "environmentEntryId": .null, "modelSelection": selection, "originalSelection": selection]); preview = nil
    }
    func changeMember(_ id: String, field: String, value: JSONValue) { guard let original = mappings[id] else { return }; mappings[id] = migrationField(original, field, value); preview = nil }
    func buildPreview() async throws {
        guard let archive, canPreview else { throw RovaiHostFailure(code: "请先完成条目和队员选择") }
        let members = mappings.keys.sorted().compactMap { id -> JSONValue? in guard case .object(var value) = mappings[id] else { return nil }; value.removeValue(forKey: "originalSelection"); return .object(value) }
        let value = try await model.query(.configurationMigrationPreview, params: .object(["schemaVersion": .number(1), "archiveId": archive["archiveId"] ?? .null, "sourceDigest": archive["sourceDigest"] ?? .null, "selections": .array(selected), "members": .array(members)])); try await current(); preview = value; undo = false
    }
    func confirm() async throws {
        guard let preview, preview["canApply"] == .bool(true), pending == nil else { throw RovaiHostFailure(code: "请重新核对预览") }
        let operation = undo ? "configuration.migration.undoConfirm" : "configuration.migration.confirm"
        let command: JSONValue = .object(["operation": .string(operation), "commandId": .string(UUID().uuidString.lowercased()), "previewId": preview["previewId"] ?? .null, "previewDigest": preview["previewDigest"] ?? .null]); try await remember(command)
        let result = try await model.query(undo ? .configurationMigrationUndoConfirm : .configurationMigrationConfirm, params: .object(["schemaVersion": .number(1), "commandId": command["commandId"] ?? .null, "previewId": command["previewId"] ?? .null, "previewDigest": command["previewDigest"] ?? .null])); try await current()
        guard result["commandId"] == command["commandId"] else { throw RovaiHostFailure(code: "原回执编号不同") }; try await readReceipt(command)
    }
    func readReceipt(_ original: JSONValue) async throws {
        let result = try await model.query(.configurationMigrationReceipt, params: .object(["schemaVersion": .number(1), "commandId": original["commandId"] ?? .null])); try await current()
        guard result["commandId"] == original["commandId"], result["previewId"] == original["previewId"], result["previewDigest"] == original["previewDigest"] else { throw RovaiHostFailure(code: "原请求回执不匹配") }
        receipt = result
        if ["applied", "rejected"].contains(result.text("state")) {
            if result.text("state") == "applied" { try model.saveBody(owner + "/last", body: original.prettyString); last = original }
            if pending?["commandId"] == original["commandId"] { try await remember(nil) }
        }
    }
    func undoPreview() async throws {
        guard let last, last.text("operation") == "configuration.migration.confirm" else { throw RovaiHostFailure(code: "请先选择已完成的原导入") }
        let value = try await model.query(.configurationMigrationUndoPreview, params: .object(["schemaVersion": .number(1), "originalCommandId": last["commandId"] ?? .null])); try await current()
        preview = value; rows = value["rows"]?.rvArray ?? []; visible = 50; undo = true
        if archive == nil { archive = .object(["archiveId": value["archiveId"] ?? .null, "sourceDigest": value["sourceDigest"] ?? .null]) }
    }
}

struct RovaiPortableConfigurationMigration: View {
    let model: RovaiWorkspaceModel
    @State private var controller: PortableMigrationController
    @State private var picker = false, confirming = false
    @State private var details = Set<String>()
    init(model: RovaiWorkspaceModel) { self.model = model; _controller = State(initialValue: PortableMigrationController(model)) }
    var body: some View {
        List {
            Section {
                Text("先选文件，再逐项选择。不会自动运行、安装程序或绑定同名队员。").foregroundStyle(Theme.textSecondary)
                if controller.pending != nil { Text("原请求尚待核对，未自动重复发送。"); Button("核对原请求") { Task { await controller.work { try await controller.reconcile() } } } }
                Button(controller.source == nil ? "选择原来的配置文件" : controller.sourceName) { picker = true }.disabled(controller.pending != nil && controller.pending?.text("operation") != "configuration.archive.register")
                SecureField("文件加密口令（未加密可留空）", text: $controller.passphrase).textInputAutocapitalization(.never).autocorrectionDisabled()
                Button(controller.pending?.text("operation") == "configuration.archive.register" ? "继续登记原文件" : "读取为迁移候选") { Task { await controller.work { try await controller.registerSource() } } }.disabled(controller.source == nil || (controller.pending != nil && controller.pending?.text("operation") != "configuration.archive.register"))
            }
            Section("已有迁移候选") {
                ForEach(controller.archives, id: \.self) { value in Button("候选 · \(value.text("sourceKind")) · \(value.text("createdAt"))") { Task { await controller.work { try await controller.open(value) } } }.disabled(controller.pending != nil) }
                if controller.archiveNext != .null { Button("加载更多候选") { Task { await controller.work { try await controller.loadArchives(more: true) } } } }
            }
            if let source = controller.archive {
                Section { NavigationLink("保存原配置完整副本（含未支持字段）") { RovaiPortableConfigurationExport(model: model, sourceArchive: .object(["archiveId": source["archiveId"] ?? .null, "sourceDigest": source["sourceDigest"] ?? .null])) } }.disabled(controller.pending != nil)
            }
            if controller.archive != nil {
                Section(controller.undo ? "撤销预览" : "选择要搬入的内容") {
                    ForEach(Array(controller.rows.prefix(controller.visible)), id: \.self) { row in migrationRow(row) }
                    if controller.visible < controller.rows.count { Button("显示更多条目") { controller.visible += 50 } }
                }
                if !controller.undo {
                    Section("明确分配给本机队员（可选）") {
                        Menu("添加队员映射") { ForEach(model.members.filter { $0.text("presence") == "present" && controller.mappings[$0.text("agentId")] == nil }, id: \.self) { member in Button(member.text("displayName")) { Task { await controller.work { try await controller.addMember(member) } } } } }.disabled(controller.pending != nil)
                        ForEach(controller.mappings.keys.sorted(), id: \.self) { id in if let mapping = controller.mappings[id] { memberRow(id, mapping) } }
                    }
                }
                Section {
                    if !controller.undo { Button("生成完整预览") { Task { await controller.work { try await controller.buildPreview() } } }.disabled(!controller.canPreview) }
                    Button(controller.undo ? "确认撤销" : "确认搬入") { confirming = true }.disabled(controller.preview?["canApply"] != .bool(true) || controller.pending != nil)
                }
            }
            if let receipt = controller.receipt {
                Section("原结果 · " + receipt.text("state")) {
                    Text("历史任务和消息保留。数据库提交与文件保存分别列出。").font(.caption)
                    ForEach(receipt["files"]?.rvArray ?? [], id: \.self) { file in LabeledContent(file.text("targetId"), value: file.text("state")) }
                    if controller.last != nil, receipt.text("state") == "applied", (receipt["files"]?.rvArray ?? []).contains(where: { $0.text("state") == "pending" }) { Button("继续保存原配置文件") { Task { await controller.work { try await controller.finishFiles() } } }.disabled(controller.pending != nil) }
                    if let last = controller.last { Button("刷新文件状态") { Task { await controller.work { try await controller.readReceipt(last) } } }.disabled(controller.pending != nil) }
                    if receipt.text("state") == "applied", controller.last?.text("operation") == "configuration.migration.confirm" { Button("预览撤销这次导入") { Task { await controller.work { try await controller.undoPreview() } } }.disabled(controller.pending != nil || !(receipt["files"]?.rvArray ?? []).allSatisfy { $0.text("state") == "applied" }) }
                }
            }
            if let issue = controller.issue { Section { Text(issue).foregroundStyle(.red) } }; if controller.busy { ProgressView() }
        }
        .disabled(controller.busy).scrollContentBackground(.hidden).navigationTitle("搬入原来的配置")
        .task(id: model.connected) { await controller.start() }.onDisappear { controller.stop() }
        .fileImporter(isPresented: $picker, allowedContentTypes: [.json, .data]) { result in Task { await controller.work { try await controller.chooseFile(result.get()) } } }
        .confirmationDialog(controller.undo ? "撤销这次配置导入？" : "搬入已预览的配置？", isPresented: $confirming, titleVisibility: .visible) {
            Button("确认", role: controller.undo ? .destructive : nil) { Task { await controller.work { try await controller.confirm() } } }; Button("返回预览", role: .cancel) {}
        } message: { Text("只处理当前预览明确选择的条目；有后续修改会拒绝覆盖。历史任务保留，也不会自动开始工作。") }
    }
    @ViewBuilder private func migrationRow(_ row: JSONValue) -> some View {
        let id = row.text("entryId"), domain = row.text("domain"), action = controller.undo ? row.text("action") : controller.choices[id]?["decision"]?.text("action") ?? "skip"
        VStack(alignment: .leading, spacing: 8) {
            Text(row.text("name")).font(.headline); Text(migrationDomainNames[domain] ?? "原值仅保留，尚无应用入口").font(.caption).foregroundStyle(Theme.textSecondary)
            if migrationDomainNames[domain] != nil && !controller.undo {
                Picker("处理方式", selection: Binding(get: { action }, set: { value in controller.decision(row, action: value); if value == "replace" && controller.targets[domain] == nil { Task { await controller.work { try await controller.loadTargets(domain) } } } })) { Text("跳过").tag("skip"); Text("新增").tag("create"); Text("替换").tag("replace") }.pickerStyle(.segmented).disabled(controller.pending != nil)
                if action == "replace" {
                    Picker("准确的本机目标", selection: Binding(get: { controller.choices[id]?["decision"]?.text("targetId") ?? "" }, set: { target in controller.decision(row, action: "replace", target: controller.targets[domain]?.first { $0.text("id") == target }) })) { Text("请选择目标").tag(""); ForEach(controller.targets[domain] ?? [], id: \.self) { Text($0.text("name")).tag($0.text("id")) } }.disabled(controller.pending != nil)
                    if let next = controller.targetNext[domain], next != .null { Button("加载更多目标") { Task { await controller.work { try await controller.loadTargets(domain, more: true) } } } }
                }
            }
            if action != "skip", let checked = (controller.preview?["rows"]?.rvArray ?? []).first(where: { $0["entryId"] == row["entryId"] }) {
                Text(checked.text("status")).font(.caption); if !checked.text("reason").isEmpty { Text(checked.text("reason")).font(.caption).foregroundStyle(.red) }
                DisclosureGroup("原值与将保存的内容", isExpanded: Binding(get: { details.contains(id) }, set: { if $0 { details.insert(id) } else { details.remove(id) } })) {
                    if details.contains(id) { Text("原值\n" + (checked["before"]?.prettyString ?? "")).font(.system(.caption, design: .monospaced)); Text("将保存\n" + (checked["after"]?.prettyString ?? "")).font(.system(.caption, design: .monospaced)) }
                }
            }
        }
    }
    @ViewBuilder private func memberRow(_ id: String, _ mapping: JSONValue) -> some View {
        let providers = controller.rows.filter { $0.text("domain") == "modelProviders" && ["create", "replace"].contains(controller.choices[$0.text("entryId")]?["decision"]?.text("action") ?? "") }
        VStack(alignment: .leading, spacing: 8) {
            Text(model.members.first { $0.text("agentId") == id }?.text("displayName") ?? id).font(.headline)
            Picker("已勾选的服务商来源", selection: Binding(get: { mapping.text("providerEntryId") }, set: { controller.changeMember(id, field: "providerEntryId", value: .string($0)) })) { Text("请选择准确来源").tag(""); ForEach(providers, id: \.self) { Text($0.text("name")).tag($0.text("entryId")) } }
            Picker("环境设置", selection: Binding(get: { mapping.text("environmentEntryId") }, set: { controller.changeMember(id, field: "environmentEntryId", value: $0.isEmpty ? .null : .string($0)) })) { Text("不迁入环境值").tag(""); ForEach(controller.rows.filter { $0.text("domain") == "agentSettings" }, id: \.self) { Text($0.text("name")).tag($0.text("entryId")) } }
            Picker("模型选择", selection: Binding(get: { mapping["modelSelection"] == mapping["originalSelection"] ? "original" : "default" }, set: { controller.changeMember(id, field: "modelSelection", value: $0 == "original" ? mapping["originalSelection"] ?? .null : .object(["mode": .string("runtime_default")])) })) { Text("保留队员模型选择").tag("original"); Text("程序默认模型").tag("default") }
            Button("移除此队员映射", role: .destructive) { controller.mappings.removeValue(forKey: id); controller.preview = nil }
        }.disabled(controller.pending != nil)
    }
}
