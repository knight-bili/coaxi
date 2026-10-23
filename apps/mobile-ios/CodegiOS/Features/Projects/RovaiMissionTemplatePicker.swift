import SwiftUI

struct RovaiMissionTemplatePicker: View {
    let model: RovaiWorkspaceModel
    let disabled: Bool
    let memberCount: Int
    let materials: [(String, String)]
    let onChooseMaterials: () -> Void
    let onApply: (String, String) -> Void
    @State private var identity = ""
    @State private var open = false
    @State private var busy = false
    @State private var confirmed = false
    @State private var rows: [JSONValue] = []
    @State private var next: JSONValue = .null
    @State private var selected: JSONValue?
    @State private var configuration: JSONValue?
    @State private var issue: String?
    @State private var mappings: [Int: String] = [:]
    private var blocks: [JSONValue] { configuration?["prompt_blocks"]?.rvArray ?? [] }
    private var required: [Int] { blocks.indices.filter { blocks[$0].text("type") != "text" } }
    private var settings: Bool { guard let configuration else { return false }; return ["agent_type", "mode_id", "base_branch"].contains { configuration[$0] != nil && configuration[$0] != .null } || !(configuration["config_values"]?.rvObject ?? [:]).isEmpty }
    private var canApply: Bool { selected != nil && configuration != nil && !disabled && !busy && required.allSatisfy { index in materials.contains { $0.0 == mappings[index] } } && (!settings || (confirmed && memberCount > 0)) }
    var body: some View {
        Section("任务模板") {
            Button(open ? "收起任务模板" : "使用任务模板") { open.toggle(); if open { work { identity = try await model.presentationIdentity(); selected = nil; configuration = nil; try await list() } } }.disabled(disabled || busy)
            if open {
                if let selected, let configuration {
                    Text(selected.text("title").isEmpty ? selected.text("name") : selected.text("title")).font(.headline)
                    if settings { Toggle("改用当前工程、分支和已选择队员的运行设置，原模板继续保留", isOn: $confirmed); if memberCount == 0 { Text("请先选择使命执行队员。").font(.caption) } }
                    if !required.isEmpty {
                        Text("旧材料路径不会直接读取。先添加本次材料，再逐项对应。").font(.caption).foregroundStyle(Theme.textSecondary)
                        Button("选择本次材料", action: onChooseMaterials)
                        ForEach(required, id: \.self) { index in
                            Picker(blocks[index].text("name").isEmpty ? "原材料 \(index + 1)" : blocks[index].text("name"), selection: Binding(get: { mappings[index] ?? "" }, set: { mappings[index] = $0 })) {
                                Text("选择已加入的材料").tag(""); ForEach(materials, id: \.0) { Text($0.1).tag($0.0) }
                            }
                        }
                    }
                    Text("填入会替换当前标题和说明；确认创建使命前不会启动。").font(.caption).foregroundStyle(Theme.textSecondary)
                    Button("填入标题和说明") { work { try await current()
                        let body = configuration["display_text"]?.stringValue ?? blocks.filter { $0.text("type") == "text" }.map { $0.text("text") }.joined(separator: "\n")
                        onApply(selected.text("title"), body); open = false; self.selected = nil; self.configuration = nil
                    } }.disabled(!canApply)
                    Button("返回列表") { self.selected = nil; self.configuration = nil }
                } else {
                    ForEach(rows, id: \.self) { row in Button(row.text("name")) { work { try await choose(row.text("id")) } } }
                    if next != .null { Button("更多模板") { work { try await list(more: true) } } }
                    if !busy && rows.isEmpty { Text("还没有模板，可在主机配置迁移与模板设置中创建或搬入。").foregroundStyle(Theme.textSecondary) }
                }
                if let issue { Text(issue).foregroundStyle(.red) }; if busy { ProgressView() }
            }
        }.disabled(disabled || busy)
        .onChange(of: model.connectionGeneration) { _, _ in open = false; identity = ""; rows = []; selected = nil; configuration = nil; mappings = [:]; confirmed = false; busy = false; issue = nil }
    }
    private func current() async throws { guard !identity.isEmpty, try await model.presentationIdentity() == identity else { throw CancellationError() } }
    private func work(_ action: @escaping () async throws -> Void) { guard !busy else { return }; let generation = model.connectionGeneration; busy = true; issue = nil; Task { do { try await action() } catch is CancellationError {} catch { if generation == model.connectionGeneration { issue = error.localizedDescription } }; if generation == model.connectionGeneration { busy = false } } }
    private func list(more: Bool = false) async throws {
        var input: [String: JSONValue] = ["operation": .string("list"), "limit": .number(100)]; if more { input["after"] = next }
        let value = try await model.privateConfiguration("templates", params: .object(input)); try await current(); let loaded = value["items"]?.rvArray ?? []
        rows = more ? rows + loaded.filter { item in !rows.contains { $0["id"] == item["id"] } } : loaded; next = value["next"] ?? .null
    }
    private func choose(_ id: String) async throws {
        let value = try await model.privateConfiguration("templates", params: .object(["operation": .string("get"), "id": .string(id)])); try await current()
        guard value != .null, let config = JSONValue.parse(value.text("config")), case .object = config else { throw RovaiHostFailure(code: "原模板需要先补齐配置，完整内容仍保留") }
        if let material = config["prompt_blocks"] { guard case .array = material else { throw RovaiHostFailure(code: "原模板材料无法读取") } }
        let blocks = config["prompt_blocks"]?.rvArray ?? []; guard blocks.count <= 8192, blocks.allSatisfy({ if case .object = $0 { return true }; return false }), blocks.filter({ $0.text("type") != "text" }).count <= 128 else { throw RovaiHostFailure(code: "原模板材料无法读取或数量超过限制") }
        selected = value; configuration = config; mappings = [:]; confirmed = false
    }
}
