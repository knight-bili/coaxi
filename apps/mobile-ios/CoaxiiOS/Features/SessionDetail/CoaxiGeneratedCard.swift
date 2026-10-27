import SwiftUI
import WebKit

/// 读取 Core 的真实回复，只在本地资源页面复用原 Coaxi 卡片。
struct CoaxiGeneratedCard: View {
    let model: CoaxiWorkspaceModel;let threadId: String;let message: JSONValue
    var conversationId: String? = nil;var child = false
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.locale) private var locale
    @State private var index = 0
    @State private var card: JSONValue?
    @State private var issue: String?
    @State private var generation: UInt64?
    private var scope: JSONValue {.object(["threadId": .string(threadId), "conversationId": conversationId.map(JSONValue.string) ?? .null, "scopeKind": .string(conversationId == nil ? "public" : child ? "delegation_child" : "single_chat")])}
    var body: some View {
        NavigationStack {VStack(spacing: 8) {
            if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning).padding(16)}
            if let card {
                if (card["cardCount"]?.rvInt ?? 0) > 1 {HStack {Button("上一张") {index -= 1}.disabled(index == 0);Spacer();Text("\(index + 1)/\(card["cardCount"]?.rvInt ?? 0)").font(.caption);Spacer();Button("下一张") {index += 1}.disabled(index + 1 >= (card["cardCount"]?.rvInt ?? 0))}.padding(.horizontal, 16)}
                CoaxiGeneratedCardPage(model: model, card: card, scope: scope, active: scenePhase == .active, dark: colorScheme == .dark, language: locale.identifier, issue: $issue).id(card).frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if issue == nil {ProgressView("正在读取卡片…").frame(maxWidth: .infinity, maxHeight: .infinity)}
        }.navigationTitle("交互卡片").navigationBarTitleDisplayMode(.inline).toolbar {Button("关闭") {dismiss()}}}
        .task(id: index) {
            card = nil;let generation = model.connectionGeneration;self.generation = generation
            do {guard Bundle.main.url(forResource: "CardPreview", withExtension: "bundle") != nil else {throw CoaxiHostFailure(code: "交互卡片资源尚未打包")}
                let value = try await model.query(.feedbackOperation, params: .object(["action": .string("cards.read"), "payload": .object(["scope": scope, "agentId": message["authorId"] ?? .null, "messageId": message["id"] ?? .null, "cardOrdinal": .number(Double(index))])]))
                guard generation == model.connectionGeneration, value["scope"] == scope, value["messageId"] == message["id"], value["agentId"] == message["authorId"] else {throw CancellationError()};card = value;issue = nil
            } catch is CancellationError {} catch {issue = error.localizedDescription}
        }.onChange(of: model.connectionGeneration) {_, value in if generation != nil && generation != value {dismiss()}}
    }
}

