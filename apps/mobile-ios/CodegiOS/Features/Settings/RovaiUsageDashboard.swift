import SwiftUI
import UniformTypeIdentifiers

private let activityRanges = [("7d", "近7天"), ("30d", "近30天"), ("90d", "近90天"), ("thisMonth", "本月"), ("thisYear", "今年"), ("all", "全部"), ("custom", "自选日期")]
private func activityNumber(_ value: JSONValue?) -> String {guard case .number(let number) = value, number.isFinite else {return "—"};let format = NumberFormatter();format.numberStyle = .decimal;format.maximumFractionDigits = 0;return format.string(from: NSNumber(value: number)) ?? "—"}
private struct ActivityRequestKey: Hashable {let query: JSONValue;let generation: UInt64;let refresh: Int;let active: Bool}

/// 沿用原生 Form 与监控趋势，Core 分时事实和冻结原生历史始终分开。
struct RovaiUsageDashboard: View {
    let model: RovaiWorkspaceModel
    let history: Bool
    @Environment(\.scenePhase) private var scenePhase
    @State private var range = "30d", bucket = "day", project = "", runtime = "", selectedModel = ""
    @State private var from = Date(), to = Date(), now = Date()
    @State private var snapshot: JSONValue?, loadedQuery: JSONValue?, issue: String?
    @State private var busy = false, exporting = false, refresh = 0
    @State private var exportDocument: ActivityDocument?
    private var calendar: Calendar {var value = Calendar(identifier: .gregorian);value.timeZone = .current;return value}
    private var query: JSONValue {
        let offset = TimeZone.current.secondsFromGMT(for: now) / 60
        var fields: [String: JSONValue] = [:]
        if history {
            let day = calendar.startOfDay(for: now), tomorrow = calendar.date(byAdding: .day, value: 1, to: day) ?? day
            var start: Date?, end: Date? = tomorrow
            switch range {
            case "7d": start = calendar.date(byAdding: .day, value: -7, to: tomorrow)
            case "30d": start = calendar.date(byAdding: .day, value: -30, to: tomorrow)
            case "90d": start = calendar.date(byAdding: .day, value: -90, to: tomorrow)
            case "thisMonth": start = calendar.date(from: calendar.dateComponents([.year, .month], from: now))
            case "thisYear": start = calendar.date(from: calendar.dateComponents([.year], from: now))
            case "all": start = nil;end = nil
            case "custom": start = calendar.startOfDay(for: from);end = calendar.date(byAdding: .day, value: 1, to: calendar.startOfDay(for: to))
            default: break
            }
            let iso = ISO8601DateFormatter()
            var filter: [String: JSONValue] = ["bucket": .string(bucket), "tzOffsetMinutes": .number(Double(offset)), "comparePrevious": .bool(true)]
            if let start {filter["start"] = .string(iso.string(from: start))};if let end {filter["end"] = .string(iso.string(from: end))}
            if !runtime.isEmpty {filter["agentTypes"] = .array([.string(runtime)])};if !selectedModel.isEmpty {filter["models"] = .array([.string(selectedModel)])}
            fields = ["operation": .string("report"), "filter": .object(filter)]
            if !project.isEmpty {fields["projectRoots"] = .array([.string(project)])}
        } else {
            fields = ["range": .string(range), "bucket": .string(bucket), "tzOffsetMinutes": .number(Double(offset))]
            if !project.isEmpty {fields["projectRoot"] = .string(project)};if !runtime.isEmpty {fields["runtimeKind"] = .string(runtime)};if !selectedModel.isEmpty {fields["modelKey"] = .string(selectedModel)}
            if range == "custom" {let format = DateFormatter();format.calendar = calendar;format.locale = Locale(identifier: "en_US_POSIX");format.timeZone = .current;format.dateFormat = "yyyy-MM-dd";fields["start"] = .string(format.string(from: from));fields["end"] = .string(format.string(from: to))}
        }
        return .object(fields)
    }
    private var visible: JSONValue? {loadedQuery == query ? snapshot : nil}
    private var report: JSONValue? {history ? visible?["report"] : visible}
    var body: some View {
        Form {
            Section("范围与筛选") {
                Picker("范围", selection: $range) {ForEach(activityRanges, id: \.0) {key, title in Text(title).tag(key)}}
                if range == "custom" {DatePicker("开始日期", selection: $from, displayedComponents: .date);DatePicker("结束日期", selection: $to, in: from..., displayedComponents: .date)}
                TextField("工程路径 留空全部", text: $project).textInputAutocapitalization(.never).autocorrectionDisabled()
                TextField("智能体 留空全部", text: $runtime).textInputAutocapitalization(.never).autocorrectionDisabled()
                TextField("模型 留空全部", text: $selectedModel).textInputAutocapitalization(.never).autocorrectionDisabled()
                Picker("粒度", selection: $bucket) {Text("按天").tag("day");Text("按周").tag("week");Text("按月").tag("month")}
            }
            if let issue {Section {Text(issue).foregroundStyle(Theme.warning);Button("重试") {refresh += 1}}}
            if let report, let visible {
                Section("已报告用量") {
                    Text(activityNumber(report["totals"]?[history ? "total_tokens" : "knownTotalTokens"])).font(.largeTitle.bold())
                    let metrics = history ? [("input_tokens", "输入 Token"), ("output_tokens", "输出 Token"), ("cache_read_tokens", "缓存读取"), ("cache_creation_tokens", "缓存写入"), ("conversation_count", "会话"), ("turn_count", "用量记录")] : [("inputTokens", "输入 Token"), ("outputTokens", "输出 Token"), ("cacheReadTokens", "缓存读取"), ("cacheWriteTokens", "缓存写入"), ("runs", "运行次数"), ("partialObservations", "未完整报告次数")]
                    ForEach(metrics, id: \.0) {key, title in LabeledContent(title, value: activityNumber(report["totals"]?[key]))}
                }
                Section("用量趋势") {ActivityTrend(points: report["series"]?.rvArray ?? [], history: history)}
                let groups = history ? [("by_folder", "工程排行"), ("by_agent", "智能体排行"), ("by_model", "模型排行")] : [("byProject", "工程排行"), ("byRuntime", "智能体排行"), ("byModel", "模型排行")]
                ForEach(groups, id: \.0) {key, title in Section(title) {ForEach(Array((report[key]?.rvArray ?? []).enumerated()), id: \.offset) {index, row in HStack {Text(String(index + 1)).font(.caption.monospaced()).foregroundStyle(Theme.textSecondary);Text(label(row)).lineLimit(2);Spacer();Text(activityNumber(row[history ? "total_tokens" : "knownTotalTokens"])).font(.caption.monospaced())}}}}
                if history {
                    Section("会话榜") {ForEach(Array((report["top_conversations"]?.rvArray ?? []).enumerated()), id: \.offset) {_, row in VStack(alignment: .leading) {Text(row.text("title").isEmpty ? "未命名会话" : row.text("title"));Text(row.text("agent_type") + " · " + activityNumber(row["total_tokens"])).font(.caption).foregroundStyle(Theme.textSecondary)}}}
                    Section {Text("只统计已导入的冻结快照；不与融合运行数据相加。还有 \(visible["missingIndex"]?.rvArray.count ?? 0) 份未索引，需在桌面同步。整会话回退 \(visible["sessionTotalFallbacks"]?.rvInt ?? 0) 份，已排除同源旧快照 \(visible["supersededSnapshots"]?.rvInt ?? 0) 份。").font(.caption).foregroundStyle(Theme.textSecondary)}
                } else {Section {Text("来源：融合 Core；采集开始 \(visible.text("recordingStartedAt"))；分时精度 \(visible["resolutionSeconds"]?.rvInt ?? 0) 秒，保留 \(visible["retentionDays"]?.rvInt ?? 0) 天。未知不等于零，不代表账单。").font(.caption).foregroundStyle(Theme.textSecondary)}}
            } else if busy {Section {ProgressView("正在读取用量…")}}
        }
        .navigationTitle(history ? "原生历史用量" : "用量看板").navigationBarTitleDisplayMode(.inline)
        .toolbar {Button {now = Date();refresh += 1} label: {Image(systemName: "arrow.clockwise")}.disabled(busy).accessibilityLabel("刷新用量");Button {exportSnapshot()} label: {Image(systemName: "square.and.arrow.up")}.disabled(visible == nil || exporting).accessibilityLabel("导出当前用量")}
        .onChange(of: range) {_, value in bucket = ["all", "thisYear"].contains(value) ? "month" : value == "90d" ? "week" : "day"}
        .task(id: ActivityRequestKey(query: query, generation: model.connectionGeneration, refresh: refresh, active: scenePhase == .active)) {await load()}
        .fileExporter(isPresented: $exporting, document: exportDocument, contentType: .json, defaultFilename: history ? "Codeg-Rovai-native-usage" : "Codeg-Rovai-activity") {if case .failure(let error) = $0 {issue = error.localizedDescription}}
    }
    private func label(_ value: JSONValue) -> String {let label = value.text(history ? "label" : "key");return label == "__other__" ? "其他" : label.isEmpty ? "未记录" : label}
    private func load() async {
        guard scenePhase == .active else {return};let captured = query, generation = model.connectionGeneration
        snapshot = nil;loadedQuery = nil;busy = true;issue = nil;defer {if captured == query, generation == model.connectionGeneration {busy = false}}
        do {try await Task.sleep(for: .milliseconds(250));let value = try await model.query(history ? .historyUsage : .monitoringReport, params: captured);try Task.checkCancellation();guard captured == query, generation == model.connectionGeneration else {return};guard value["schemaVersion"]?.rvInt == 1 else {throw RovaiHostFailure(code: "用量格式不受支持")};snapshot = value;loadedQuery = captured}
        catch is CancellationError {} catch {if captured == query, generation == model.connectionGeneration {issue = error.localizedDescription}}
    }
    private func exportSnapshot() {guard let value = visible else {return};do {let encoder = JSONEncoder();encoder.outputFormatting = [.sortedKeys, .prettyPrinted];exportDocument = ActivityDocument(data: try encoder.encode(JSONValue.object(["query": query, "snapshot": value])));exporting = true} catch {issue = error.localizedDescription}}
}

