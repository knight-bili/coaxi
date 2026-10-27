import SwiftUI
import WebKit
import ImageIO

struct CoaxiMessageArtifacts: View {
    let model: CoaxiWorkspaceModel
    let threadId: String
    let message: JSONValue
    var conversationId: String? = nil
    var child = false
    @State private var source: JSONValue?
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(message["quotes"]?.rvArray ?? [], id: \.self) { quote in GlassRow { VStack(alignment: .leading, spacing: 4) { Text("引用 · " + (quote["authorAtCapture"]?.text("displayName") ?? "")).font(.caption).foregroundStyle(Theme.textSecondary); MarkdownContent(raw: quote.text("text")); if let origin = quote["source"], origin.text("scope") == "camp", origin.text("threadId") == threadId, !origin.text("messageId").isEmpty {CoaxiPublicMessageOrigin(model: model, threadId: threadId, messageId: origin.text("messageId"), label: "查看引用原消息")} } } }
            if conversationId == nil, !message.text("replyToThreadMessageId").isEmpty {CoaxiPublicMessageOrigin(model: model, threadId: threadId, messageId: message.text("replyToThreadMessageId"), label: "查看回复原消息")}
            ForEach((message["attachments"]?.rvArray ?? []).filter { $0.text("previewKind") != "image" }.map { CoaxiRow(value: $0) }) { row in
                Button { source = request(row.id) } label: { Label(row.value.text("displayName"), systemImage: row.value.text("previewKind") == "image" ? "photo" : "doc").font(.subheadline) }.disabled(["missing", "unreadable", "kind_changed"].contains(row.value.text("availability")))
            }
        }.sheet(isPresented: Binding(get: { source != nil }, set: { if !$0 { source = nil } })) { if let source { CoaxiFilePreview(model: model, source: source) } }
    }
    private func request(_ id: String) -> JSONValue {
        var locator: [String: JSONValue] = ["threadId": .string(threadId), "attachmentRefId": .string(id)]
        if let conversationId { locator["owner"] = .string(child ? "delegation_child_message" : "single_chat_message"); locator["conversationId"] = .string(conversationId); locator["conversationMessageId"] = message["id"] ?? .null }
        else { locator["owner"] = .string("message"); locator["messageId"] = message["id"] ?? .null }
        return .object(["kind": .string("attachment"), "threadId": .string(threadId), "locator": .object(locator)])
    }
}
struct CoaxiFilePreview: View {
    let model: CoaxiWorkspaceModel
    let source: JSONValue
    @Environment(\.dismiss) private var dismiss
    @State private var file: JSONValue?
    @State private var page: JSONValue?
    @State private var image: UIImage?
    @State private var html: String?
    @State private var rendered = false
    @State private var office = false
    @State private var issue: String?
    @State private var busy = false
    @State private var previous: [Int] = []
    @State private var generation: UInt64?
    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 12) {
                if source.text("kind") == "attachment", let name = file?.text("fileName"), coaxiOfficeFile(name) {Button("打开Office文档预览") {office = true}}
                if busy { ProgressView().frame(maxWidth: .infinity) }
                if let issue { Text(issue).font(.caption).foregroundStyle(Theme.warning); Button("重试读取") { Task { await read(page?["startOffset"]?.rvInt ?? 0) } }.disabled(busy || file == nil) }
                if let image { ScrollView([.horizontal, .vertical]) { Image(uiImage: image).resizable().scaledToFit().frame(maxWidth: .infinity).clipShape(RoundedRectangle(cornerRadius: 10)) } }
                else if rendered, let html { CoaxiStaticHtml(html: html).frame(maxWidth: .infinity, maxHeight: .infinity) }
                else if let page { ScrollView([.vertical, .horizontal]) { if file?.text("kind") == "markdown" { MarkdownContent(raw: page.text("text")) } else { Text(page.text("text")).font(.system(.subheadline, design: .monospaced)).textSelection(.enabled) } } }
                if page != nil && !rendered { HStack { Button("上一页") { guard let offset = previous.popLast() else { return }; Task { await read(offset) } }.disabled(busy || previous.isEmpty); Spacer(); Button("下一页") { guard let page else { return }; previous.append(page["startOffset"]?.rvInt ?? 0); Task { await read(page["endOffset"]?.rvInt ?? 0) } }.disabled(busy || page?["hasNext"]?.rvBool != true) } }
                Spacer(minLength: 0)
            }.padding(16).navigationTitle(file?.text("fileName") ?? "文件预览").navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("关闭") { dismiss() } }; if file?.text("kind") == "html" { ToolbarItem(placement: .topBarTrailing) { Button(rendered ? "源码" : "查看页面") { if rendered { rendered = false } else { Task { await readHtml() } } }.disabled(busy) } } }
        }.sheet(isPresented: $office) {CoaxiOfficePreview(model: model, threadId: source.text("threadId"), path: file?.text("fileName") ?? "文档", attachmentLocator: source["locator"])}
        .task(id: source) { generation = model.connectionGeneration; await open() }
        .onChange(of: model.connectionGeneration) { _, value in if generation != value { dismiss() } }
        .onDisappear { guard let file, generation == model.connectionGeneration else { return }; Task { _ = try? await model.files("release", params: .object(["handleId": file["handleId"] ?? .null])) } }
    }
    private func input(_ extra: [String: JSONValue] = [:]) -> JSONValue { var fields = extra; fields["handleId"] = file?["handleId"] ?? .null; fields["expectedGeneration"] = file?["contentGeneration"] ?? .null; return .object(fields) }
    private func open() async {
        busy = true; defer { busy = false }
        do { let value = try await model.files("open", params: source); guard value.text("kind") == "file_preview", let selected = value["file"], generation == model.connectionGeneration else { throw CoaxiHostFailure(code: "此来源暂不能预览") }; file = selected
            if selected.text("kind") == "image" { guard (selected["size"]?.rvInt ?? 0) <= 20 * 1024 * 1024 else { throw CoaxiHostFailure(code: "图片超过20MB，请在工程中查看") }; let result = try await model.fileBytes("readBinary", params: input()); let data = result.data; let decoded = await Task.detached(priority: .userInitiated) { () -> UIImage? in guard let source = CGImageSourceCreateWithData(data as CFData, nil), let thumbnail = CGImageSourceCreateThumbnailAtIndex(source, 0, [kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceThumbnailMaxPixelSize: 2048, kCGImageSourceCreateThumbnailWithTransform: true] as CFDictionary) else { return nil }; return UIImage(cgImage: thumbnail) }.value; guard generation == model.connectionGeneration, !Task.isCancelled else { return }; guard let decoded else { throw CoaxiHostFailure(code: "图片无法解码") }; image = decoded }
            else if !coaxiOfficeFile(selected.text("fileName")) { busy = false; await read(0) }
        } catch is CancellationError {} catch { issue = error.localizedDescription }
    }
    private func read(_ offset: Int) async {
        guard !busy, file != nil, generation == model.connectionGeneration else { return }; busy = true; defer { busy = false }
        do { let value = try await model.files("readPage", params: input(["offset": .number(Double(offset)), "maxBytes": .number(128 * 1024)])); guard generation == model.connectionGeneration, !Task.isCancelled else { return }; page = value; issue = nil } catch is CancellationError {} catch { issue = error.localizedDescription }
    }
    private func readHtml() async {
        guard !busy else { return }; busy = true; defer { busy = false }
        do { let value = try await model.files("readHtml", params: input()); guard generation == model.connectionGeneration, !Task.isCancelled else { return }; guard value.text("text").utf8.count <= 2 * 1024 * 1024 else { throw CoaxiHostFailure(code: "页面较大，请使用源码分页查看") }; html = value.text("text"); rendered = true; issue = nil } catch is CancellationError {} catch { issue = error.localizedDescription }
    }
}
private struct CoaxiStaticHtml: UIViewRepresentable {
    let html: String
    func makeCoordinator() -> Coordinator { Coordinator() }
    func makeUIView(context: Context) -> WKWebView { let configuration = WKWebViewConfiguration(); configuration.websiteDataStore = .nonPersistent(); configuration.defaultWebpagePreferences.allowsContentJavaScript = false; let view = WKWebView(frame: .zero, configuration: configuration); view.navigationDelegate = context.coordinator; view.loadHTMLString("<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data:; style-src 'unsafe-inline'\">" + html, baseURL: nil); return view }
    func updateUIView(_ view: WKWebView, context: Context) {}
    static func dismantleUIView(_ view: WKWebView, coordinator: Coordinator) { view.stopLoading(); view.navigationDelegate = nil }
    final class Coordinator: NSObject, WKNavigationDelegate { func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) { decisionHandler(action.navigationType == .other && action.request.url?.scheme == "about" ? .allow : .cancel) } }
}
