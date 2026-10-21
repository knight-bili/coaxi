import SwiftUI
import UniformTypeIdentifiers

/// The original MissionEdit definition boundary, with one version-fenced attachment batch.
struct RovaiMissionEdit: View {
    let model: RovaiWorkspaceModel
    let mission: JSONValue
    @Environment(\.dismiss) private var dismiss
    @State private var baseline: JSONValue = .null
    @State private var title = "", tags = ""
    @State private var content: [JSONValue] = []
    @State private var kept: [String] = []
    @State private var sources: [JSONValue] = []
    @State private var commandId = UUID().uuidString.lowercased()
    @State private var legacySent: JSONValue?, latest: JSONValue?, preview: JSONValue?, issue: String?
    @State private var ready = false, busy = false, picking = false
    @State private var generation: UInt64?, pickerGeneration: UInt64?
    private var owner: String {"mission-edit:\(mission.text("missionId"))"}
    private var blocked: Bool {!ready || busy || model.busy || model.pending != nil || generation != model.connectionGeneration}
    private var locked: Bool {blocked || legacySent != nil}
    private var rendered: String {missionDescriptionDisplay(content, members: model.members)}
    private var parsedTags: [String] {var seen = Set<String>();return tags.components(separatedBy: CharacterSet(charactersIn: ",，\n")).map {$0.trimmingCharacters(in: .whitespacesAndNewlines)}.filter {!$0.isEmpty && seen.insert($0).inserted}}
    private var changed: Bool {title.trimmingCharacters(in: .whitespacesAndNewlines) != baseline.text("title") || content != contentOf(baseline) || parsedTags != (baseline["tags"]?.rvArray ?? []).compactMap(\.stringValue) || kept != (baseline["attachments"]?.rvArray ?? []).map {$0.text("id")} || !sources.isEmpty}
    private var valid: Bool {!title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && title.trimmingCharacters(in: .whitespacesAndNewlines).unicodeScalars.count <= 200 && rendered.unicodeScalars.count <= 12000 && parsedTags.count <= 30 && parsedTags.allSatisfy {$0.unicodeScalars.count <= 24} && kept.count + sources.count <= 10}
    private func contentOf(_ value: JSONValue) -> [JSONValue] {value["descriptionContent"]?.rvArray ?? [.object(["kind": .string("text"), "text": .string(value.text("description"))])]}
    var body: some View {
        NavigationStack {Form {
            if let issue {Section {Text(issue).font(.caption).foregroundStyle(Theme.warning)}}
            if model.pending != nil {Section {Text("原请求待核对，完整草稿和附件已保留。关闭后可在工作区核对或重试同一请求。").font(.caption).foregroundStyle(Theme.textSecondary)}}
            Section("内容") {
                TextField("名称", text: changedBinding($title))
                RovaiMissionDescriptionInput(content: content, members: model.members, disabled: locked) {content = $0;persist()}
                TextField("标签，用逗号分开", text: changedBinding($tags))
            }.disabled(locked)
            Section("工作目录与队伍") {
                Text(baseline.text("projectPath").isEmpty ? "快速对话" : baseline.text("projectPath"))
                let ids = (baseline["memberAgentIds"]?.rvArray ?? []).compactMap(\.stringValue)
                Text(ids.map {id in model.members.first {$0.text("agentId") == id}?.text("displayName") ?? "已不可用队员"}.joined(separator: "、"))
            } footer: {Text("队伍与队长由原管理入口调整。保存描述中的新提及会邀请对应队员；移除提及保留原成员。")}
            Section("附件 · \(kept.count + sources.count)/10") {
                ForEach(baseline["attachments"]?.rvArray ?? [], id: \.self) {attachment in
                    HStack {
                        Button(attachment.text("displayName")) {preview = attachmentSource(attachment)}
                        Spacer()
                        Button(kept.contains(attachment.text("id")) ? "移除引用" : "撤销移除") {
                            let id = attachment.text("id")
                            if kept.contains(id) {kept.removeAll {$0 == id}} else {kept = (baseline["attachments"]?.rvArray ?? []).map {$0.text("id")}.filter {kept.contains($0) || $0 == id}}
                            persist()
                        }.disabled(locked || !kept.contains(attachment.text("id")) && kept.count + sources.count >= 10)
                    }
                }
                ForEach(sources, id: \.self) {source in HStack {
                    Text(source["intent"]?.text("displayName") ?? "新附件");Spacer()
                    Button("移除新附件", role: .destructive) {let previous = sources;sources.removeAll {$0 == source};do {try save();model.removeMissionFile(source)} catch {sources = previous;issue = error.localizedDescription}}.disabled(locked)
                }}
                Button(busy ? "正在保存…" : "添加附件") {pickerGeneration = model.connectionGeneration;picking = true}.disabled(locked || kept.count + sources.count >= 10)
            }
            if legacySent != nil {Section("原保存请求") {
                Text("旧版编辑器留下的请求必须先核对；结果不明确时不会生成新命令。").font(.caption)
                Button("核对原保存") {Task {await reconcileLegacy()}}.disabled(blocked)
                Button("重试原保存") {if let frozen = legacySent {Task {await model.execute(.missionsUpdate, params: frozen)}}}.disabled(blocked)
            }}
            Section {Button("查看最新内容并重新编辑") {Task {await loadLatest()}}.disabled(locked)}
        }.navigationTitle("编辑使命").navigationBarTitleDisplayMode(.inline).toolbar {
            ToolbarItem(placement: .cancellationAction) {Button("保存草稿并关闭") {close()}.disabled(busy)}
            ToolbarItem(placement: .confirmationAction) {Button("保存使命") {Task {await submit()}}.disabled(locked || !valid || !changed)}
        }}.interactiveDismissDisabled(true)
        .task {restore()}
        .fileImporter(isPresented: $picking, allowedContentTypes: [.item], allowsMultipleSelection: true) {result in
            guard !locked, pickerGeneration == model.connectionGeneration else {issue = "工作区已切换，请重新选择附件。";return}
            switch result {case .failure(let error): issue = error.localizedDescription;case .success(let urls):
                guard kept.count + sources.count + urls.count <= 10 else {issue = "使命最多保留10份附件";return}
                busy = true;let previous = sources
                Task {var added: [JSONValue] = [];defer {busy = false};do {
                    for url in urls {added.append(try await model.prepareMissionFile(url));guard pickerGeneration == model.connectionGeneration else {throw CancellationError()}}
                    sources = previous + added;try save()
                } catch {sources = previous;for source in added {model.removeMissionFile(source)};issue = error.localizedDescription}}
            }
        }
        .onChange(of: model.lastResult) {_, result in
            guard model.lastOperation == .missionsUpdate, model.lastInputOwner == owner || legacySent != nil else {return}
            finishReceipt(result?["commandResult"] ?? result)
        }
        .sheet(isPresented: Binding(get: {preview != nil}, set: {if !$0 {preview = nil}})) {if let preview {RovaiFilePreview(model: model, source: preview)}}
        .sheet(isPresented: Binding(get: {latest != nil}, set: {if !$0 {latest = nil}})) {
            if let latest {NavigationStack {Form {
                Section("最新内容") {Text(latest.text("title"));Text(missionDescriptionDisplay(contentOf(latest), members: model.members));ForEach(latest["attachments"]?.rvArray ?? [], id: \.self) {Text($0.text("displayName"))}}
                Section {Text("重新编辑将放弃当前草稿及新选附件。原使命附件与已发布消息保留；此操作只加载内容，不提交修改。");Button("放弃草稿，使用最新内容", role: .destructive) {replaceDraft(with: latest)}.disabled(locked)}
            }.navigationTitle("使用最新内容？").toolbar {Button("保留当前草稿") {self.latest = nil}}}}
        }
    }
    private func changedBinding<T>(_ value: Binding<T>) -> Binding<T> {Binding(get: {value.wrappedValue}, set: {value.wrappedValue = $0;persist()})}
    private func document() throws -> String {
        let value = JSONValue.object(["version": .number(2), "title": .string(title), "description": .string(rendered), "structuredDescription": .array(content), "tags": .string(tags), "baseline": baseline, "kept": .array(kept.map(JSONValue.string)), "sources": .array(sources), "commandId": .string(commandId), "sent": legacySent ?? .null])
        let encoder = JSONEncoder();encoder.outputFormatting = [.sortedKeys];return String(decoding: try encoder.encode(value), as: UTF8.self)
    }
    private func save() throws {guard generation == model.connectionGeneration else {throw CancellationError()};try model.saveBody(owner, body: document())}
    private func persist() {guard ready else {return};do {try save()} catch {issue = error.localizedDescription}}
    private func restore() {
        guard !ready else {return};generation = model.connectionGeneration
        baseline = mission;title = mission.text("title");content = contentOf(mission);tags = (mission["tags"]?.rvArray ?? []).compactMap(\.stringValue).joined(separator: ", ");kept = (mission["attachments"]?.rvArray ?? []).map {$0.text("id")}
        do {let raw = try model.readBody(owner);if !raw.isEmpty {
            guard let value = JSONValue.parse(raw), value["version"] == nil || value["version"]?.rvInt == 2, let previous = value["baseline"], previous.text("missionId") == mission.text("missionId") else {throw RovaiHostFailure(code: "原使命草稿无法读取，原记录保留")}
            baseline = previous;title = value.text("title");tags = value.text("tags");let storedID = value.text("commandId");if !storedID.isEmpty {commandId = storedID}
            if let frozen = value["sent"], frozen != .null {legacySent = frozen}
            content = value["structuredDescription"]?.rvArray ?? (value.text("description") == previous.text("description") ? contentOf(previous) : [.object(["kind": .string("text"), "text": .string(value.text("description"))])])
            if value["version"]?.rvInt == 2 {guard let ids = value["kept"]?.rvArray else {throw RovaiHostFailure(code: "原附件保留清单无法读取")};kept = ids.compactMap(\.stringValue)} else {kept = (previous["attachments"]?.rvArray ?? []).map {$0.text("id")}}
            sources = value["sources"]?.rvArray ?? []
        };ready = true} catch {issue = error.localizedDescription}
    }
    private func close() {guard !busy else {return};guard ready, generation == model.connectionGeneration else {dismiss();return};do {try save();dismiss()} catch {issue = error.localizedDescription}}
    private func finishReceipt(_ result: JSONValue?) {
        guard result?.text("commandId") == commandId else {return}
        if ["applied", "accepted"].contains(result?.text("status") ?? "") {do {try model.saveBody(owner, body: "");ready = false;dismiss()} catch {issue = error.localizedDescription}}
        else if result?.text("status") == "rejected" {legacySent = nil;commandId = UUID().uuidString.lowercased();issue = result?.text("code");persist()}
    }
    private func attachmentSource(_ attachment: JSONValue) -> JSONValue {.object(["kind": .string("attachment"), "threadId": baseline["threadId"] ?? .null, "locator": .object(["owner": .string("mission"), "threadId": baseline["threadId"] ?? .null, "missionId": mission["missionId"] ?? .null, "attachmentRefId": attachment["id"] ?? .null])])}
    private func reconcileLegacy() async {
        guard !blocked, let frozen = legacySent else {return};busy = true;defer {busy = false}
        do {let result = try await model.query(.commandsReconcile, params: .object(["operation": .string(RovaiOperation.missionsUpdate.rawValue), "params": frozen]));guard generation == model.connectionGeneration else {throw CancellationError()};if result.text("state") == "recorded" {finishReceipt(result["result"])} else {issue = "原结果尚未记录；可以稍后核对或重试原保存。"}} catch {issue = error.localizedDescription}
    }
    private func loadLatest() async {
        guard !locked else {return};busy = true;defer {busy = false}
        do {let value = try await model.query(.missionsGet, params: .object(["missionId": mission["missionId"] ?? .null]));guard generation == model.connectionGeneration else {throw CancellationError()};latest = value} catch {issue = error.localizedDescription}
    }
    private func replaceDraft(with latest: JSONValue) {
        guard !locked else {return}
        let previousBaseline = baseline, previousTitle = title, previousContent = content, previousTags = tags, previousKept = kept, previousSources = sources, previousID = commandId
        baseline = latest;title = latest.text("title");content = contentOf(latest);tags = (latest["tags"]?.rvArray ?? []).compactMap(\.stringValue).joined(separator: ", ");kept = (latest["attachments"]?.rvArray ?? []).map {$0.text("id")};sources = [];commandId = UUID().uuidString.lowercased()
        do {try save();for source in previousSources {model.removeMissionFile(source)};self.latest = nil;issue = nil}
        catch {baseline = previousBaseline;title = previousTitle;content = previousContent;tags = previousTags;kept = previousKept;sources = previousSources;commandId = previousID;issue = error.localizedDescription}
    }
    private func submit() async {
        guard !locked, valid, changed else {return};busy = true;defer {busy = false}
        let command = JSONValue.object(["missionId": mission["missionId"] ?? .null, "expectedDetailsVersion": baseline["detailsVersion"] ?? .null, "title": .string(title.trimmingCharacters(in: .whitespacesAndNewlines)), "descriptionContent": .array(content), "tags": .array(parsedTags.map(JSONValue.string))])
        do {let body = try document();try save();guard generation == model.connectionGeneration else {throw CancellationError()};await model.executeMissionBatch(.missionsUpdate, commandId: commandId, command: command, kept: kept, sources: sources, owner: owner, body: body)} catch {issue = error.localizedDescription}
    }
}
