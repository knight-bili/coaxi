import SwiftUI
import UniformTypeIdentifiers

struct RovaiMissionCreator: View {
    let model: RovaiWorkspaceModel
    let onClose: () -> Void
    @State private var name = "", detail = "", path = "", tags = "", lead = ""
    @State private var ids = Set<String>(), mentions = Set<String>()
    @State private var sources: [JSONValue] = []
    @State private var commandId: String?
    @State private var ready = false, preparing = false, picking = false
    @State private var pickerGeneration: UInt64?
    @State private var issue: String?
    private let owner = "mission-create-draft"
    private var locked: Bool {!ready || preparing || model.busy || model.pending != nil}
    private var members: [JSONValue] {model.members.filter {$0.text("presence") == "present"}}
    private var available: Set<String> {Set(members.map {$0.text("agentId")})}
    private var parsedTags: [String] {var seen = Set<String>();return tags.components(separatedBy: CharacterSet(charactersIn: ",，\n")).map {$0.trimmingCharacters(in: .whitespacesAndNewlines)}.filter {!$0.isEmpty && seen.insert($0).inserted}}
    private var valid: Bool {!name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && name.unicodeScalars.count <= 200 && detail.unicodeScalars.count <= 12000 && !ids.isEmpty && ids.contains(lead) && ids.isSubset(of: available) && mentions.isSubset(of: ids) && parsedTags.count <= 30 && parsedTags.allSatisfy {$0.unicodeScalars.count <= 24}}
    var body: some View {
        NavigationStack {Form {
            if let issue {Section {Text(issue).font(.caption).foregroundStyle(Theme.warning)}}
            if model.pending != nil {Section {Text("连接中断时会保留草稿和附件。关闭后可在工作区核对原结果。").font(.caption).foregroundStyle(Theme.textSecondary)}}
            Section("内容") {TextField("名称", text: changed($name));TextField("使命描述", text: changed($detail), axis: .vertical).lineLimit(3...8);TextField("标签，用逗号分开", text: changed($tags))}.disabled(locked)
            Section("工作目录") {Picker("工程", selection: changed($path)) {Text("快速对话").tag("");ForEach((model.navigation["projects"]?.rvArray ?? []).map {RovaiRow(value: $0)}, id: \.value) {row in Text(row.value.text("name")).tag(row.value.text("projectPath"))}}}.disabled(locked)
            Section("队员与负责人") {
                ForEach(members.map {RovaiRow(value: $0)}) {row in VStack(alignment: .leading, spacing: 8) {
                    Toggle(row.value.text("displayName"), isOn: Binding(get: {ids.contains(row.id)}, set: {selected in if selected {ids.insert(row.id)} else {ids.remove(row.id);mentions.remove(row.id)};if !ids.contains(lead) {lead = ids.sorted().first ?? ""};persist()}))
                    if ids.contains(row.id) {Toggle("在描述中提及此队员", isOn: Binding(get: {mentions.contains(row.id)}, set: {selected in if selected {mentions.insert(row.id)} else {mentions.remove(row.id)};persist()})).font(.caption)}
                }}
                ForEach(ids.subtracting(available).sorted(), id: \.self) {id in HStack {Text("草稿中的队员已不可用").font(.caption).foregroundStyle(Theme.warning);Spacer();Button("移除") {ids.remove(id);mentions.remove(id);if lead == id {lead = ids.sorted().first ?? ""};persist()}}}
                Picker("负责人", selection: changed($lead)) {ForEach(members.filter {ids.contains($0.text("agentId"))}.map {RovaiRow(value: $0)}) {row in Text(row.value.text("displayName")).tag(row.id)}}
            }.disabled(locked)
            Section("附件 · \(sources.count)/10") {
                ForEach(sources, id: \.self) {source in HStack {Text(source["intent"]?.text("displayName") ?? "附件");Spacer();Button("移除", role: .destructive) {let previous = sources;sources.removeAll {$0 == source};do {try model.saveBody(owner, body: document());model.removeMissionFile(source)} catch {sources = previous;issue = error.localizedDescription}}.disabled(locked)}}
                Button(preparing ? "正在保存…" : "添加附件") {pickerGeneration = model.connectionGeneration;picking = true}.disabled(locked || sources.count >= 10)
            }
        }.navigationTitle("新建使命").navigationBarTitleDisplayMode(.inline).toolbar {
            ToolbarItem(placement: .cancellationAction) {Button("保存草稿并关闭") {close()}.disabled(preparing)}
            ToolbarItem(placement: .confirmationAction) {Button("创建使命") {submit()}.disabled(locked || !valid)}
        }}.interactiveDismissDisabled(true)
        .fileImporter(isPresented: $picking, allowedContentTypes: [.item], allowsMultipleSelection: true) {result in
            guard !locked, pickerGeneration == model.connectionGeneration else {issue = "工作区已切换，请重新选择附件。";return}
            switch result {case .failure(let error): issue = error.localizedDescription;case .success(let urls):
                guard sources.count + urls.count <= 10 else {issue = "使命最多保留10份附件";return};preparing = true;let original = sources
                Task {var added: [JSONValue] = [];defer {preparing = false};do {for url in urls {added.append(try await model.prepareMissionFile(url));guard pickerGeneration == model.connectionGeneration else {throw CancellationError()}};sources = original + added;try model.saveBody(owner, body: document())} catch {for source in added {model.removeMissionFile(source)};sources = original;issue = error.localizedDescription}}
            }
        }
        .task {do {let raw = try model.readBody(owner);if !raw.isEmpty {guard let value = JSONValue.parse(raw), value["version"]?.rvInt == 1 else {throw RovaiHostFailure(code: "原使命草稿格式无法读取，原记录保留")};name = value.text("name");detail = value.text("description");path = value.text("path");tags = value.text("tags");lead = value.text("lead");ids = Set((value["ids"]?.rvArray ?? []).compactMap(\.stringValue));mentions = Set((value["mentions"]?.rvArray ?? []).compactMap(\.stringValue));sources = value["sources"]?.rvArray ?? [];commandId = value["commandId"]?.stringValue};ready = true} catch {issue = error.localizedDescription}}
        .onChange(of: model.lastResult) {_, result in guard model.lastOperation == .missionsCreate, model.lastInputOwner == owner, result?.text("commandId") == commandId else {return};if ["applied", "accepted"].contains(result?.text("status") ?? "") {onClose()} else if result?.text("status") == "rejected" {commandId = nil;issue = result?.text("code");persist()}}
    }
    private func changed<T>(_ value: Binding<T>) -> Binding<T> {Binding(get: {value.wrappedValue}, set: {value.wrappedValue = $0;persist()})}
    private func document() throws -> String {let value = JSONValue.object(["version": .number(1), "name": .string(name), "description": .string(detail), "path": .string(path), "tags": .string(tags), "lead": .string(lead), "ids": .array(ids.sorted().map(JSONValue.string)), "mentions": .array(mentions.sorted().map(JSONValue.string)), "sources": .array(sources), "commandId": commandId.map(JSONValue.string) ?? .null]);let encoder = JSONEncoder();encoder.outputFormatting = [.sortedKeys];return String(decoding: try encoder.encode(value), as: UTF8.self)}
    private func persist() {guard ready else {return};do {try model.saveBody(owner, body: document())} catch {issue = error.localizedDescription}}
    private func close() {guard !preparing else {return};if model.lastOperation == .missionsCreate, model.lastInputOwner == owner, model.lastResult?.text("commandId") == commandId, ["applied", "accepted"].contains(model.lastResult?.text("status") ?? "") {onClose();return};do {if ready {try model.saveBody(owner, body: document())};onClose()} catch {issue = error.localizedDescription}}
    private func submit() {
        guard !locked, valid else {return};let id = commandId ?? UUID().uuidString.lowercased();commandId = id
        var content: [JSONValue] = [.object(["kind": .string("text"), "text": .string(detail)])]
        for agent in mentions.sorted() {content.append(.object(["kind": .string("text"), "text": .string("\n")]));content.append(.object(["kind": .string("member_mention"), "agentId": .string(agent)]))}
        let command = JSONValue.object(["title": .string(name.trimmingCharacters(in: .whitespacesAndNewlines)), "description": .string(detail), "descriptionContent": .array(content), "projectPath": .string(path), "projectBindingKind": .string(path.isEmpty ? "quick_chat" : "directory"), "memberAgentIds": .array(ids.sorted().map(JSONValue.string)), "defaultLeadAgentId": .string(lead), "tags": .array(parsedTags.map(JSONValue.string))])
        do {let saved = try document();try model.saveBody(owner, body: saved);let generation = model.connectionGeneration;Task {guard generation == model.connectionGeneration else {return};await model.executeMissionBatch(.missionsCreate, commandId: id, command: command, kept: [], sources: sources, owner: owner, body: saved)}} catch {issue = error.localizedDescription}
    }
}
