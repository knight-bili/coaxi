import SwiftUI

/// 原生列表和文件阅读区；注册表、安装与模型输入不在这个只读页面发生。
@MainActor struct RovaiNativeSkills: View {
    let model: RovaiWorkspaceModel
    @State private var runtimes = ["codex-cli"], target = "codex-cli", search = "", busy = false
    @State private var scan: JSONValue = .object([:]), issue: String?, generation: UInt64?
    private var custom: [JSONValue] {model.members.filter {$0.text("presence") != "removed" && $0["runtimeConfiguration"]?.text("adapterKind") == "custom-acp"}}
    private var skills: [JSONValue] {(scan["skills"]?.rvArray ?? []).filter {search.isEmpty || ($0.text("name") + " " + $0.text("description")).localizedCaseInsensitiveContains(search)}}
    var body: some View {
        List {
            Section {
                Picker("智能体", selection: $target) {
                    ForEach(runtimes, id: \.self) {Text($0).tag($0)}
                    ForEach(custom, id: \.self) {member in Text(member.text("displayName") + " · 自定义智能体").tag("member:" + member.text("agentId"))}
                }.disabled(busy || generation != model.connectionGeneration)
                Text("只读当前执行端的目录；自定义程序按队员保存的具体版本读取。").font(.caption).foregroundStyle(Theme.textSecondary)
            }
            if let issue {Section {Text(issue).foregroundStyle(Theme.warning)}}
            ForEach(scan["errors"]?.rvArray ?? [], id: \.self) {Text($0.stringValue ?? "来源暂不可读").font(.caption).foregroundStyle(Theme.warning)}
            ForEach(skills, id: \.self) {skill in NavigationLink {RovaiNativeSkillReader(model: model, skill: skill)} label: {VStack(alignment: .leading, spacing: 6) {Text(skill.text("name")).font(.headline);Text(skill.text("description")).font(.caption).foregroundStyle(Theme.textSecondary).lineLimit(3)}}}
            if !busy && skills.isEmpty {Text(search.isEmpty ? "这个智能体尚未发现 Skills。" : "没有匹配的 Skill。")}
        }
        .scrollContentBackground(.hidden).navigationTitle("原生 Skills").searchable(text: $search, prompt: "搜索 Skill")
        .toolbar {Button("刷新") {Task {await load(true)}}.disabled(busy || generation != model.connectionGeneration)}
        .task {generation = model.connectionGeneration;do {let values = try await model.query(.runtimeInstallationsList).rvArray;guard generation == model.connectionGeneration else {throw CancellationError()};let kinds = Set(values.map {$0.text("adapterKind")}.filter {!$0.isEmpty && $0 != "custom-acp"});if !kinds.isEmpty {runtimes = kinds.sorted()}} catch {issue = error.localizedDescription};await load(false)}
        .onChange(of: target) {_, _ in scan = .object([:]);Task {await load(false)}}
    }
    private func load(_ refresh: Bool) async {
        guard !busy, generation == model.connectionGeneration else {return};busy = true;let requested = target;defer {busy = false}
        do {var params: [String: JSONValue] = ["adapterKind": .string(requested.hasPrefix("member:") ? "custom-acp" : requested), "refresh": .bool(refresh)];if requested.hasPrefix("member:") {params["agentId"] = .string(String(requested.dropFirst(7)))}
            let result = try await model.query(.nativeSkillsList, params: .object(params));guard generation == model.connectionGeneration, requested == target else {throw CancellationError()};scan = result;issue = nil
        } catch is CancellationError {} catch {issue = error.localizedDescription}
    }
}

@MainActor private struct RovaiNativeSkillReader: View {
    let model: RovaiWorkspaceModel, skill: JSONValue
    @State private var content: JSONValue = .object([:]), files: [JSONValue] = [], path = "SKILL.md", raw = false, busy = false, issue: String?, generation: UInt64?
    var body: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 12) {
                Text(skill.text("entryPath")).font(.caption).foregroundStyle(Theme.textSecondary).textSelection(.enabled)
                Toggle("原文", isOn: $raw)
                if let issue {Text(issue).foregroundStyle(Theme.warning)}
                ForEach(files, id: \.self) {file in Button(file.text("path")) {path = file.text("path")}.disabled(busy)}
                if busy {ProgressView()}
                else if content.text("status") == "text" {if raw {Text(content.text("content")).textSelection(.enabled)} else {MarkdownContent(raw: content.text("content"))}}
                else {Text(content.text("status") == "binary" ? "该文件不是文本文件。" : "正文暂不可读或超过预览上限。")}
            }.padding(.horizontal, Theme.Layout.screenHMargin).padding(.vertical, 12)
        }
        .navigationTitle(skill.text("name")).toolbar {Button("刷新") {Task {await read()}}.disabled(busy || generation != model.connectionGeneration)}
        .task {generation = model.connectionGeneration;await read()}.onChange(of: path) {_, _ in Task {await read()}}
    }
    private func read() async {
        guard !busy, generation == model.connectionGeneration else {return};busy = true;let requested = path;content = .object([:]);defer {busy = false}
        do {let result = try await model.query(.nativeSkillsRead, params: .object(["skillId": skill["id"] ?? .null, "path": .string(requested)]));guard generation == model.connectionGeneration, requested == path else {throw CancellationError()};content = result;files = result["files"]?.rvArray ?? [];issue = nil} catch is CancellationError {} catch {issue = error.localizedDescription}
    }
}
