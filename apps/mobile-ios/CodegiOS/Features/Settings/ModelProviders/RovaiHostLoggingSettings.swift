import SwiftUI

struct RovaiHostLoggingSettings: View {
    let model: RovaiWorkspaceModel
    @Environment(\.dismiss) private var dismiss
    @State private var identity = ""
    @State private var saved: JSONValue?
    @State private var issue: String?
    @State private var draft: JSONValue = .object(["level": .string("info"), "targets": .array([])])
    @State private var busy = false
    @State private var leaving = false
    private let levels = [("off", "关闭"), ("error", "错误"), ("warn", "警告"), ("info", "信息"), ("debug", "调试"), ("trace", "追踪")]
    private var dirty: Bool { saved != nil && draft != saved?["settings"] }
    private var writable: Bool { saved?["writable"] != .bool(false) }
    private var targets: [JSONValue] { draft["targets"]?.rvArray ?? [] }
    var body: some View {
        Form {
            if !writable { Section { Text("这台主机的日志设置由桌面程序管理，请在主机桌面设置中修改。") } }
            else {
                Section { Text("调整独立主机结构化诊断日志的详细程度。原始故障记录、运行状态、审批和操作回执照常保留。").foregroundStyle(Theme.textSecondary) }
                Section("默认级别") { Picker("日志级别", selection: Binding(get: { draft.text("level") }, set: { setField("level", .string($0)) })) { ForEach(levels, id: \.0) { Text($0.1).tag($0.0) } } }
                Section("按目标覆盖") {
                    ForEach(targets.indices, id: \.self) { index in
                        VStack(alignment: .leading) {
                            TextField("日志目标", text: Binding(get: { targets[index].text("target") }, set: { setTarget(index, "target", $0) })).textInputAutocapitalization(.never).autocorrectionDisabled()
                            Picker("级别", selection: Binding(get: { targets[index].text("level") }, set: { setTarget(index, "level", $0) })) { ForEach(levels, id: \.0) { Text($0.1).tag($0.0) } }
                            Button("移除目标", role: .destructive) { setField("targets", .array(targets.enumerated().filter { $0.offset != index }.map(\.element))) }
                        }
                    }
                    Button("添加日志目标") { setField("targets", .array(targets + [.object(["target": .string(""), "level": .string("info")])])) }.disabled(targets.count >= 1024)
                }
                Section {
                    Button("保存日志设置") { work { try await save() } }.disabled(saved == nil || !dirty)
                    Button("放弃未保存修改") { if let value = saved?["settings"] { draft = value } }.disabled(!dirty)
                }
            }
            Section { Button("重新读取已保存设置") { work { try await read() } }.disabled(dirty) }
            if let issue { Section { Text(issue).foregroundStyle(.red) } }; if busy { ProgressView() }
        }.disabled(busy).navigationTitle("主机日志输出").navigationBarBackButtonHidden(true).interactiveDismissDisabled(dirty || busy)
        .toolbar { ToolbarItem(placement: .cancellationAction) { Button("返回") { if dirty { leaving = true } else { dismiss() } }.disabled(busy) } }
        .confirmationDialog("放弃未保存的日志修改？", isPresented: $leaving) { Button("放弃并返回", role: .destructive) { dismiss() }; Button("继续编辑", role: .cancel) {} }
        .task(id: model.connected) { guard model.connected else { return }; busy = true; do { identity = try await model.presentationIdentity(); try await read() } catch { issue = error.localizedDescription }; busy = false }
    }
    private func current() async throws { guard !identity.isEmpty, try await model.presentationIdentity() == identity else { throw CancellationError() } }
    private func setField(_ field: String, _ value: JSONValue) { var object = draft.rvObject; object[field] = value; draft = .object(object) }
    private func setTarget(_ index: Int, _ field: String, _ value: String) { var rows = targets; guard rows.indices.contains(index) else { return }; var row = rows[index].rvObject; row[field] = .string(value); rows[index] = .object(row); setField("targets", .array(rows)) }
    private func work(_ action: @escaping () async throws -> Void) { guard !busy else { return }; busy = true; issue = nil; Task { do { try await current(); try await action() } catch is CancellationError {} catch { issue = error.localizedDescription }; busy = false } }
    private func read() async throws { let value = try await model.privateConfiguration("logging", params: .object(["operation": .string("get")])); try await current(); saved = value; draft = value["settings"] ?? .null }
    private func save() async throws {
        let names = targets.map { $0.text("target") }
        guard Set(names).count == names.count, names.allSatisfy({ $0.range(of: "^[a-zA-Z0-9_.:-]{1,256}$", options: .regularExpression) != nil }), let saved else { throw RovaiHostFailure(code: "请填写有效且不重复的日志目标名称") }
        let value = try await model.privateConfiguration("logging", params: .object(["operation": .string("set"), "expectedDigest": saved["digest"] ?? .null, "settings": draft])); try await current(); self.saved = value; draft = value["settings"] ?? .null
    }
}
