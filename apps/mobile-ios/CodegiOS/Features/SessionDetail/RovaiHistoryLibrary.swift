import SwiftUI
import CryptoKit
import UniformTypeIdentifiers
import ImageIO

private struct ImportedHistoryDocument {
    let file: URL
    let transcript: JSONValue?
    let bytes: Int
}

@MainActor private func readImportedHistory(_ model: RovaiWorkspaceModel, threadId: String, id: String, progress: (Int, Int) -> Void) async throws -> ImportedHistoryDocument {
    let generation = model.connectionGeneration
    let file = FileManager.default.temporaryDirectory.appendingPathComponent("rovai-import-\(UUID().uuidString).json")
    guard FileManager.default.createFile(atPath: file.path, contents: nil, attributes: [.posixPermissions: 0o600, .protectionKey: FileProtectionType.completeUntilFirstUserAuthentication]) else {throw RovaiHostFailure(code: "无法准备临时记录")}
    var offset = 0, total: Int?, expected: String?, hasher = SHA256()
    do {
        while true {
            try Task.checkCancellation()
            guard generation == model.connectionGeneration else {throw CancellationError()}
            let page = try await model.query(.historyLibrary, params: .object(["scope": .object(["threadId": .string(threadId)]), "operation": .string("read"), "importId": .string(id), "offset": .number(Double(offset))]))
            let size = page["totalBytes"]?.rvInt ?? -1
            guard (0...536870912).contains(size), page["offset"]?.rvInt == offset, total == nil || total == size, expected == nil || expected == page.text("digest"), page.text("bytes").count <= 720896, let data = Data(base64Encoded: page.text("bytes")), offset + data.count <= size else {throw RovaiHostFailure(code: "导入记录快照发生变化")}
            total = size; expected = page.text("digest")
            try await Task.detached {let handle = try FileHandle(forWritingTo: file);defer {try? handle.close()};try handle.seekToEnd();try handle.write(contentsOf: data)}.value
            hasher.update(data: data); offset += data.count;progress(offset, size)
            if page["next"] == .null || page["next"] == nil {guard offset == size else {throw RovaiHostFailure(code: "历史快照读取不完整")};break}
            guard page["next"]?.rvInt == offset, !data.isEmpty else {throw RovaiHostFailure(code: "历史分页无效")}
        }
        guard hasher.finalize().map({String(format: "%02x", $0)}).joined() == expected, generation == model.connectionGeneration else {throw RovaiHostFailure(code: "导入记录摘要不一致或工作区已切换")}
        var transcript: JSONValue?
        if offset <= 32 * 1024 * 1024 {
            transcript = try await Task.detached {try JSONDecoder().decode(JSONValue.self, from: Data(contentsOf: file, options: .mappedIfSafe))}.value
            guard transcript?["schema_version"]?.rvInt == 1 else {throw RovaiHostFailure(code: "未知的原生记录格式")}
        }
        return ImportedHistoryDocument(file: file, transcript: transcript, bytes: offset)
    } catch {try? FileManager.default.removeItem(at: file);throw error}
}