private struct CoaxiGeneratedCardPage: UIViewRepresentable {
    let model: CoaxiWorkspaceModel;let card: JSONValue;let scope: JSONValue;let active: Bool;let dark: Bool;let language: String
    @Binding var issue: String?
    func makeCoordinator() -> Coordinator {Coordinator(model: model, card: card, scope: scope, active: active, dark: dark, language: language, failure: {issue = $0})}
    func makeUIView(context: Context) -> WKWebView {
        let config = WKWebViewConfiguration();config.websiteDataStore = .nonPersistent();config.defaultWebpagePreferences.allowsContentJavaScript = true;config.preferences.javaScriptCanOpenWindowsAutomatically = false
        config.userContentController.add(context.coordinator, name: "coaxiCardNative")
        let view = WKWebView(frame: .zero, configuration: config);view.navigationDelegate = context.coordinator;context.coordinator.view = view
        guard let bundle = Bundle.main.url(forResource: "CardPreview", withExtension: "bundle") else {issue = "交互卡片资源尚未打包";return view}
        let page = bundle.appendingPathComponent("index.html");context.coordinator.page = page
        let allowed = "^" + NSRegularExpression.escapedPattern(for: bundle.absoluteString) + "/"
        let rules: [[String: Any]] = [["trigger": ["url-filter": ".*"], "action": ["type": "block"]], ["trigger": ["url-filter": allowed], "action": ["type": "ignore-previous-rules"]], ["trigger": ["url-filter": "^data:(image/|font/)"], "action": ["type": "ignore-previous-rules"]]]
        guard let data = try? JSONSerialization.data(withJSONObject: rules), let encoded = String(data: data, encoding: .utf8) else {issue = "卡片隔离规则无法创建";return view}
        let coordinator = context.coordinator
        WKContentRuleListStore.default().compileContentRuleList(forIdentifier: "coaxi-card-" + UUID().uuidString.lowercased(), encodedContentRuleList: encoded) {[weak coordinator, weak view] list, error in
            guard let coordinator, let view, !coordinator.retired else {return};guard let list, error == nil else {coordinator.failure("卡片隔离规则无法启用");return}
            coordinator.ruleId = list.identifier;view.configuration.userContentController.add(list);view.loadFileURL(page, allowingReadAccessTo: bundle)
        };return view
    }
    func updateUIView(_ view: WKWebView, context: Context) {context.coordinator.active = active;context.coordinator.dark = dark}
    static func dismantleUIView(_ view: WKWebView, coordinator: Coordinator) {
        coordinator.retired = true;coordinator.tasks.values.forEach {$0.cancel()};coordinator.tasks.removeAll();view.stopLoading();view.navigationDelegate = nil;view.configuration.userContentController.removeScriptMessageHandler(forName: "coaxiCardNative");view.configuration.userContentController.removeAllContentRuleLists();if let rule = coordinator.ruleId {WKContentRuleListStore.default().removeContentRuleList(forIdentifier: rule) {_ in}}
    }
    @MainActor final class Coordinator: NSObject, WKNavigationDelegate, WKScriptMessageHandler {
        let model: CoaxiWorkspaceModel;let card: JSONValue;let scope: JSONValue;let generation: UInt64;let channel = UUID().uuidString.lowercased();let failure: (String) -> Void
        var active: Bool;var dark: Bool;let language: String;var retired = false;var page: URL?, ruleId: String?
        weak var view: WKWebView?
        var previews = Set<String>(), tasks: [String: Task<Void, Never>] = [:]
        init(model: CoaxiWorkspaceModel, card: JSONValue, scope: JSONValue, active: Bool, dark: Bool, language: String, failure: @escaping (String) -> Void) {self.model = model;self.card = card;self.scope = scope;self.active = active;self.dark = dark;self.language = language;self.failure = failure;generation = model.connectionGeneration}
        private var live: Bool {!retired && active && generation == model.connectionGeneration && page != nil && view?.url == page}
        func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {decisionHandler(!retired && action.targetFrame?.isMainFrame == true && action.request.url == page && action.navigationType == .other ? .allow : .cancel)}
        func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
            guard live else {return};let input = JSONValue.object(["channel": .string(channel), "card": card, "dark": .bool(dark), "language": .string(language)])
            guard let data = try? JSONEncoder().encode(input), let text = String(data: data, encoding: .utf8) else {failure("卡片来源无法显示");return};webView.evaluateJavaScript("window.CoaxiCardPage?.mount(\(text));") {_, error in if error != nil {self.failure("卡片页面未能加载")}}
        }
        func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
            guard live, message.frameInfo.isMainFrame, message.webView === view, let raw = message.body as? String, raw.utf8.count <= 65536, let input = try? JSONDecoder().decode(JSONValue.self, from: Data(raw.utf8)), input.text("channel") == channel, input["schemaVersion"]?.rvInt == 1 else {return}
            let id = input.text("requestId");guard UUID(uuidString: id) != nil, tasks[id] == nil, tasks.count < 16 else {return}
            tasks[id] = Task {[weak self] in guard let self else {return};defer {self.tasks.removeValue(forKey: id)}
                var result: JSONValue
                do {
                    let action = input.text("action"), payload = input["payload"] ?? .null
                    guard ["targets", "preview", "submit"].contains(action) else {throw CoaxiHostFailure(code: "卡片操作不可用")}
                    if action != "submit", payload["scope"] != self.scope {throw CoaxiHostFailure(code: "卡片归属已变化")}
                    if action == "preview" {guard payload["agentId"] == self.card["agentId"], payload["source"]?.text("kind") == "generated_card", payload["source"]?["origin"]?["messageId"] == self.card["messageId"], payload["source"]?["origin"]?["cardOrdinal"] == self.card["cardOrdinal"] else {throw CoaxiHostFailure(code: "卡片来源已变化")}}
                    if action == "submit", !self.previews.contains(payload.text("previewId")) {throw CoaxiHostFailure(code: "原预览不属于这张卡片")}
                    var value = try await self.model.query(.feedbackOperation, params: .object(["action": .string(action), "payload": payload]));guard self.live else {return}
                    if action == "targets", case .object(var object) = value {object["targets"] = .array((value["targets"]?.rvArray ?? []).filter {$0["agentId"] == self.card["agentId"]});value = .object(object)}
                    if action == "preview" {self.previews.insert(value.text("previewId"))}
                    result = .object(["channel": .string(self.channel), "requestId": .string(id), "ok": .bool(true), "result": value])
                } catch is CancellationError {return} catch {result = .object(["channel": .string(self.channel), "requestId": .string(id), "ok": .bool(false), "error": .string(error.localizedDescription)])}
                guard self.live, let data = try? JSONEncoder().encode(result), let text = String(data: data, encoding: .utf8) else {return};self.view?.evaluateJavaScript("window.CoaxiCardPage?.reply(\(text));", completionHandler: nil)
            }
        }
    }
}