private struct ActivityTrend: View {
    let points: [JSONValue]
    let history: Bool
    @State private var index = 0.0
    private var totalKey: String {history ? "total_tokens" : "knownTotalTokens"}
    private func number(_ value: JSONValue?) -> Double {guard case .number(let value) = value, value.isFinite else {return 0};return max(0, value)}
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            let selected = points.indices.contains(Int(index)) ? points[Int(index)] : nil
            Text((selected?.text(history ? "bucket_key" : "key") ?? "暂无记录") + " · " + activityNumber(selected?[totalKey])).font(.caption)
            Canvas {context, size in
                let chunkSize = max(1, Int(ceil(Double(points.count) / 48)))
                var values: [(Double, Double)] = []
                for offset in stride(from: 0, to: points.count, by: chunkSize) {let chunk = points[offset..<min(points.count, offset + chunkSize)];let total = chunk.reduce(0) {$0 + number($1[totalKey])};let cache = min(total, chunk.reduce(0) {sum, row in sum + (!history && row["inputTokens"] == .null ? 0 : number(row[history ? "cache_read_tokens" : "cacheReadTokens"]))});values.append((cache, total - cache))}
                let maximum = max(1, values.map {$0.0 + $0.1}.max() ?? 0), width = values.isEmpty ? 0 : size.width / CGFloat(values.count)
                for (index, value) in values.enumerated() {let cache = size.height * value.0 / maximum, fresh = size.height * value.1 / maximum;context.fill(Path(CGRect(x: CGFloat(index) * width, y: size.height - cache, width: max(1, width - 2), height: cache)), with: .color(Theme.accent));context.fill(Path(CGRect(x: CGFloat(index) * width, y: size.height - cache - fresh, width: max(1, width - 2), height: fresh)), with: .color(Theme.textPrimary))}
            }.frame(height: 120).accessibilityLabel("缓存和其他已报告Token趋势，下方滑块选择时间点")
            if points.count > 1 {Slider(value: $index, in: 0...Double(points.count - 1), step: 1)}
            Text("缓存读取 · 其他已报告 Token").font(.caption).foregroundStyle(Theme.textSecondary)
        }.onAppear {index = Double(max(0, points.count - 1))}.onChange(of: points) {_, value in index = Double(max(0, value.count - 1))}
    }
}

private struct ActivityDocument: FileDocument {
    static var readableContentTypes: [UTType] {[.json]}
    let data: Data
    init(data: Data) {self.data = data}
    init(configuration: ReadConfiguration) throws {data = configuration.file.regularFileContents ?? Data()}
    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {FileWrapper(regularFileWithContents: data)}
}
