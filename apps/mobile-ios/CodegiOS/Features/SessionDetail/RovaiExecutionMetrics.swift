import SwiftUI

private struct ExecutionMetricIdentity: Hashable {let id: String;let epoch: Int;let status: String}
private struct ExecutionMetricsRequest: Hashable {let identities: [ExecutionMetricIdentity];let visible: Bool;let generation: UInt64}

/// One visible Thread reader, retaining the source Run-epoch and current native Session boundaries.
struct RovaiExecutionMetrics: View {
    let model: RovaiWorkspaceModel
    let threadId: String
    let projection: JSONValue
    @Environment(\.scenePhase) private var scenePhase
    @State private var expanded = false
    @State private var visible = false
    @State private var busy = false
    @State private var count = 50
    @State private var metrics: JSONValue?
    @State private var issue: String?
    private var all: [JSONValue] {projection["agentRuns"]?.rvArray ?? []}
    private var rows: [JSONValue] {Array(all.suffix(count))}
    private var selected: [JSONValue] {var seen = Set<String>();return Array((rows + all.filter {["running", "waiting"].contains($0.text("status"))}).filter {seen.insert($0.text("id")).inserted}.suffix(500))}
    private var identities: [ExecutionMetricIdentity] {selected.map {ExecutionMetricIdentity(id: $0.text("id"), epoch: $0["executionEpoch"]?.rvInt ?? 0, status: $0.text("status"))}}
    private var active: Bool {expanded && visible && scenePhase == .active}
    private var live: Bool {selected.contains {["running", "waiting"].contains($0.text("status"))}}
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {Button(expanded ? "收起执行用量与上下文" : "执行用量与上下文") {expanded.toggle()};if expanded {Spacer();Button("刷新") {Task {await load()}}.disabled(busy || !active)}}
            if expanded {
                if let issue {Text(issue + (metrics == nil ? "" : "；保留上次读取结果")).font(.caption).foregroundStyle(Theme.warning)}
                ForEach(metrics?["sessions"]?.rvArray ?? [], id: \.self) {session in GlassRow {
                    VStack(alignment: .leading, spacing: 8) {
                        let member = (projection["members"]?.rvArray ?? []).first {$0.text("agentId") == session.text("agentId")}
                        Text((member?.text("displayName") ?? "队员") + " · 当前原生会话上下文").font(.headline)
                        let fraction = ratio(session)
                        Text(sessionUsageSummary(session, fraction: fraction))
                        if let fraction {ProgressView(value: fraction, total: 1).tint(Theme.accent)}
                        Text([session.text("runtimeKind"), session.text("modelKey"), session.text("source"), session.text("observedAt")].filter {!$0.isEmpty}.joined(separator: " · ")).font(.caption).foregroundStyle(Theme.textSecondary)
                    }
                }}
                if metrics != nil, (metrics?["sessions"]?.rvArray ?? []).isEmpty {Text("当前原生会话上下文尚无有效观测").font(.caption).foregroundStyle(Theme.textSecondary)}
                ForEach(rows, id: \.self) {run in GlassRow {
                    VStack(alignment: .leading, spacing: 8) {
                        let usage = (metrics?["runs"]?.rvArray ?? []).first {$0.text("agentRunId") == run.text("id") && $0["executionEpoch"] == run["executionEpoch"]}
                        let member = (projection["members"]?.rvArray ?? []).first {$0.text("agentId") == run.text("agentId")}
                        Text((member?.text("displayName") ?? "队员") + " · " + run.text("status")).font(.headline)
                        ForEach([("promptInputTotalTokens", "输入Token"), ("outputTokens", "输出Token"), ("cacheReadTokens", "缓存读取"), ("cacheWriteTokens", "缓存写入")], id: \.0) {key, title in LabeledContent(title, value: display(usage?[key]))}
                        if usage?["inputOutputComplete"]?.rvBool != true {Text("输入/输出为部分统计或未知").font(.caption).foregroundStyle(Theme.textSecondary)}
                        Text("最近观测 · " + (usage?["lastObservedAt"]?.stringValue ?? "—")).font(.caption).foregroundStyle(Theme.textSecondary)
                    }
                }}
                if rows.count < all.count, count < 500 {Button("查看更多执行用量") {count = min(500, count + 50)}}
                Text("不从比例推算Token数量；上下文与单次执行用量分别读取。历史稳定后停止轮询，关闭面板或退后台后停止读取。").font(.caption).foregroundStyle(Theme.textSecondary)
            }
        }
        .onAppear {visible = true}
        .onDisappear {visible = false}
        .onChange(of: model.connectionGeneration) {_, _ in metrics = nil;issue = nil;expanded = false}
        .task(id: ExecutionMetricsRequest(identities: identities, visible: active, generation: model.connectionGeneration)) {
            guard active else {return};await load()
            if live {while !Task.isCancelled {do {try await Task.sleep(for: .seconds(4))} catch {return};await load()}}
            else {var previous = 0;for next in [250, 1000, 4000] {do {try await Task.sleep(for: .milliseconds(next - previous))} catch {return};previous = next;await load()}}
        }
    }
    private func number(_ value: JSONValue?) -> Double? {if case .number(let number) = value {return number};return nil}
    private func display(_ value: JSONValue?) -> String {guard let number = number(value) else {return "—"};let formatter = NumberFormatter();formatter.numberStyle = .decimal;formatter.maximumFractionDigits = 0;return formatter.string(from: NSNumber(value: number)) ?? "—"}
    private func percent(_ value: Double) -> String {let formatter = NumberFormatter();formatter.numberStyle = .percent;formatter.maximumFractionDigits = 1;return formatter.string(from: NSNumber(value: value)) ?? "—"}
    private func sessionUsageSummary(_ session: JSONValue, fraction: Double?) -> String {
        let usedTokens: String = display(session["usedTokens"])
        let windowTokens: String = display(session["windowTokens"])
        let fractionText: String
        if let fraction {fractionText = percent(fraction)} else {fractionText = "比例未知"}
        let tokenText: String = [usedTokens, windowTokens].joined(separator: " / ")
        return [tokenText, fractionText].joined(separator: " · ")
    }
    private func ratio(_ session: JSONValue) -> Double? {
        if let used = number(session["usedTokens"]), used >= 0, let window = number(session["windowTokens"]), window > 0, used <= window {return used / window}
        if let native = number(session["nativeRatio"]), native.isFinite, (0...1).contains(native) {return native}
        return nil
    }
    private func load() async {
        guard active, !busy else {return};busy = true;defer {busy = false};let captured = identities, generation = model.connectionGeneration
        do {
            let value = try await model.query(.monitoringExecution, params: .object(["threadId": .string(threadId), "agentRunIds": .array(captured.map {JSONValue.string($0.id)})]));try Task.checkCancellation()
            guard active, captured == identities, generation == model.connectionGeneration else {return}
            guard value["schemaVersion"]?.rvInt == 1 else {throw RovaiHostFailure(code: "执行用量格式不受支持")};metrics = value;issue = nil
        } catch is CancellationError {} catch {issue = error.localizedDescription}
    }
}
