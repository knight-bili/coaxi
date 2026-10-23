import SwiftUI
import UniformTypeIdentifiers

private let monitoringMetrics: [(String, String)] = [("promptInputTotalTokens", "输入Token"), ("outputTokens", "输出Token"), ("uncachedInputTokens", "未缓存输入"), ("cacheReadTokens", "缓存读取"), ("cacheWriteTokens", "缓存写入"), ("reasoningOutputTokens", "推理输出"), ("cacheReadShare", "缓存读取占比"), ("requestCacheHitRate", "请求缓存命中率")]
private struct MonitoringCoverageMetric {
    let key: String
    let title: String
}

private let monitoringCoverageMetrics: [MonitoringCoverageMetric] =
    monitoringMetrics
        .filter { $0.0 != "cacheReadShare" }
        .map { MonitoringCoverageMetric(key: $0.0, title: $0.1) }
        + [MonitoringCoverageMetric(key: "cost", title: "成本")]
private func monitoringNumber(_ value: JSONValue?, percent: Bool = false) -> String {guard case .number(let number) = value else {return "—"};let format = NumberFormatter();format.numberStyle = percent ? .percent : .decimal;format.maximumFractionDigits = percent ? 1 : 0;return format.string(from: NSNumber(value: number)) ?? "—"}
private func monitoringMoney(_ value: JSONValue?) -> String {let values = (value?.rvArray ?? []).map {row in let origin = [row.text("kind"), row.text("source")].filter {!$0.isEmpty}.joined(separator: " / ");return row.text("currency") + " " + row.text("amount") + (origin.isEmpty ? "" : "（" + origin + "）")};return values.isEmpty ? "—" : values.joined(separator: " · ")}
private struct MonitoringRequestKey: Hashable {let filter: JSONValue;let active: Bool;let generation: UInt64}

/// Native presentation of the original RuntimeMonitoring snapshot; no inference or collection is launched.
struct RovaiMonitoring: View {
    let model: RovaiWorkspaceModel
    @State private var surface = 0
    var body: some View {
        VStack(spacing: 0) {
            Picker("用量视图", selection: $surface) {Text("运行监控").tag(0);Text("用量看板").tag(1);Text("原生历史").tag(2)}.pickerStyle(.segmented).padding(.horizontal).padding(.vertical, 8)
            if surface == 0 {RovaiRuntimeMonitoring(model: model)} else {RovaiUsageDashboard(model: model, history: surface == 2).id(surface)}
        }
    }
}

