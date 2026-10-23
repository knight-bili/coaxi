import SwiftUI
import UniformTypeIdentifiers

private struct EncryptedConfigurationDocument: FileDocument {
    static var readableContentTypes: [UTType] { [.data] }
    var data: Data
    init(data: Data = Data()) { self.data = data }
    init(configuration: ReadConfiguration) throws { data = configuration.file.regularFileContents ?? Data() }
    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper { FileWrapper(regularFileWithContents: data) }
}

struct RovaiPortableConfigurationExport: View {
    let model: RovaiWorkspaceModel
    var sourceArchive: JSONValue? = nil
    @State private var identity = ""
    @State private var passphrase = ""
    @State private var message = ""
    @State private var includeSecrets = false
    @State private var busy = false
    @State private var saving = false
    @State private var pending: JSONValue?
    @State private var receipt: JSONValue?
    @State private var issue: String?
    @State private var document = EncryptedConfigurationDocument()
    private var owner: String { "portable-export/" + identity + (sourceArchive.map { "/original/" + $0.text("archiveId") } ?? "") }
    var body: some View {
        Form {
            Section { Text(sourceArchive == nil ? "保存一份加密副本，到另一台主机再选择要搬入的内容和本机队员。" : "保存这份候选的完整原内容，包括尚不支持的字段；不会启用其中配置。").foregroundStyle(Theme.textSecondary) }
            if receipt?.text("state") != "ready" {
                Section("副本范围") {
                    Toggle(sourceArchive == nil ? "同时包含密钥" : "确认原内容和可能包含的密钥一起加密保存", isOn: $includeSecrets).disabled(pending != nil)
                    SecureField("副本加密口令（至少8个字符）", text: $passphrase).textInputAutocapitalization(.never).autocorrectionDisabled()
                    Text(sourceArchive == nil ? "默认不含密钥。原生登录、服务令牌和本机安装锁不随配置迁移。" : "原副本可能包含原应用保存的敏感设置；明确勾选后才能导出，始终加密。").font(.caption).foregroundStyle(Theme.textSecondary)
                    Button(pending == nil ? "准备加密副本" : "继续准备原副本") { work { try await prepare() } }.disabled(identity.isEmpty || passphrase.unicodeScalars.count < 8 || (sourceArchive != nil && !includeSecrets))
                }
            }
            if pending != nil { Section { Button("核对原导出") { work { try await refresh() } } } }
            if let receipt { Section { Text(receipt.text("state") == "ready" ? "副本已准备好" : receipt.text("state") == "prepared" ? "原副本仍待完成，请使用原口令继续" : "尚未查到原导出结果，未自动重发") } }
            if receipt?.text("state") == "ready" {
                Section {
                    Button("保存加密副本") { work { guard let receipt else { return }; let bytes = try await model.downloadConfigurationExport(receipt); try await current(); document = EncryptedConfigurationDocument(data: bytes); saving = true } }
                    Button("另建一份副本") { work { try model.saveBody(owner, body: ""); pending = nil; receipt = nil; passphrase = ""; includeSecrets = false; message = "" } }
                }
            }
            if !message.isEmpty { Section { Text(message) } }; if let issue { Section { Text(issue).foregroundStyle(.red) } }; if busy { ProgressView() }
        }.disabled(busy).navigationTitle(sourceArchive == nil ? "导出完整配置" : "保留原配置的完整副本")
        .task(id: model.connected) { await start() }.onDisappear { passphrase = "" }
        .fileExporter(isPresented: $saving, document: document, contentType: .data, defaultFilename: "configuration.rovai-config") { result in
            document = EncryptedConfigurationDocument()
            switch result { case .success: message = "加密配置副本已保存"; case .failure(let error): if (error as NSError).code != NSUserCancelledError { issue = "副本未能保存到所选位置，原加密结果仍可再次读取" } }
        }
    }
    private func current() async throws { guard !identity.isEmpty, try await model.presentationIdentity() == identity else { throw CancellationError() } }
    private func work(_ action: @escaping () async throws -> Void) {
        guard !busy else { return }; busy = true; issue = nil
        Task { do { try await current(); try await action() } catch is CancellationError {} catch { issue = error.localizedDescription }; busy = false }
    }
    private func start() async {
        identity = ""; pending = nil; receipt = nil; passphrase = ""; includeSecrets = false; message = ""
        guard model.connected else { return }; busy = true
        do {
            identity = try await model.presentationIdentity(); let raw = try model.readBody(owner)
            if !raw.isEmpty {
                guard let value = JSONValue.parse(raw), case .object(let fields) = value, Set(fields.keys).isSubset(of: Set(["operationId", "includeSecrets", "sourceArchive"])), value["sourceArchive"] == sourceArchive, UUID(uuidString: value.text("operationId")) != nil, case .bool(let included) = value["includeSecrets"] else { throw RovaiHostFailure(code: "原导出恢复记录无效，已保留") }
                pending = value; includeSecrets = included; try await refresh()
            }
        } catch is CancellationError {} catch { issue = error.localizedDescription }; busy = false
    }
    private func refresh() async throws {
        guard let pending else { return }; let value = try await model.privateConfiguration("export-receipt", params: .object(["operationId": pending["operationId"] ?? .null])); try await current()
        guard value["operationId"] == pending["operationId"] else { throw RovaiHostFailure(code: "原导出编号不同") }; receipt = value; if value.text("state") == "ready" { passphrase = "" }
    }
    private func prepare() async throws {
        guard sourceArchive == nil || includeSecrets else { throw RovaiHostFailure(code: "请先确认原文件可能包含的密钥一并加密保存") }
        var fields: [String: JSONValue] = ["operationId": .string(UUID().uuidString.lowercased()), "includeSecrets": .bool(includeSecrets)]
        if let sourceArchive { fields["sourceArchive"] = sourceArchive }
        let original = pending ?? .object(fields)
        if pending == nil { try model.saveBody(owner, body: original.prettyString); pending = original }
        var request: [String: JSONValue] = ["operationId": original["operationId"] ?? .null, "includeSecrets": original["includeSecrets"] ?? .null, "passphrase": .string(passphrase)]
        if let source = original["sourceArchive"] { request["sourceArchive"] = source }
        let value = try await model.privateConfiguration("export", params: .object(request)); try await current()
        guard value["operationId"] == original["operationId"] else { throw RovaiHostFailure(code: "原导出编号不同") }; receipt = value; if value.text("state") == "ready" { passphrase = "" }
    }
}
