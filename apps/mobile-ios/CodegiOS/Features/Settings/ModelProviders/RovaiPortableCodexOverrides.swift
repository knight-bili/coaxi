import SwiftUI

private struct PortableCodexField: Identifiable {
    let id: String; let label: String; let options: [JSONValue]
}
// Codeg aff1e6c codex-model-list-editor.tsx 原协议/行为字段与闭集。
private let portableCodexFields: [PortableCodexField] = [
    .init(id: "tool_mode", label: "工具调用方式", options: [.null,.string("direct"),.string("code_mode"),.string("code_mode_only")]),
    .init(id: "multi_agent_version", label: "多智能体协议", options: [.null,.string("v1"),.string("v2")]),
    .init(id: "use_responses_lite", label: "使用精简 Responses 协议", options: [.bool(true),.bool(false)]),
    .init(id: "default_reasoning_summary", label: "推理摘要", options: [.string("auto"),.string("concise"),.string("detailed"),.string("none")]),
    .init(id: "default_verbosity", label: "回答详细程度", options: [.null,.string("low"),.string("medium"),.string("high")]),
    .init(id: "shell_type", label: "终端工具类型", options: [.string("default"),.string("local"),.string("unified_exec"),.string("disabled"),.string("shell_command")]),
    .init(id: "apply_patch_tool_type", label: "文件补丁工具", options: [.null,.string("freeform")]),
    .init(id: "web_search_tool_type", label: "网页搜索内容", options: [.string("text"),.string("text_and_image")]),
    .init(id: "supports_reasoning_summary_parameter", label: "支持推理摘要参数", options: [.bool(true),.bool(false)]),
    .init(id: "support_verbosity", label: "支持详细程度参数", options: [.bool(true),.bool(false)]),
    .init(id: "supports_parallel_tool_calls", label: "支持并行工具调用", options: [.bool(true),.bool(false)]),
    .init(id: "supports_search_tool", label: "支持搜索工具", options: [.bool(true),.bool(false)]),
    .init(id: "supports_image_detail_original", label: "支持原始图片细节", options: [.bool(true),.bool(false)]),
    .init(id: "include_apps_usage_instructions", label: "包含应用使用说明", options: [.bool(true),.bool(false)]),
    .init(id: "include_plugin_usage_instructions", label: "包含插件使用说明", options: [.bool(true),.bool(false)]),
    .init(id: "include_skills_usage_instructions", label: "包含技能使用说明", options: [.bool(true),.bool(false)])
]

struct RovaiPortableCodexOverrides: View {
    let value: JSONValue; let disabled: Bool; let onChange: (JSONValue) -> Void
    private var fields: [String: JSONValue] { if case .object(let object) = value { return object }; return [:] }
    private func set(_ key: String, _ next: JSONValue?) { var copy = fields; copy[key] = next; onChange(.object(copy)) }
    private func label(_ option: JSONValue) -> String {
        switch option { case .null: "不使用"; case .bool(let on): on ? "开启" : "关闭"; case .string(let text): text; default: "保留原设置" }
    }
    private func selection(_ field: PortableCodexField) -> Binding<String> {
        Binding(get: {
            guard let current = fields[field.id] else { return "inherit" }
            return field.options.firstIndex(of: current).map { "option\($0)" } ?? "stored"
        }, set: { key in
            if key == "inherit" { set(field.id, nil) }
            else if key.hasPrefix("option"), let index = Int(key.dropFirst(6)), field.options.indices.contains(index) { set(field.id, field.options[index]) }
        })
    }
    var body: some View {
        DisclosureGroup("高级设置") {
            Text("继承表示不覆盖基准模型。实际程序仍会核对目录和支持范围。").font(.caption).foregroundStyle(Theme.textSecondary)
            ForEach(Array(portableCodexFields.enumerated()), id: \.element.id) { index, field in
                if index == 0 || index == 3 { Text(index == 0 ? "协议" : "模型行为").font(.subheadline).foregroundStyle(Theme.textSecondary) }
                Picker(field.label, selection: selection(field)) {
                    Text("继承基准模型").tag("inherit")
                    if selection(field).wrappedValue == "stored" { Text("保留原设置（由原程序核对）").tag("stored") }
                    ForEach(field.options.indices, id: \.self) { index in Text(label(field.options[index])).tag("option\(index)") }
                }.disabled(disabled)
            }
            ForEach([("description", "模型说明"), ("base_instructions", "模型基础说明")], id: \.0) { key, title in
                Toggle("覆盖" + title, isOn: Binding(get: { fields[key] != nil }, set: { set(key, $0 ? .string("") : nil) })).disabled(disabled)
                if fields[key] != nil { TextField(title, text: Binding(get: { value.text(key) }, set: { set(key, .string($0)) }), axis: .vertical).lineLimit(2...8).disabled(disabled) }
            }
            let other = fields.keys.filter { !Set(portableCodexFields.map(\.id) + ["description", "base_instructions"]).contains($0) }.count
            if other > 0 { Text("另有 \(other) 项原覆盖设置完整保留，未编辑字段不会删除。").font(.caption).foregroundStyle(Theme.textSecondary) }
        }
    }
}
