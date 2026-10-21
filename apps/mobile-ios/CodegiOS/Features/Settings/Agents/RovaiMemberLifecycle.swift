import SwiftUI

struct RovaiMemberLifecycle: View {
    let model: RovaiWorkspaceModel
    let member: JSONValue
    @State private var preview: JSONValue?
    @State private var name = ""
    @State private var issue: String?
    @State private var loading = false
    @State private var submitted: String?
    private var disabled: Bool { loading || model.busy || model.pending != nil }
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Button(member.text("presence") == "present" ? "暂离" : "归队") { Task { await model.command(.membersPresenceSet, value: .object(["agentId": member["agentId"] ?? .null, "expectedVersion": member["version"] ?? .number(0), "presence": .string(member.text("presence") == "present" ? "away" : "present")])) } }.disabled(disabled || member.text("presence") == "removed")
                Button("移除", role: .destructive) { Task { await inspect() } }.disabled(disabled)
            }.buttonStyle(.borderless)
            if let issue { Text(issue).font(.caption).foregroundStyle(Theme.warning) }
        }
        .sheet(isPresented: Binding(get: { preview != nil }, set: { if !$0 && !disabled { preview = nil } })) {
            NavigationStack { Form {
                if let preview {
                    Section("影响范围") { LabeledContent("正在执行", value: "\(preview["nonTerminalAgentRunCount"]?.rvInt ?? 0)"); LabeledContent("所在对话", value: "\(preview["currentThreadMembershipCount"]?.rvInt ?? 0)"); LabeledContent("未完成责任任务", value: "\(preview["openAssignedTaskCount"]?.rvInt ?? 0)"); LabeledContent("担任负责人", value: "\(preview["defaultLeadThreadCount"]?.rvInt ?? 0)") }
                    Section { Text("移除后历史身份和记录保留。"); if preview["removable"]?.rvBool != true { Text("当前仍有责任或执行需要处理，请先完成对应交接。").foregroundStyle(Theme.warning) }; TextField("输入队员名称确认", text: $name).disabled(disabled) }
                    Section { Button("确认移除", role: .destructive) { let id = UUID().uuidString.lowercased(); submitted = id; Task { await model.execute(.membersRemove, params: .object(["commandId": .string(id), "command": .object(["agentId": preview["agentId"] ?? .null, "expectedVersion": preview["version"] ?? .number(0), "confirmationName": .string(name)])])) } }.disabled(disabled || preview["removable"]?.rvBool != true || name != preview.text("displayName")) }
                    if let issue { Section { Text(issue).font(.caption).foregroundStyle(Theme.warning) } }
                }
            }.navigationTitle("移除队员").toolbar { ToolbarItem(placement: .cancellationAction) { Button("取消") { preview = nil }.disabled(disabled) } } }.interactiveDismissDisabled(disabled)
        }
        .onChange(of: model.lastResult) { _, value in let receipt = value?["commandResult"] ?? value; if receipt?.text("commandId") == submitted { if receipt?.text("status") == "applied" { preview = nil; name = "" } else { issue = receipt?.text("code") } } }
    }
    private func inspect() async {
        loading = true; defer { loading = false }
        do { let value = try await model.query(.membersRemovalPreview, params: .object(["agentId": member["agentId"] ?? .null])); guard value.text("agentId") == member.text("agentId") else { issue = "队员归属发生变化"; return }; preview = value; name = ""; issue = nil } catch is CancellationError {} catch { issue = error.localizedDescription }
    }
}
