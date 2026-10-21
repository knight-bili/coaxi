import SwiftUI
import WebKit

func rovaiOfficeFile(_ path: String) -> Bool { ["docx", "xlsx", "pptx"].contains((path as NSString).pathExtension.lowercased()) }
struct RovaiOfficePreview: View {
    let model: RovaiWorkspaceModel
    let threadId: String
    let path: String
    var attachmentLocator: JSONValue? = nil
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase
    @State private var editable = false
    @State private var retry = 0
    @State private var url: URL?
    @State private var showingHold: String?
    @State private var issue: String?
    @State private var generation: UInt64?
    var body: some View {
        NavigationStack {
            VStack(spacing: 12) {
                HStack { if attachmentLocator == nil {Toggle("允许编辑此文件", isOn: $editable)} else {Text("消息附件只读预览")}; Button("重新连接") { retry += 1 } }.padding(.horizontal, 16)
                if let issue { Text(issue).font(.caption).foregroundStyle(Theme.warning).padding(.horizontal, 16) }
                if let url { RovaiOfficeWebView(url: url, issue: $issue).id(url).frame(maxWidth: .infinity, maxHeight: .infinity) }
                else if issue == nil && scenePhase == .active { ProgressView("正在准备文档预览…").frame(maxWidth: .infinity, maxHeight: .infinity) }
            }.navigationTitle((path as NSString).lastPathComponent).navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("关闭") { dismiss() } } }
        }.task(id: "\(threadId):\(path):\(editable):\(retry):\(scenePhase)") { await watch() }
        .onChange(of: model.connectionGeneration) { _, value in if generation != nil && generation != value { dismiss() } }
    }
    private func watch() async {
        url = nil; guard scenePhase == .active else { return }; let lifetime = model.connectionGeneration; generation = lifetime
        let hold = UUID().uuidString.lowercased(); let scope = JSONValue.object(["threadId": .string(threadId)])
        func params(_ operation: String) -> JSONValue {var result: [String: JSONValue] = ["scope": scope, "operation": .string(operation), "holdId": .string(hold), "path": .string(path), "editable": .bool(editable && attachmentLocator == nil)];if let attachmentLocator {result["attachmentLocator"] = attachmentLocator};return .object(result)}
        do { let lease = try await model.query(.officeWatch, params: params("start")); guard lifetime == model.connectionGeneration, lease.text("holdId") == hold else { throw CancellationError() }; let selected = try await model.officePreviewURL(port: lease["port"]?.rvInt ?? 0, cap: lease.text("cap")); try Task.checkCancellation(); url = selected; showingHold = hold; issue = nil
            while !Task.isCancelled { try await Task.sleep(for: .seconds(30)); _ = try await model.query(.officeWatch, params: params("heartbeat")) }
        } catch is CancellationError {} catch { if lifetime == model.connectionGeneration { issue = error.localizedDescription } }
        if lifetime == model.connectionGeneration { _ = try? await model.query(.officeWatch, params: params("stop")) }
        if showingHold == hold { url = nil; showingHold = nil }
    }
}
private struct RovaiOfficeWebView: UIViewRepresentable {
    let url: URL
    @Binding var issue: String?
    func makeCoordinator() -> Coordinator { Coordinator(url: url, failure: { issue = $0 }) }
    func makeUIView(context: Context) -> WKWebView {
        let configuration = WKWebViewConfiguration(); configuration.websiteDataStore = .nonPersistent(); configuration.defaultWebpagePreferences.allowsContentJavaScript = true
        let view = WKWebView(frame: .zero, configuration: configuration); view.navigationDelegate = context.coordinator
        let absolute = url.absoluteString; let components = URLComponents(url: url, resolvingAgainstBaseURL: false); let cap = components?.queryItems?.first { $0.name == "cap" }?.value ?? ""; let prefix = absolute.components(separatedBy: "?")[0]
        let pattern = "^" + NSRegularExpression.escapedPattern(for: prefix) + "[^?#]*[?]([^#]*&)?cap=" + NSRegularExpression.escapedPattern(for: cap) + "(&[^#]*)?$"
        let rules: [[String: Any]] = [["trigger": ["url-filter": ".*"], "action": ["type": "block"]], ["trigger": ["url-filter": pattern], "action": ["type": "ignore-previous-rules"]], ["trigger": ["url-filter": "^about:blank$"], "action": ["type": "ignore-previous-rules"]], ["trigger": ["url-filter": "^data:(image/|font/)"], "action": ["type": "ignore-previous-rules"]]]
        guard let data = try? JSONSerialization.data(withJSONObject: rules), let encoded = String(data: data, encoding: .utf8) else { issue = "文档预览隔离规则无法创建"; return view }
        let coordinator = context.coordinator
        WKContentRuleListStore.default().compileContentRuleList(forIdentifier: "rovai-office-" + UUID().uuidString, encodedContentRuleList: encoded) { [weak view, weak coordinator] list, error in
            guard let view, let coordinator, !coordinator.retired else { return }
            guard let list, error == nil else { coordinator.failure("文档预览隔离规则无法启用"); return }
            view.configuration.userContentController.add(list); coordinator.ruleId = list.identifier
            let escaped = absolute.replacingOccurrences(of: "&", with: "&amp;").replacingOccurrences(of: "\"", with: "&quot;")
            view.loadHTMLString("<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><style>html,body,iframe{width:100%;height:100%;margin:0;border:0}</style><iframe sandbox=\"allow-scripts\" referrerpolicy=\"no-referrer\" src=\"" + escaped + "\"></iframe>", baseURL: nil)
        }
        return view
    }
    func updateUIView(_ view: WKWebView, context: Context) {}
    static func dismantleUIView(_ view: WKWebView, coordinator: Coordinator) { coordinator.retired = true; view.stopLoading(); view.navigationDelegate = nil; view.configuration.userContentController.removeAllContentRuleLists(); if let id = coordinator.ruleId { WKContentRuleListStore.default().removeContentRuleList(forIdentifier: id) { _ in } } }
    final class Coordinator: NSObject, WKNavigationDelegate {
        let url: URL
        let failure: (String) -> Void
        var ruleId: String?
        var retired = false
        init(url: URL, failure: @escaping (String) -> Void) { self.url = url; self.failure = failure }
        func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
            guard !retired, let target = action.request.url else { decisionHandler(.cancel); return }
            if action.targetFrame?.isMainFrame != false { decisionHandler(target.scheme == "about" ? .allow : .cancel); return }
            let allowed = target.scheme == url.scheme && target.host == url.host && target.port == url.port && target.path.hasPrefix(url.path) && URLComponents(url: target, resolvingAgainstBaseURL: false)?.queryItems?.contains(where: { $0.name == "cap" && $0.value == URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems?.first(where: { $0.name == "cap" })?.value }) == true
            decisionHandler(allowed ? .allow : .cancel)
        }
        func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) { if !retired { failure("文档预览连接失败，请重新连接。") } }
    }
}