private struct RovaiRuntimeMonitoring: View {
    let model: RovaiWorkspaceModel
    @Environment(\.scenePhase) private var scenePhase
    @State private var range = "24h"
    @State private var runtime = ""
    @State private var provider = ""
    @State private var selectedModel = ""
    @State private var costKind = ""
    @State private var snapshot: JSONValue?
    @State private var loadedFilter: JSONValue?
    @State private var issue: String?
    @State private var busy = false
    @State private var exporting = false
    @State private var exportDocument: RovaiMonitoringDocument?
    private var filter: JSONValue {var fields: [String: JSONValue] = ["range": .string(range)];if !runtime.isEmpty {fields["runtimeKind"] = .string(runtime)};if !provider.isEmpty {fields["providerKey"] = .string(provider)};if !selectedModel.isEmpty {fields["modelKey"] = .string(selectedModel)};if !costKind.isEmpty {fields["costKind"] = .string(costKind)};return .object(fields)}
    private var visible: JSONValue? {loadedFilter == filter ? snapshot : nil}
    var body: some View {
        Form {
            Section("范围与筛选") {
                Picker("范围", selection: $range) {Text("过去24小时").tag("24h");Text("过去7天").tag("7d");Text("过去30天").tag("30d")}
                filterPicker("智能体", selection: $runtime, choices: choices("byRuntime", key: "runtimeKind", selected: runtime))
                filterPicker("Provider", selection: $provider, choices: choices("byModel", key: "providerKey", selected: provider))
                filterPicker("模型", selection: $selectedModel, choices: choices("byModel", key: "modelKey", selected: selectedModel))
                filterPicker("成本", selection: $costKind, choices: ["model_call", "turn", "run", "session"])
            }
            if let issue {Section {Text(issue).foregroundStyle(Theme.warning);if let value = visible {Text("显示截至\(value["range"]?.text("to") ?? "")的数据。").font(.caption).foregroundStyle(Theme.textSecondary)};Button("重试") {Task {await load()}}.disabled(busy)}}
            if let value = visible {
                Section("用量") {
                    Text("\(value["range"]?.text("from") ?? "") — \(value["range"]?.text("to") ?? "")").font(.caption).foregroundStyle(Theme.textSecondary)
                    metricRows(value["summary"])
                    LabeledContent("最佳可用成本", value: monitoringMoney(value["summary"]?["cost"]?["run"]))
                }
                Section("用量趋势") {RovaiMonitoringTrend(points: value["trend"]?.rvArray ?? [])}
                Section("采集覆盖率") {
                    ForEach(monitoringCoverageMetrics, id: \.key) { metric in
                        monitoringCoverageRow(metric, value: value)
                    }
                } footer: {Text("仅包含已收到的观测。未知不等于零，正常结束也不代表统计完整。")}
                breakdown("按智能体", rows: value["byRuntime"]?.rvArray ?? [])
                breakdown("按模型", rows: value["byModel"]?.rvArray ?? [])
                if let cost = value["summary"]?["cost"], cost != .null {Section("Provider成本对账") {
                    LabeledContent("Run最佳可用成本", value: monitoringMoney(cost["run"]))
                    LabeledContent("Provider对账成本", value: monitoringMoney(cost["reconciliation"]))
                    LabeledContent("差额（Provider − Run）", value: monitoringMoney(cost["difference"]))
                    LabeledContent("对账截止", value: cost["latestReconciledAt"]?.stringValue ?? "—")
                } footer: {Text("Provider聚合账单与Run成本分开保存，不分摊为单次运行费用。")}}
                Section {Text("采集开始 · \(value["collection"]?.text("startedAt") ?? "")").font(.caption).foregroundStyle(Theme.textSecondary)}
            } else {Section {if busy {ProgressView("正在汇总已保存的用量…")} else {Text("暂无可显示的数据").foregroundStyle(Theme.textSecondary)}}}
        }.navigationTitle("运行监控").navigationBarTitleDisplayMode(.inline)
        .toolbar {
            Button {Task {await load()}} label: {Image(systemName: "arrow.clockwise")}.disabled(busy).accessibilityLabel("刷新运行监控")
            Button {exportSnapshot()} label: {Image(systemName: "square.and.arrow.up")}.disabled(visible == nil || exporting).accessibilityLabel("导出当前快照")
        }
        .task(id: MonitoringRequestKey(filter: filter, active: scenePhase == .active, generation: model.connectionGeneration)) {
            guard scenePhase == .active else {return}
            while !Task.isCancelled {await load();do {try await Task.sleep(for: .seconds(12))} catch {return}}
        }
        .fileExporter(isPresented: $exporting, document: exportDocument, contentType: .json, defaultFilename: "Codeg-Rovai-usage") {result in if case .failure(let error) = result {issue = error.localizedDescription}}
    }
    private func choices(_ group: String, key: String, selected: String) -> [String] {Array(Set((snapshot?[group]?.rvArray ?? []).map {$0.text(key)} + [selected])).filter {!$0.isEmpty}.sorted()}
    private func filterPicker(_ title: String, selection: Binding<String>, choices: [String]) -> some View {Picker(title, selection: selection) {Text("全部").tag("");ForEach(choices, id: \.self) {Text($0).tag($0)}}}
    private func monitoringCoverageRow(_ metric: MonitoringCoverageMetric, value: JSONValue) -> some View {
        let coverage = value["coverage"]?[metric.key]
        let observedRuns = coverage?["observedRuns"]?.rvInt ?? 0
        let eligibleRuns = coverage?["eligibleRuns"]?.rvInt ?? 0
        return LabeledContent(metric.title, value: "\(observedRuns)/\(eligibleRuns)")
    }
    private func metricRows(_ value: JSONValue?) -> some View {ForEach(monitoringMetrics, id: \.0) {key, title in LabeledContent(title, value: monitoringNumber(value?[key], percent: ["cacheReadShare", "requestCacheHitRate"].contains(key)))}}
    private func breakdown(_ title: String, rows: [JSONValue]) -> some View {
        Section(title) {ForEach(Array(rows.enumerated()), id: \.offset) {_, row in DisclosureGroup {
            metricRows(row);Text(monitoringMoney(row["cost"]));LabeledContent("覆盖率", value: "\(row["coverage"]?["observedRuns"]?.rvInt ?? 0)/\(row["coverage"]?["eligibleRuns"]?.rvInt ?? 0)")
        } label: {Text([row.text("runtimeKind"), row.text("providerKey"), row.text("modelKey")].filter {!$0.isEmpty}.joined(separator: " · "))}}}
    }
    private func load() async {
        guard !busy, scenePhase == .active else {return};busy = true;defer {busy = false};let captured = filter, generation = model.connectionGeneration
        do {let value = try await model.query(.monitoringSnapshot, params: captured);try Task.checkCancellation();guard captured == filter, generation == model.connectionGeneration else {return};guard value["schemaVersion"]?.rvInt == 2 else {throw RovaiHostFailure(code: "运行监控格式不受支持")};snapshot = value;loadedFilter = captured;issue = nil} catch is CancellationError {} catch {issue = error.localizedDescription}
    }
    private func exportSnapshot() {guard let snapshot = visible else {return};do {let encoder = JSONEncoder();encoder.outputFormatting = [.sortedKeys, .prettyPrinted];exportDocument = RovaiMonitoringDocument(data: try encoder.encode(JSONValue.object(["filter": filter, "snapshot": snapshot])));exporting = true} catch {issue = error.localizedDescription}}
}

