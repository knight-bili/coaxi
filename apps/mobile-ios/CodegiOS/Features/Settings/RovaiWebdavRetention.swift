import SwiftUI

struct RovaiWebdavRetention: View {
    let model: RovaiWorkspaceModel
    let revision: String
    let disabled: Bool
    let onChanged: () -> Void
    @State private var backups = "10", configurations = "20"
    @State private var busy = false, ready = false
    @State private var saved: JSONValue?, confirm: JSONValue?
    @State private var cleanup: [JSONValue] = []
    @State private var cursor: String?, issue: String?, message: String?
    private let owner = "webdav-retention-plan"
    var body: some View {
        Section("远端副本保留") {
            Text("先预览，每批最多移除20份。已移除副本的文件可另行清理。").font(.caption).foregroundStyle(Theme.textSecondary)
            TextField("保留完整备份数量", text: $backups).keyboardType(.numberPad).disabled(saved != nil || disabled || busy)
            TextField("保留配置快照数量", text: $configurations).keyboardType(.numberPad).disabled(saved != nil || disabled || busy)
            HStack {
                Button("预览保留计划") {Task {await work {
                    let original = input("retention_preview", ["commandId": .string(UUID().uuidString.lowercased()), "keepBackups": .number(Double(Int(backups) ?? 0)), "keepConfigurations": .number(Double(Int(configurations) ?? 0))])
                    try store(.object(["request": original, "phase": .string("preparing")]));try await preview(original)
                }}}.disabled(disabled || busy || !ready || saved != nil || !(1...1000).contains(Int(backups) ?? 0) || !(1...1000).contains(Int(configurations) ?? 0))
                Spacer();Button("查找待清理文件") {Task {await work {try await scan()}}}.disabled(disabled || busy || !ready)
            }
            if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning)}
            if let message {Text(message).font(.caption).foregroundStyle(Theme.textSecondary)}
            if let saved, let original = saved["request"] {
                let plan = saved["result"]?["plan"]
                Text("\(plan?["delete"]?.rvArray.count ?? 0)份待移除；之后还有\(plan?["remaining"]?.rvInt ?? 0)份可重新预览").font(.caption)
                ForEach(plan?["delete"]?.rvArray ?? [], id: \.self) {snapshot in Text("\(snapshot.text("kind") == "configuration" ? "配置快照" : "完整备份") · \(snapshot.text("createdAt"))").font(.caption)}
                HStack {
                    Button("核对原批次") {Task {await work {
                        let status = try await request(originalAction(original, "retention_status"))
                        if status["removed"]?.rvBool == true {try store(nil);message = "本批移除已完成";onChanged()} else {message = "原批次尚未完成，原请求继续保留。"}
                    }}}
                    if saved.text("phase") == "preparing" {Button("重读原预览") {Task {await work {try await preview(original)}}}}
                    else {Button("确认移除本批", role: .destructive) {confirm = originalAction(original, "retention_apply", confirmed: true)}.disabled((plan?["delete"]?.rvArray ?? []).isEmpty)}
                }.disabled(disabled || busy || !ready)
                if saved.text("phase") == "previewed" {Button("放弃此预览") {Task {await work {try store(nil)}}}.disabled(disabled || busy)}
            }
            ForEach(cleanup, id: \.self) {entry in HStack {Text(entry["snapshot"]?.text("createdAt") ?? "").font(.caption);Spacer();Button("清理残留文件", role: .destructive) {confirm = input("cleanup_removed", ["snapshotId": entry["snapshot"]?["id"] ?? .null, "expectedDigest": entry["snapshot"]?["sha256"] ?? .null, "confirmed": .bool(true)])}.disabled(disabled || busy)}}
            if let cursor {Button("继续查找更早记录") {Task {await work {try await scan(cursor)}}}.disabled(disabled || busy)}
        }.task {do {let raw = try model.readBody(owner);if !raw.isEmpty {guard let value = JSONValue.parse(raw) else {throw RovaiHostFailure(code: "原保留计划无法读取，保留本机记录")};saved = value};ready = true} catch {issue = error.localizedDescription}}
        .confirmationDialog(confirm?.text("operation") == "retention_apply" ? "移除预览中的这一批？" : "清理已移除副本的文件？", isPresented: Binding(get: {confirm != nil}, set: {if !$0 && !busy {confirm = nil}}), titleVisibility: .visible) {
            Button("确认", role: .destructive) {if let frozen = confirm {Task {await work {
                if frozen.text("operation") == "retention_apply" {if case .object(var fields) = saved {fields["phase"] = .string("applying");try store(.object(fields))};_ = try await request(frozen);try store(nil);onChanged()}
                else {_ = try await request(frozen);try await scan()};confirm = nil
            }}}}.disabled(disabled || busy)
            Button("取消", role: .cancel) {confirm = nil}.disabled(busy)
        } message: {Text("只处理已预览的明确对象；保留本地数据、其他副本与远端墓碑，中断后先核对原结果。")}
    }
    private func input(_ action: String, _ fields: [String: JSONValue] = [:]) -> JSONValue {var value = fields;value["operation"] = .string(action);value["expectedRevision"] = .string(revision);return .object(value)}
    private func originalAction(_ original: JSONValue, _ action: String, confirmed: Bool = false) -> JSONValue {var value: [String: JSONValue] = ["operation": .string(action), "commandId": original["commandId"] ?? .null, "expectedRevision": original["expectedRevision"] ?? .null];if confirmed {value["confirmed"] = .bool(true)};return .object(value)}
    private func request(_ value: JSONValue) async throws -> JSONValue {let generation = model.connectionGeneration;let result = try await model.query(.backupWebdavRemote, params: value);guard generation == model.connectionGeneration, !Task.isCancelled else {throw CancellationError()};return result}
    private func store(_ value: JSONValue?) throws {let raw = try value.map {String(decoding: try JSONEncoder().encode($0), as: UTF8.self)} ?? "";try model.saveBody(owner, body: raw);saved = value}
    private func preview(_ original: JSONValue) async throws {let result = try await request(original);try store(.object(["request": original, "result": result, "phase": .string("previewed")]))}
    private func scan(_ after: String? = nil) async throws {var fields: [String: JSONValue] = [:];if let after {fields["snapshotId"] = .string(after)};let result = try await request(input("cleanup_preview", fields));let next = result["items"]?.rvArray ?? [];if after == nil {cleanup = next} else {let ids = Set(cleanup.compactMap {$0["snapshot"]?["id"]});cleanup += next.filter {!ids.contains($0["snapshot"]?["id"] ?? .null)}};cursor = result["nextCursor"]?.stringValue}
    private func work(_ action: () async throws -> Void) async {guard ready, !busy, !disabled else {return};busy = true;issue = nil;message = nil;defer {busy = false};do {try await action()} catch is CancellationError {} catch {issue = error.localizedDescription}}
}
