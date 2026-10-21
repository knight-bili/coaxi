import SwiftUI

struct RovaiExecutionHistory: View {
    let model: RovaiWorkspaceModel
    let threadId: String
    let run: JSONValue
    let observed: [JSONValue]
    @State private var expanded = false
    @State private var page: JSONValue?
    @State private var before: Int?
    @State private var busy = false
    @State private var issue: String?
    @State private var full: [String: JSONValue] = [:]
    private var runId: String { run.text("id") }
    private var epoch: Int { run["executionEpoch"]?.rvInt ?? 0 }
    private struct EvidenceRow: Identifiable { let value: JSONValue; var id: String { let canonical = value["canonical"]?.text("operationId") ?? ""; return canonical.isEmpty ? value.text("id") : canonical } }
    private var rows: [EvidenceRow] {
        let source = page == nil ? observed : (page?["evidence"]?.rvArray ?? []) + (page?["activeEvidence"]?.rvArray ?? [])
        let relevant = source.filter { $0.text("agentRunId") == runId && $0["executionEpoch"]?.rvInt == epoch && $0.text("kind") != "reasoning_summary" }
        let groups = Dictionary(grouping: relevant.map { EvidenceRow(value: $0) }, by: \.id)
        return groups.values.compactMap { group in group.max { left, right in let a = left.value; let b = right.value; if a["revision"]?.rvInt != b["revision"]?.rvInt { return (a["revision"]?.rvInt ?? 0) < (b["revision"]?.rvInt ?? 0) }; if a["changeSequence"]?.rvInt != b["changeSequence"]?.rvInt { return (a["changeSequence"]?.rvInt ?? 0) < (b["changeSequence"]?.rvInt ?? 0) }; return (a["sequence"]?.rvInt ?? 0) < (b["sequence"]?.rvInt ?? 0) } }.sorted { ($0.value["canonical"]?["firstEvidenceSequence"]?.rvInt ?? $0.value["sequence"]?.rvInt ?? 0) < ($1.value["canonical"]?["firstEvidenceSequence"]?.rvInt ?? $1.value["sequence"]?.rvInt ?? 0) }
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack { Button { expanded.toggle(); if expanded { Task { await load() } } } label: { Label("\(expanded ? "收起过程" : "查看过程") · \(run.text("status"))", systemImage: expanded ? "chevron.down" : "chevron.right") }; if expanded && before != nil { Button("返回最新") { Task { await load() } }.disabled(busy) } }
            if expanded {
                if let issue { Text(issue).font(.caption).foregroundStyle(Theme.warning) }
                if page?["hasMore"]?.rvBool == true, let cursor = page?["nextBeforeSequence"], cursor != .null { Button("更早的过程") { Task { await load(cursor: cursor.rvInt) } }.disabled(busy) }
                ForEach(rows) { row in evidence(row.value) }
                if busy { ProgressView().frame(maxWidth: .infinity) }
            }
        }.task(id: "\(runId):\(epoch)") { expanded = ["running", "queued", "waiting"].contains(run.text("status")); if expanded { await load() } }
        .onChange(of: run) { _, _ in if expanded && before == nil { Task { await load() } } }
        .onChange(of: observed) { _, _ in if expanded && before == nil { Task { await load() } } }
    }
    @ViewBuilder private func evidence(_ value: JSONValue) -> some View {
        let payload = full[value.text("id")] ?? value["payload"] ?? .object([:]); let canonical = value["canonical"] ?? .object([:]); let kind = value.text("kind")
        if ["narration", "plan", "step"].contains(kind) && canonical.text("operationId").isEmpty {
            let text = [payload.text("text"), payload.text("message"), payload.text("title")].first { !$0.isEmpty } ?? ""
            if !text.isEmpty { MarkdownContent(raw: text) }
        } else {
            ToolCallCard(vm: tool(value, payload: payload, canonical: canonical))
            if ["unknown", "unsettled", "cancelled", "not_executed", "denied"].contains(canonical.text("outcome")) { Text(["cancelled": "已停止", "not_executed": "未执行", "denied": "已拒绝"][canonical.text("outcome")] ?? "结果尚未确定").font(.caption).foregroundStyle(Theme.textSecondary) }
        }
        if value["outputTruncated"]?.rvBool == true { Text("结果过长，部分内容已永久省略。").font(.caption).foregroundStyle(Theme.textSecondary) }
        if let blob = value["contentBlobId"], blob != .null, full[value.text("id")] == nil { Button("读取完整输入和输出") { Task { await read(value) } }.disabled(busy) }
    }
    private func tool(_ value: JSONValue, payload: JSONValue, canonical: JSONValue) -> ToolCallVM {
        let item = payload["item"] ?? .object([:]); let outcome = canonical.text("outcome"); let phase = canonical.text("phase").isEmpty ? value.text("phase") : canonical.text("phase")
        let running = !["terminal", "completed", "failed"].contains(phase) && ["running", "queued", "waiting"].contains(run.text("status"))
        let failed = ["failed", "denied", "unknown", "unsettled", "cancelled", "not_executed"].contains(outcome) || value.text("phase") == "failed"
        let input = text(payload["input"]) ?? text(item["arguments"]) ?? (item.text("command").isEmpty ? nil : JSONValue.object(["command": .string(item.text("command"))]).prettyString)
        let output = text(payload["output"]) ?? text(item["aggregatedOutput"]) ?? text(item["output"])
        let diff = canonical["diffProjection"]?.text("status") == "available" ? (canonical["diffProjection"]?["entries"]?.rvArray ?? []).map { $0.text("diff") }.joined(separator: "\n") : ""
        let name = [canonical.text("toolName"), payload.text("title"), item.text("tool"), canonical.text("presentationHint"), value.text("kind")].first { !$0.isEmpty } ?? "工具调用"
        return ToolCallVM(id: canonical.text("operationId").isEmpty ? value.text("id") : canonical.text("operationId"), rawName: name, kind: canonical.text("activityDomain") == "shell" ? "execute" : payload.text("kind"), state: running ? .running : failed ? .error : .done, input: input, output: output, content: diff.isEmpty ? nil : diff, isError: failed)
    }
    private func text(_ value: JSONValue?) -> String? { guard let value, value != .null else { return nil }; return value.stringValue ?? value.prettyString }
    private func load(cursor: Int? = nil) async {
        guard !busy else { return }; busy = true; defer { busy = false }
        do { let value = try await model.query(.agentRunExecutionPage, params: .object(["threadId": .string(threadId), "agentRunId": .string(runId), "beforeSequence": cursor.map { .number(Double($0)) } ?? .null, "limit": .number(50)])); guard value.text("threadId") == threadId, value.text("agentRunId") == runId, value["schemaVersion"]?.rvInt == 2 else { issue = "执行过程的归属或格式发生变化"; return }; page = value; before = cursor; full = [:]; issue = nil } catch is CancellationError {} catch { issue = error.localizedDescription }
    }
    private func read(_ value: JSONValue) async {
        guard !busy else { return }; busy = true; defer { busy = false }
        do { let result = try await model.query(.agentRunEvidenceGetContent, params: .object(["threadId": .string(threadId), "evidenceId": value["id"] ?? .null])); let payload = result["payload"] ?? .null; guard payload.prettyString.utf8.count <= 4 * 1024 * 1024 else { issue = "完整结果较大，请在工程中查看产物"; return }; full = [value.text("id"): payload]; issue = nil } catch is CancellationError {} catch { issue = error.localizedDescription }
    }
}