/// Codeg imported sessions and native message rows; imported addresses never execute.
struct RovaiHistoryLibrary: View {
    let model: RovaiWorkspaceModel
    let threadId: String
    @State private var entries: [JSONValue] = []
    @State private var selected: JSONValue?
    @State private var archived = false, busy = false, confirming = false, exporting = false
    @State private var search = "", tags = "", progress = ""
    @State private var issue: String?
    @State private var document: ImportedHistoryDocument?
    @State private var readTask: Task<Void, Never>?
    @State private var turnLimit = 50
    @State private var execution: NativeContinuationSelection?
    private var filtered: [JSONValue] {entries.filter {search.isEmpty || "\($0["summary"]?.text("title") ?? "") \($0.text("sourceName")) \(($0["tags"]?.rvArray ?? []).compactMap(\.stringValue).joined(separator: " "))".localizedCaseInsensitiveContains(search)}}
    var body: some View {
        List {
            if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning)}
            if !progress.isEmpty {Text(progress).font(.caption).foregroundStyle(Theme.textSecondary)}
            if let selected {
                Section {TextField("逗号分隔标签", text: $tags);HStack {Button("保存标签") {Task {await mutate("tags")}};Spacer();Button(selected["archived"]?.rvBool == true ? "恢复记录" : "归档记录") {confirming = true}}.disabled(busy);Button("导出会话记录") {exporting = true}.disabled(busy || document == nil)}
                if let document, document.transcript == nil {Text("记录较大，已完整校验并保存为临时文件。请导出查看完整内容，或在桌面打开。").foregroundStyle(Theme.textSecondary)}
                if let transcript = document?.transcript {
                    let turns = transcript["turns"]?.rvArray ?? []
                    if selected["archived"]?.rvBool != true {Section {Button("从原生会话继续") {execution = NativeContinuationSelection(mode: "continue")};Button("从末尾分叉") {execution = NativeContinuationSelection(mode: "fork_tail")}}.disabled(busy)}
                    ForEach(Array(turns.prefix(turnLimit).enumerated()), id: \.offset) {_, turn in GlassRow(isSelected: turn.text("role") == "user") {VStack(alignment: .leading, spacing: 8) {HStack {Text(turn.text("role") == "user" ? "你" : turn.text("role") == "assistant" ? "队员" : "系统").font(.caption.weight(.semibold));Spacer();Text(turn.text("timestamp")).font(.caption2).foregroundStyle(Theme.textSecondary)};ForEach(Array((turn["blocks"]?.rvArray ?? []).enumerated()), id: \.offset) {_, block in RovaiImportedBlock(block: block)};if selected["archived"]?.rvBool != true, turn.text("role") == "assistant", ["codex", "claude", "deepseek"].contains(selected.text("provider")) {Button("从这条回答分叉") {execution = NativeContinuationSelection(mode: "fork_point", turn: turn.text("id"))}.disabled(busy)}}}.listRowBackground(Color.clear).listRowSeparator(.hidden)}
                    if turns.count > turnLimit {Button("更多消息") {turnLimit += 50}}
                    if (transcript["notes"]?["malformed_records"]?.rvInt ?? 0) > 0 || transcript["notes"]?["incomplete_tail"]?.rvBool == true {Text("原文件含无法解析或尚未写完的记录，已保留可解析部分。").font(.caption).foregroundStyle(Theme.textSecondary)}
                }
            } else {
                Section {Toggle("显示归档", isOn: $archived).disabled(busy)}
                ForEach(filtered.map {RovaiRow(value: $0)}) {row in Button {read(row.value)} label: {GlassRow {VStack(alignment: .leading, spacing: 8) {Text((row.value["summary"]?.text("title") ?? "").isEmpty ? row.value.text("sourceName") : row.value["summary"]!.text("title")).font(.headline);Text("\(row.value.text("provider")) · \(row.value.text("importedAt"))\(row.value["archived"]?.rvBool == true ? " · 已归档" : "")").font(.caption).foregroundStyle(Theme.textSecondary);Text((row.value["tags"]?.rvArray ?? []).compactMap(\.stringValue).joined(separator: " · ")).font(.caption)}}}.buttonStyle(.plain).disabled(busy).listRowBackground(Color.clear).listRowSeparator(.hidden)}
                if entries.isEmpty && !busy {Text("还没有导入记录。可在桌面端选择原生会话文件夹导入。").foregroundStyle(Theme.textSecondary)}
            }
        }.scrollContentBackground(.hidden).navigationTitle("已导入会话")
        .searchable(text: $search, prompt: "搜索标题、来源或标签")
        .toolbar {if selected != nil {ToolbarItem(placement: .topBarTrailing) {Button("返回记录") {readTask?.cancel();selected = nil;release()}.disabled(busy)}}}
        .task(id: "\(threadId):\(model.connectionGeneration):\(archived)") {busy = true;defer {busy = false};do {try await load()} catch is CancellationError {} catch {issue = error.localizedDescription}}
        .onDisappear {readTask?.cancel();release()}
        .confirmationDialog(selected?["archived"]?.rvBool == true ? "恢复这条记录？" : "归档这条记录？", isPresented: $confirming, titleVisibility: .visible) {Button("确认") {Task {await mutate(selected?["archived"]?.rvBool == true ? "restore" : "archive")}};Button("取消", role: .cancel) {}} message: {Text("只调整导入库中的记录，保留来源资料。")}
        .sheet(isPresented: $exporting) {if let file = document?.file {ImportedHistoryExport(file: file) {exporting = false}}}
        .sheet(item: $execution) {selection in if let selected, let transcript = document?.transcript {RovaiNativeContinuation(model: model, entry: selected, transcript: transcript, selection: selection)}}
    }
    private func load() async throws {
        let generation = model.connectionGeneration
        let result = try await model.query(.historyLibrary, params: .object(["scope": .object(["threadId": .string(threadId)]), "operation": .string("list"), "includeArchived": .bool(archived)]))
        guard generation == model.connectionGeneration else {throw CancellationError()};entries = result["entries"]?.rvArray ?? [];if let previous = selected {selected = entries.first {$0.text("id") == previous.text("id")}}
    }
    private func read(_ row: JSONValue) {
        guard !busy else {return};readTask?.cancel();release();selected = row;tags = (row["tags"]?.rvArray ?? []).compactMap(\.stringValue).joined(separator: ", ");turnLimit = 50;busy = true;issue = nil
        readTask = Task {defer {busy = false;progress = ""};do {let result = try await readImportedHistory(model, threadId: threadId, id: row.text("id")) {done, total in progress = "\(done) / \(total)"};if Task.isCancelled {try? FileManager.default.removeItem(at: result.file)} else {document = result}} catch is CancellationError {} catch {issue = error.localizedDescription}}
    }
    private func mutate(_ operation: String) async {
        guard !busy, let row = selected else {return};busy = true;defer {busy = false};issue = nil
        var params: [String: JSONValue] = ["scope": .object(["threadId": .string(threadId)]), "operation": .string(operation), "importId": .string(row.text("id")), "expectedRevision": .string(row.text("revision"))]
        if operation == "tags" {params["tags"] = .array(Array(Set(tags.components(separatedBy: CharacterSet(charactersIn: ",，")).map {$0.trimmingCharacters(in: .whitespacesAndNewlines)}.filter {!$0.isEmpty})).sorted().map(JSONValue.string))}
        do {_ = try await model.query(.historyLibrary, params: .object(params));try await load();if operation != "tags" {selected = nil;release()}} catch is CancellationError {} catch {issue = error.localizedDescription}
    }
    private func release() {if let file = document?.file {try? FileManager.default.removeItem(at: file)};document = nil}
}

