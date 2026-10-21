import SwiftUI

struct RovaiMemberEditor: View {
    let model: RovaiWorkspaceModel
    let member: JSONValue?
    @Environment(\.dismiss) private var dismiss
    @State private var values: [String: String]
    @State private var traits: String
    @State private var issue: String?
    @State private var submitted: String?
    @State private var discard = false
    private static let fields = [("displayName", "名称", 80), ("teamRole", "团队角色", 120), ("professionalResponsibilities", "专业职责", 300), ("workingPrinciples", "工作准则", 300), ("growthTopic", "成长课题", 300)]
    init(model: RovaiWorkspaceModel, member: JSONValue?) {
        self.model = model; self.member = member
        _values = State(initialValue: Dictionary(uniqueKeysWithValues: Self.fields.map { ($0.0, member?.text($0.0) ?? "") }))
        _traits = State(initialValue: (member?["personalityTraits"]?.rvArray ?? []).compactMap(\.stringValue).joined(separator: "，"))
    }
    private var dirty: Bool { Self.fields.contains { values[$0.0] != (member?.text($0.0) ?? "") } || traits != (member?["personalityTraits"]?.rvArray ?? []).compactMap(\.stringValue).joined(separator: "，") }
    private var disabled: Bool { model.pending != nil || model.busy }
    private var changed: Bool { guard let member, let current = model.members.first(where: { $0.text("agentId") == member.text("agentId") }) else { return false }; return current["version"] != member["version"] }
    var body: some View {
        NavigationStack {
            Form {
                if changed { Section { Text("信息已在其他位置更新，当前草稿保留；请关闭后重新打开再编辑。").foregroundStyle(Theme.warning) } }
                if let issue { Section { Text(issue).font(.caption).foregroundStyle(Theme.warning) } }
                Section("队员信息") {
                    ForEach(Self.fields, id: \.0) { key, title, limit in
                        VStack(alignment: .leading, spacing: 5) { HStack { Text(title).font(.caption).foregroundStyle(Theme.textSecondary); Spacer(); Text("\((values[key] ?? "").unicodeScalars.count) / \(limit)").font(.caption2).foregroundStyle(Theme.textSecondary) }; TextField(title, text: Binding(get: { values[key] ?? "" }, set: { values[key] = $0 }), axis: key == "displayName" || key == "teamRole" ? .horizontal : .vertical).lineLimit(1...5) }.disabled(disabled)
                    }
                    TextField("性格底色（逗号分隔，最多6项）", text: $traits, axis: .vertical).lineLimit(1...3).disabled(disabled)
                }
            }.navigationTitle(member == nil ? "创建队员" : "队员信息").scrollContentBackground(.hidden)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("关闭") { if dirty { discard = true } else { dismiss() } }.disabled(disabled) }; ToolbarItem(placement: .confirmationAction) { Button(member == nil ? "创建" : "保存", action: save).disabled(disabled || changed || (values["displayName"] ?? "").trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || member != nil && !dirty) } }
        }.interactiveDismissDisabled(dirty || disabled)
        .confirmationDialog("放弃未保存的队员信息？", isPresented: $discard, titleVisibility: .visible) { Button("放弃", role: .destructive) { dismiss() } }
        .onChange(of: model.lastResult) { _, value in let result = value?["commandResult"] ?? value; if result?.text("commandId") == submitted { if result?.text("status") == "applied" { dismiss() } else { issue = result?.text("code") } } }
    }
    private func save() {
        if let field = Self.fields.first(where: { (values[$0.0] ?? "").unicodeScalars.count > $0.2 }) { issue = "\(field.1)超过\(field.2)个字符"; return }
        let split = traits.split(whereSeparator: { $0 == "," || $0 == "，" }).map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }.filter { !$0.isEmpty }
        var tags: [String] = []; for tag in split where !tags.contains(tag) { tags.append(tag) }
        guard tags.count <= 6, tags.allSatisfy({ $0.unicodeScalars.count <= 16 && !$0.unicodeScalars.contains(where: CharacterSet.controlCharacters.contains) }) else { issue = "性格标签最多6项，每项最多16个字符，不能包含控制字符。"; return }
        var command = Dictionary(uniqueKeysWithValues: values.map { ($0.key, JSONValue.string($0.value.trimmingCharacters(in: .whitespacesAndNewlines))) }); command["personalityTraits"] = .array(tags.map(JSONValue.string))
        if let member { command["agentId"] = member["agentId"] ?? .null; command["expectedVersion"] = member["version"] ?? .number(0) } else { command["avatarRef"] = .null }
        let id = UUID().uuidString.lowercased(); submitted = id
        Task { await model.execute(member == nil ? .membersCreate : .membersUpdate, params: .object(["commandId": .string(id), "command": .object(command)])) }
    }
}