private struct RovaiMonitoringTrend: View {
    let points: [JSONValue]
    @State private var metric = "promptInputTotalTokens"
    @State private var index = 0.0
    private var keys: [(String, String)] {monitoringMetrics.filter {["promptInputTotalTokens", "outputTokens", "cacheReadTokens", "cacheWriteTokens"].contains($0.0)}}
    private var selected: JSONValue? {points.indices.contains(Int(index)) ? points[Int(index)] : nil}
    private func number(_ value: JSONValue?) -> Double? {if case .number(let value) = value {return value};return nil}
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Picker("趋势指标", selection: $metric) {ForEach(keys, id: \.0) {key, title in Text(title).tag(key)}}
            Text("\(selected?.text("bucketStartAt") ?? "暂无时间点") · \(monitoringNumber(selected?[metric]))").font(.caption).foregroundStyle(Theme.textSecondary)
            Canvas {context, size in
                let maximum = max(1, points.compactMap {number($0[metric])}.max() ?? 0)
                var grid = Path();for tick in 0...4 {let y = size.height * CGFloat(tick) / 4;grid.move(to: CGPoint(x: 0, y: y));grid.addLine(to: CGPoint(x: size.width, y: y))};context.stroke(grid, with: .color(Theme.textSecondary.opacity(0.15)), lineWidth: 1)
                var path = Path();var started = false
                for (index, point) in points.enumerated() {
                    guard let value = number(point[metric]) else {started = false;continue}
                    let location = CGPoint(x: points.count > 1 ? size.width * CGFloat(index) / CGFloat(points.count - 1) : size.width / 2, y: size.height * CGFloat(1 - value / maximum))
                    if started {path.addLine(to: location)} else {path.move(to: location)};started = true
                    context.fill(Path(ellipseIn: CGRect(x: location.x - 2, y: location.y - 2, width: 4, height: 4)), with: .color(Theme.accent))
                }
                context.stroke(path, with: .color(Theme.accent), lineWidth: 2)
            }.frame(height: 180).accessibilityLabel("用量趋势；下方滑块可选择时间点，缺失观测不连线。")
            if points.count > 1 {Slider(value: $index, in: 0...Double(points.count - 1), step: 1).accessibilityLabel("趋势时间点")}
            Text("缺失观测保留断点；明确的0显示在基线。").font(.caption).foregroundStyle(Theme.textSecondary)
        }.onChange(of: points) {_, points in index = Double(max(0, points.count - 1))}.onAppear {index = Double(max(0, points.count - 1))}
    }
}

private struct RovaiMonitoringDocument: FileDocument {
    static var readableContentTypes: [UTType] {[.json]}
    var data: Data
    init(data: Data) {self.data = data}
    init(configuration: ReadConfiguration) throws {data = configuration.file.regularFileContents ?? Data()}
    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {FileWrapper(regularFileWithContents: data)}
}