private struct RovaiImportedBlock: View {
    let block: JSONValue
    var body: some View {
        switch block.text("type") {
        case "text": MarkdownContent(raw: block.text("text"))
        case "thinking": DisclosureGroup("思考") {MarkdownContent(raw: block.text("text"))}
        case "turn_error": Text(block.text("message")).foregroundStyle(Theme.warning)
        case "tool_use": DisclosureGroup(block.text("tool_name").isEmpty ? "工具调用" : block.text("tool_name")) {Text(block.text("input_preview")).font(.system(.caption, design: .monospaced)).textSelection(.enabled)}
        case "tool_result": DisclosureGroup("工具结果") {Text(block.text("output_preview")).font(.system(.caption, design: .monospaced)).textSelection(.enabled);ForEach(Array((block["images"]?.rvArray ?? []).enumerated()), id: \.offset) {_, image in RovaiImportedImage(value: image)}}
        case "image": RovaiImportedImage(value: block)
        case "image_generation": VStack(alignment: .leading) {if !block.text("revised_prompt").isEmpty {Text(block.text("revised_prompt")).font(.caption)};if let image = block["image"], image != .null {RovaiImportedImage(value: image)}}
        default: EmptyView()
        }
    }
}

private struct RovaiImportedImage: View {
    let value: JSONValue
    @State private var image: UIImage?
    @State private var issue: String?
    @State private var open = false
    var body: some View {Group {if let image {Button {open = true} label: {Image(uiImage: image).resizable().scaledToFit().frame(maxWidth: 180, maxHeight: 180)}} else if let issue {Text(issue).font(.caption).foregroundStyle(Theme.textSecondary)}}.task(id: value) {
        guard !value.text("data").isEmpty else {issue = "原记录只保留图片引用，未读取外部地址";return}
        guard value.text("data").count <= 28 * 1024 * 1024, let data = Data(base64Encoded: value.text("data")) else {issue = "图片较大或格式不支持，请导出记录查看";return}
        let decoded = await Task.detached {() -> UIImage? in guard let source = CGImageSourceCreateWithData(data as CFData, nil), let thumb = CGImageSourceCreateThumbnailAtIndex(source, 0, [kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceCreateThumbnailWithTransform: true, kCGImageSourceThumbnailMaxPixelSize: 2048] as CFDictionary) else {return nil};return UIImage(cgImage: thumb)}.value
        guard !Task.isCancelled else {return};image = decoded;if decoded == nil {issue = "图片格式不支持"}
    }.sheet(isPresented: $open) {NavigationStack {if let image {ScrollView([.horizontal, .vertical]) {Image(uiImage: image).resizable().scaledToFit()}.navigationTitle("原生会话图片").toolbar {Button("关闭") {open = false}}}}}}
}

private struct ImportedHistoryExport: UIViewControllerRepresentable {
    let file: URL;let finished: () -> Void
    func makeCoordinator() -> Coordinator {Coordinator(finished)}
    func makeUIViewController(context: Context) -> UIDocumentPickerViewController {let controller = UIDocumentPickerViewController(forExporting: [file], asCopy: true);controller.delegate = context.coordinator;return controller}
    func updateUIViewController(_ uiViewController: UIDocumentPickerViewController, context: Context) {}
    final class Coordinator: NSObject, UIDocumentPickerDelegate {let finished: () -> Void;init(_ finished: @escaping () -> Void) {self.finished = finished};func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {finished()};func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {finished()}}
}
