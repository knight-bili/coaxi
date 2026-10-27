import SwiftUI

struct CoaxiForgeAccounts: View {
    let model: CoaxiWorkspaceModel
    let onChanged: () -> Void
    @State private var open = false
    var body: some View {Button("仓库账号") {open = true}.sheet(isPresented: $open, onDismiss: onChanged) {CoaxiForgeAccountSettings(model: model)}}
}
private struct CoaxiForgeAccountSettings: View {
    let model: CoaxiWorkspaceModel
    @Environment(\.dismiss) private var dismiss
    @State private var accounts: [JSONValue] = []
    @State private var selected: JSONValue?
    @State private var removing: JSONValue?
    @State private var creating = false
    @State private var busy = false
    @State private var id = ""
    @State private var provider = "github"
    @State private var base = "https://github.com"
    @State private var token = ""
    @State private var issue: String?
    var body: some View {
        NavigationStack {Form {
            if let issue {Text(issue).foregroundStyle(Theme.warning)}
            if creating || selected != nil {Section {Picker("平台", selection: $provider) {ForEach(["github", "gitlab", "gitea", "forgejo"], id: \.self) {Text($0).tag($0)}}.disabled(!creating);TextField("网站地址", text: $base).textInputAutocapitalization(.never).autocorrectionDisabled().keyboardType(.URL);SecureField("访问令牌", text: $token).textInputAutocapitalization(.never).autocorrectionDisabled()} footer: {Text("凭据仅由主机保存。此页面不会读取现有令牌；留空保留原令牌。")};Section {Button("保存") {Task {await save()}}.disabled(base.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty);Button("取消") {token = "";selected = nil;creating = false}}}
            else {Section {Button("添加账号") {edit(nil)}};ForEach(accounts.map {CoaxiRow(value: $0)}) {row in Section {Text(row.value.text("provider"));Text(row.value.text("baseUrl")).font(.caption).foregroundStyle(Theme.textSecondary);Text(row.value["hasToken"]?.rvBool == true ? "已保存令牌" : "未设置令牌").font(.caption);HStack {Button("编辑") {edit(row.value)};Spacer();Button("移除", role: .destructive) {removing = row.value}}}}}
        }.disabled(busy).navigationTitle("仓库账号").toolbar {ToolbarItem(placement: .cancellationAction) {Button("关闭") {token = "";dismiss()}.disabled(busy)}}}.interactiveDismissDisabled(busy)
        .task {busy = true;defer {busy = false};do {accounts = try await model.query(.forgeAccountsGet)["accounts"]?.rvArray ?? []} catch {issue = error.localizedDescription}}
        .onDisappear {token = ""}
        .confirmationDialog("移除这个仓库账号？", isPresented: Binding(get: {removing != nil}, set: {if !$0 {removing = nil}}), titleVisibility: .visible) {Button("移除", role: .destructive) {let row = removing;Task {await save(remove: row)}};Button("取消", role: .cancel) {removing = nil}} message: {Text("\(removing?.text("provider") ?? "") · \(removing?.text("baseUrl") ?? "")")}
    }
    private func edit(_ row: JSONValue?) {selected = row;creating = row == nil;id = row?.text("id") ?? UUID().uuidString.lowercased();provider = row?.text("provider") ?? "github";base = row?.text("baseUrl") ?? "https://github.com";token = "";issue = nil}
    private func save(remove: JSONValue? = nil) async {
        guard !busy else {return};let generation = model.connectionGeneration;busy = true;defer {busy = false};issue = nil
        var params: [String: JSONValue] = ["id": .string(remove?.text("id") ?? id), "provider": .string(remove?.text("provider") ?? provider), "baseUrl": .string(remove?.text("baseUrl") ?? base)]
        if remove != nil {params["remove"] = .bool(true)} else if !token.isEmpty {params["token"] = .string(token)}
        do {let result = try await model.query(.forgeAccountsSet, params: .object(params));guard generation == model.connectionGeneration else {throw CancellationError()};accounts = result["accounts"]?.rvArray ?? [];selected = nil;creating = false;token = "";removing = nil} catch is CancellationError {} catch {issue = error.localizedDescription}
    }
}
