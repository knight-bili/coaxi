import SwiftUI

struct RovaiWorkspaceRoot: View {
    let store: ServerStore
    @Binding var selectedServerID: UUID?
    @Binding var incomingLink: RovaiDeepLink?
    let onManageServers: () -> Void
    @State private var model = RovaiWorkspaceModel()
    @State private var tab = 0
    @State private var create: String?
    @State private var notifications = false
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass
    @State private var columns: NavigationSplitViewVisibility = .all
    @State private var detailRoute: RovaiRoute?
    @State private var compactPaths: [Int: [RovaiRoute]] = [:]
    @State private var linkIssue: String?
    @State private var linkSettings: String?
    @State private var memberEditor: MemberEditorSelection?
    private struct MemberEditorSelection: Identifiable { let value: JSONValue?; let id = UUID() }
    private var profile: ServerProfile? { store.servers.first { $0.id == selectedServerID } ?? store.servers.first }
    var body: some View {
        Group { if horizontalSizeClass == .regular { splitShell } else { compactShell } }
        .task(id: "\(profile?.id.uuidString ?? ""):\(profile?.urlString ?? "")") { notifications = false; memberEditor = nil; detailRoute = nil; await model.bind(profile, store: store) }
        .onChange(of: scenePhase, initial: true) { _, phase in model.foreground(phase == .active) }
        .task(id: "\(incomingLink?.id.uuidString ?? ""):\(model.connected):\(model.busy):\(model.pending != nil)") { await openIncomingLink() }
        .sheet(isPresented: Binding(get: { linkSettings != nil }, set: { if !$0 { linkSettings = nil } })) { NavigationStack { linkedSettings.navigationTitle("设置").toolbar { ToolbarItem(placement: .confirmationAction) { Button("关闭") { linkSettings = nil } } } } }
        .sheet(isPresented: $notifications) { RovaiNotificationInbox(model: model) }
        .sheet(item: $memberEditor) { item in RovaiMemberEditor(model: model, member: item.value) }
        .sheet(isPresented: Binding(get: { create != nil }, set: { if !$0 { create = nil } })) { if create == "mission" {RovaiMissionCreator(model: model) {create = nil}} else {RovaiNewConversation(model: model, mission: false) {create = nil}} }
    }
    @ViewBuilder private var linkedSettings: some View {
        switch linkSettings { case "appearance": AppearanceSettingsView(); case "language": LanguageSettingsView(); case "runtime": RovaiRuntimeSettingsView(model: model); case "skills": RovaiSkillSettingsView(model: model); case "quickMessages": RovaiMessageLibrary(model: model); case "mcp": RovaiMcpSettingsView(model: model); case "notifications": RovaiNotificationSettings(model: model); default: EmptyView() }
    }
    private func openIncomingLink() async {
        guard let link = incomingLink, model.connected, !model.busy, model.pending == nil else { return }; let generation = model.connectionGeneration
        do {
            var route: RovaiRoute?; var nextTab: Int?; var pane: String?
            switch link.kind {
            case "thread": route = .thread(link.value); nextTab = 0
            case "mission": let mission = try await model.query(.missionsGet, params: .object(["missionId": .string(link.value)])); guard !mission.text("threadId").isEmpty else { throw RovaiHostFailure(code: "使命对话不存在") }; route = .thread(mission.text("threadId")); nextTab = 1
            case "canvas": let result = try await model.query(.canvasBoardsGet, params: .object(["boardId": .string(link.value)])); guard result["board"] != nil, result["board"] != .null else { throw RovaiHostFailure(code: "画布不存在") }; route = .canvas(link.value); nextTab = 3
            case "tab": switch link.value.lowercased() { case "chats", "threads": nextTab = 0; case "missions": nextTab = 1; case "members": nextTab = 2; case "canvas", "canvases": nextTab = 3; case "settings": nextTab = 4; default: throw RovaiHostFailure(code: "此旧版入口请从当前工作区导航打开") }
            case "settings": nextTab = 4; switch link.value.lowercased() { case "appearance", "language", "mcp", "notifications": pane = link.value.lowercased(); case "agents", "runtime": pane = "runtime"; case "skills", "experts": pane = "skills"; case "quickmessages": pane = "quickMessages"; default: throw RovaiHostFailure(code: "该设置入口尚未映射，请从设置页打开") }
            default: throw RovaiHostFailure(code: "这是原Codeg对象链接，请先导入原会话或从工程列表打开；不会将旧编号当作新对话。")
            }
            guard generation == model.connectionGeneration, incomingLink == link, !Task.isCancelled else { return }
            notifications = false; memberEditor = nil; linkSettings = pane; linkIssue = nil
            if let nextTab { tab = nextTab; compactPaths[nextTab] = route.map { [$0] } ?? [] }; detailRoute = route
            if route == nil { await model.open(nil) }
        } catch is CancellationError { return } catch { if generation == model.connectionGeneration { linkIssue = error.localizedDescription } }
        if incomingLink == link { incomingLink = nil }
    }
    private var compactShell: some View {
        TabView(selection: $tab) {
            tabPage("对话", icon: "message", value: 0) { threadList }
            tabPage("使命", icon: "flag", value: 1) { missionList }
            tabPage("队员", icon: "person.2", value: 2) { memberList }
            tabPage("画布", icon: "map", value: 3) { canvasList }
            tabPage("设置", icon: "gearshape", value: 4) { settings }
        }
    }
    // Same balanced three-column shell and widths as Codeg's original iPad root.
    private var splitShell: some View {
        NavigationSplitView(columnVisibility: $columns) {
            List(selection: $tab) { Label("对话", systemImage: "message").tag(0); Label("使命", systemImage: "flag").tag(1); Label("队员", systemImage: "person.2").tag(2); Label("画布", systemImage: "map").tag(3); Label("设置", systemImage: "gearshape").tag(4) }
                .navigationTitle(profile?.name ?? "Rovai Workbench")
                .navigationSplitViewColumnWidth(min: 220, ideal: 260, max: 320)
                .toolbarTitleMenu { Picker("服务器", selection: $selectedServerID) { ForEach(store.servers) { server in Text(server.name).tag(Optional(server.id)) } }; Divider(); Button("管理服务器", action: onManageServers) }
        } content: {
            NavigationStack {
                VStack(spacing: 0) {
                    if let linkIssue { Text(linkIssue).font(.caption).foregroundStyle(Theme.warning).padding(12) }
                    if let linkIssue { Text(linkIssue).font(.caption).foregroundStyle(Theme.warning).padding(12) }
                if let issue = model.issue { Text(issue).font(.caption).foregroundStyle(Theme.warning).padding(12) }
                    if model.pending != nil { HStack { Text("原操作待核对").font(.caption); Spacer(); Button("核对") { Task { await model.reconcile() } }; Button("重试原操作") { Task { await model.retry() } } }.padding(12).disabled(model.busy) }
                    if model.connected { switch tab { case 0: threadList; case 1: missionList; case 2: memberList; case 3: canvasList; default: settings } }
                    else { ContentUnavailableView { Label("连接工作区", systemImage: "network") } description: { Text(profile?.name ?? "Rovai Workbench") } actions: { Button("连接") { Task { await model.authenticate() } }.disabled(model.busy); Button("管理服务器", action: onManageServers) } }
                }.navigationTitle(["对话", "使命", "队员", "画布", "设置"][tab])
                    .toolbar { ToolbarItemGroup(placement: .topBarTrailing) { Button { notifications = true } label: { Image(systemName: "bell") }.disabled(!model.connected); if tab == 2 { Button { memberEditor = MemberEditorSelection(value: nil) } label: { Image(systemName: "person.badge.plus") }.disabled(!model.connected || model.pending != nil) }; if tab == 0 || tab == 1 { Button { create = tab == 1 ? "mission" : "thread" } label: { Image(systemName: "plus") }.disabled(!model.connected || model.pending != nil) }; Button { Task { await model.refresh() } } label: { Image(systemName: "arrow.clockwise") }.disabled(!model.connected) } }
            }.navigationSplitViewColumnWidth(min: 340, ideal: 400, max: 520)
        } detail: {
            NavigationStack {
                if model.connected, let detailRoute { switch detailRoute { case .thread(let id): RovaiThreadDetail(model: model, id: id).id(id); case .canvas(let id): RovaiCanvasView(model: model, id: id, onOpenThread: {detailRoute = .thread($0)}).id(id) } }
                else { ContentUnavailableView("选择对话或画布", systemImage: "sidebar.left", description: Text("从列表打开内容，在此查看并继续工作。")) }
            }
        }.navigationSplitViewStyle(.balanced)
    }
    @ViewBuilder private func routeLink<Content: View>(_ route: RovaiRoute, @ViewBuilder label: () -> Content) -> some View {
        if horizontalSizeClass == .regular { Button { detailRoute = route } label: { label() }.buttonStyle(.plain) }
        else { NavigationLink(value: route) { label() } }
    }
    private func tabPage<Content: View>(_ title: String, icon: String, value: Int, @ViewBuilder content: () -> Content) -> some View {
        NavigationStack(path: Binding(get: { compactPaths[value, default: []] }, set: { compactPaths[value] = $0 })) {
            VStack(spacing: 0) {
                if let issue = model.issue { Text(issue).font(.caption).foregroundStyle(Theme.warning).frame(maxWidth: .infinity, alignment: .leading).padding(12) }
                if model.pending != nil { HStack { Text("原操作待核对").font(.caption); Spacer(); Button("核对") { Task { await model.reconcile() } }; Button("重试原操作") { Task { await model.retry() } } }.padding(12).disabled(model.busy) }
                if model.connected { content() } else { ContentUnavailableView { Label("连接工作区", systemImage: "network") } description: { Text(profile?.name ?? "Rovai Workbench") } actions: { Button("连接") { Task { await model.authenticate() } }.buttonStyle(.borderedProminent).disabled(model.busy); Button("管理服务器", action: onManageServers) } }
            }
            .navigationTitle(title)
            .toolbar { ToolbarItem(placement: .topBarLeading) { Menu { ForEach(store.servers) { profile in Button(profile.name) { selectedServerID = profile.id } }; Divider(); Button("管理服务器", action: onManageServers) } label: { Label(profile?.name ?? "工作区", systemImage: "server.rack") } }; ToolbarItemGroup(placement: .topBarTrailing) { Button { notifications = true } label: { Image(systemName: "bell") }.disabled(!model.connected); if value == 2 { Button { memberEditor = MemberEditorSelection(value: nil) } label: { Image(systemName: "person.badge.plus") }.disabled(!model.connected || model.pending != nil) }; if value == 0 || value == 1 { Button { create = value == 1 ? "mission" : "thread" } label: { Image(systemName: "plus") }.disabled(!model.connected || model.pending != nil) }; Button { Task { await model.refresh() } } label: { Image(systemName: "arrow.clockwise") }.disabled(!model.connected) } }
            .navigationDestination(for: RovaiRoute.self) { route in switch route { case .thread(let id): RovaiThreadDetail(model: model, id: id); case .canvas(let id): RovaiCanvasView(model: model, id: id, onOpenThread: {compactPaths[3, default: []].append(.thread($0))}) } }
        }.tabItem { Label(title, systemImage: icon).environment(\.symbolVariants, .none) }.tag(value)
    }
    private var threadList: some View {
        List {
            Section("快速对话") { ForEach((model.navigation["quickChat"]?["recentThreads"]?.rvArray ?? []).map { RovaiRow(value: $0) }) { row in threadLink(row.value) } }
            ForEach((model.navigation["projects"]?.rvArray ?? []).map { RovaiRow(value: $0) }, id: \.value) { project in Section(project.value.text("name")) { ForEach((project.value["recentThreads"]?.rvArray ?? []).map { RovaiRow(value: $0) }) { row in threadLink(row.value) }; RovaiMoreThreads(model: model, path: project.value.text("projectPath"), total: project.value["totalCount"]?.rvInt ?? 0, displayed: Set((project.value["recentThreads"]?.rvArray ?? []).map { $0.text("id") }), onSelect: horizontalSizeClass == .regular ? { detailRoute = .thread($0) } : nil) } }
        }.scrollContentBackground(.hidden).refreshable { await model.refresh() }
    }
    private func threadLink(_ value: JSONValue) -> some View { routeLink(.thread(value.text("id"))) { VStack(alignment: .leading, spacing: 6) { Text(value.text("title")).font(.headline); if let lead = value["defaultLead"] { Text(lead.text("displayName")).font(.caption).foregroundStyle(Theme.textSecondary) } } }.listRowBackground(Color.clear) }
    private var missionList: some View {RovaiMissionBoard(model: model) {id in if horizontalSizeClass == .regular {detailRoute = .thread(id)} else {compactPaths.append(.thread(id))}}}
    private var memberList: some View {
        List { ForEach(model.members.filter { $0.text("presence") != "removed" }.map { RovaiRow(value: $0) }) { row in
            GlassRow { VStack(alignment: .leading, spacing: 12) { Button { memberEditor = MemberEditorSelection(value: row.value) } label: { VStack(alignment: .leading, spacing: 6) { Text(row.value.text("displayName")).font(.headline); Text(row.value.text("teamRole")).font(.subheadline).foregroundStyle(Theme.textSecondary); Text(row.value.text("professionalResponsibilities")); Text(row.value["runtimeReadiness"]?.text("status") ?? "").font(.caption).foregroundStyle(Theme.accent) } }.buttonStyle(.plain); RovaiMemberLifecycle(model: model, member: row.value) } }.listRowBackground(Color.clear).listRowSeparator(.hidden)
        } }.scrollContentBackground(.hidden)
    }
    private var canvasList: some View {RovaiCanvasBoards(model: model) {id in
        if horizontalSizeClass == .regular {detailRoute = .canvas(id)} else {compactPaths[3, default: []].append(.canvas(id))}
    }}
    private var settings: some View { List { NavigationLink {RovaiDiagnostics(model: model)} label: {Label("诊断中心", systemImage: "stethoscope")}; NavigationLink {RovaiMonitoring(model: model)} label: {Label("运行监控", systemImage: "chart.xyaxis.line")}; NavigationLink {RovaiWebdavSettings(model: model)} label: {Label("主机WebDAV", systemImage: "externaldrive.badge.icloud")}; NavigationLink { RovaiMemorySettings(model: model) } label: { Label("记忆", systemImage: "brain") }; NavigationLink { RovaiAutomationSettings(model: model) { id in detailRoute = .thread(id); tab = 0; compactPaths[0] = [.thread(id)] } } label: { Label("定时任务", systemImage: "calendar.badge.clock") }; NavigationLink { RovaiMessageLibrary(model: model) } label: { Label("快捷消息和标签", systemImage: "text.bubble") }; NavigationLink { RovaiNotificationSettings(model: model) } label: { Label("提醒", systemImage: "bell") }; NavigationLink { RovaiRuntimeSettingsView(model: model) } label: { Label("智能体、模型和权限", systemImage: "cpu") }; NavigationLink { AppearanceSettingsView() } label: { Label("外观", systemImage: "paintpalette") }; NavigationLink { LanguageSettingsView() } label: { Label("界面语言", systemImage: "globe") }; NavigationLink { RovaiMcpSettingsView(model: model) } label: { Label("MCP连接", systemImage: "puzzlepiece.extension") }; NavigationLink { RovaiSkillSettingsView(model: model) } label: { Label("技能与专家库", systemImage: "graduationcap") }; Button("管理服务器", action: onManageServers); if let profile { LabeledContent("当前服务器", value: profile.displayHost) }; Button("退出当前连接", role: .destructive) { Task { await model.logout() } } }.scrollContentBackground(.hidden) }
}
enum RovaiRoute: Hashable { case thread(String), canvas(String) }

private struct RovaiMoreThreads: View {
    let model: RovaiWorkspaceModel; let path: String; let total: Int; let displayed: Set<String>
    var onSelect: ((String) -> Void)? = nil
    @State private var rows: [JSONValue] = []; @State private var offset: Int? = 0; @State private var busy = false; @State private var issue: String?
    var body: some View { ForEach(rows.filter { !displayed.contains($0.text("id")) }.map { RovaiRow(value: $0) }) { row in if let onSelect { Button(row.value.text("title")) { onSelect(row.id) } } else { NavigationLink(row.value.text("title"), value: RovaiRoute.thread(row.id)) } }; if offset != nil && total > displayed.count { Button(busy ? "正在读取…" : "更多对话") { Task { busy = true; defer { busy = false }; do { let result = try await model.query(.navigationGroupThreads, params: .object(["projectPath": .string(path), "offset": .number(Double(offset ?? 0)), "limit": .number(50)])); rows += result["threads"]?.rvArray ?? []; if result["nextOffset"] == .null { offset = nil } else { offset = result["nextOffset"]?.rvInt } } catch { issue = error.localizedDescription } } }.disabled(busy) }; if let issue { Text(issue).font(.caption).foregroundStyle(Theme.warning) } }
}
private struct RovaiThreadDetail: View {
    let model: RovaiWorkspaceModel; let id: String
    @State private var importedHistory = false
    @State private var messageHistory = false
    @State private var engineering = false
    @State private var privateChat = false
    @State private var library = false
    @State private var text = ""; @State private var recipients = Set<String>(); @State private var reply: JSONValue?; @State private var quotes: [JSONValue] = []; @State private var removedQuote: (Int, JSONValue)?
    @State private var bodyReady = false; @State private var inputRevision = 0; @State private var inputIssue: String?
    @State private var bodyGeneration: UInt64?
    private var owner: String { "public:" + id }
    private var inputLocked: Bool { !bodyReady || bodyGeneration != model.connectionGeneration || model.pending != nil || model.busy }
    var body: some View {
        VStack(spacing: 0) {
            ScrollView { LazyVStack(alignment: .leading, spacing: 14) { RovaiPublicAttachments(model: model, threadId: id); RovaiThreadControls(model: model, threadId: id); ForEach((model.thread?["messages"]?.rvArray ?? []).map { RovaiRow(value: $0) }) { row in GlassRow(isSelected: row.value.text("authorType") == "user") { VStack(alignment: .leading, spacing: 8) { Text(row.value.text("authorDisplayName").isEmpty ? row.value.text("authorType") == "user" ? "你" : row.value.text("authorId") : row.value.text("authorDisplayName")).font(.caption.weight(.semibold)).foregroundStyle(Theme.textSecondary); RovaiMessageBody(model: model, threadId: id, message: row.value); RovaiRuntimeImageGallery(model: model, threadId: id, groups: rovaiImageGroups(model.thread, message: row.value)); RovaiMessageActions(model: model, threadId: id, message: row.value, enabled: !inputLocked, onQuote: {quote in guard !inputLocked else {return false};quotes.append(quote);inputRevision += 1;persistPublicBody();return true}) {chosen in reply = chosen; let author = chosen.text("authorId"); if (model.thread?["members"]?.rvArray ?? []).contains(where: {$0.text("agentId") == author}) {recipients.insert(author)}; inputRevision += 1; persistPublicBody()} } } }; ForEach(rovaiOrphanImages(model.thread), id: \.self) { group in GlassRow { RovaiRuntimeImageGallery(model: model, threadId: id, groups: [group]) } }; ForEach((model.thread?["agentRuns"]?.rvArray ?? []).filter { ["running", "queued", "waiting_approval", "waiting_user"].contains($0.text("status")) }.map { RovaiRow(value: $0) }) { row in HStack { Text(row.value.text("agentId")).font(.caption); Spacer(); Button("停止") { Task { await model.command(.agentRunsCancel, value: .object(["threadId": .string(id), "agentRunId": .string(row.id), "expectedVersion": row.value["version"] ?? .number(0)])) } }.disabled(model.pending != nil) } }.padding(16) }
            RovaiComposerQuotes(quotes: quotes, enabled: !inputLocked) {quoteId in if let index = quotes.firstIndex(where: {$0.text("quoteId") == quoteId}) {removedQuote = (index, quotes.remove(at: index));inputRevision += 1;persistPublicBody()}}.padding(.horizontal, 16)
            if let removedQuote {Button("撤销移除引用") {quotes.insert(removedQuote.1, at: min(removedQuote.0, quotes.count));self.removedQuote = nil;inputRevision += 1;persistPublicBody()}.font(.caption).disabled(inputLocked)}
            if let reply {RovaiReplyPreview(reply: reply, enabled: !inputLocked) {self.reply = nil; inputRevision += 1; persistPublicBody()}.padding(.horizontal, 16)}
            ScrollView(.horizontal, showsIndicators: false) { HStack {
                ForEach((model.thread?["members"]?.rvArray ?? []).map { RovaiRow(value: $0) }) { row in
                    Button { changeRecipient(row.id) } label: { Text(row.value.text("displayName")).font(.caption).padding(.horizontal, 12).padding(.vertical, 8).background(Capsule().fill(recipients.contains(row.id) ? Theme.accent.opacity(0.22) : Theme.accent.opacity(0.06))) }.disabled(inputLocked)
                }
                ForEach(recipients.subtracting(Set((model.thread?["members"]?.rvArray ?? []).map { $0.text("agentId") })).sorted(), id: \.self) { recipient in Button { changeRecipient(recipient) } label: { Label("已离开的收件人", systemImage: "xmark.circle").font(.caption) }.disabled(inputLocked).accessibilityHint(recipient) }
            }.padding(.horizontal, 16) }
            if let inputIssue { Text(inputIssue).font(.caption).foregroundStyle(Theme.warning).padding(.horizontal, 16) }
            HStack(alignment: .bottom, spacing: 10) {
                TextField("发送消息…", text: Binding(get: { text }, set: { value in guard !inputLocked, value != text else { return }; text = value; inputRevision += 1; persistPublicBody() }), axis: .vertical).lineLimit(1...6).padding(12).background(RoundedRectangle(cornerRadius: 18).fill(Theme.bgElevated)).disabled(inputLocked)
                Button { sendPublicBody() } label: { Image(systemName: "paperplane").font(.title3).padding(12) }.disabled(inputLocked || inputIssue != nil || text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !model.publicSourcesReady)
            }.padding(12)
        }.navigationTitle(model.thread?["thread"]?.text("title") ?? "对话")
        .toolbar { ToolbarItem(placement: .topBarTrailing) { Button { library = true } label: { Image(systemName: "text.bubble") }.disabled(inputLocked); Button { privateChat = true } label: { Image(systemName: "person.bubble") }; Button {messageHistory = true} label: {Image(systemName: "magnifyingglass")}.accessibilityLabel("消息记录和搜索"); Button { importedHistory = true } label: { Image(systemName: "clock.arrow.circlepath") }.accessibilityLabel("已导入会话"); Button { engineering = true } label: { Image(systemName: "folder") } } }
        .sheet(isPresented: $library) { NavigationStack { RovaiMessageLibrary(model: model, threadId: id) { content in
            guard !inputLocked else { return false }; text = text.isEmpty ? content : text + "\n" + content; inputRevision += 1; persistPublicBody(); library = false; return true
        }.toolbar { ToolbarItem(placement: .cancellationAction) { Button("返回输入") { library = false } } } } }
        .sheet(isPresented: $privateChat) { RovaiPrivateConversationView(model: model, threadId: id) }
        .sheet(isPresented: $messageHistory) {RovaiThreadHistory(model: model, threadId: id)}
        .sheet(isPresented: $importedHistory) {NavigationStack {RovaiHistoryLibrary(model: model, threadId: id).toolbar {ToolbarItem(placement: .cancellationAction) {Button("返回对话") {importedHistory = false}}}}}
        .sheet(isPresented: $engineering) { RovaiEngineeringView(model: model, threadId: id) }
        .task(id: id) {
            bodyReady = false; let lifetime = model.connectionGeneration; await model.open(id)
            guard lifetime == model.connectionGeneration, !Task.isCancelled else { return }
            do { try restorePublicBody(model.readBody(owner)); bodyGeneration = lifetime; bodyReady = true; inputIssue = nil } catch { inputIssue = error.localizedDescription }
        }
        .onDisappear { persistPublicBody(); if model.threadId == id { Task { await model.open(nil) } } }
        .onChange(of: model.lastResult) { _, result in
            guard bodyReady, bodyGeneration == model.connectionGeneration, model.lastOperation == .threadMessagesSend, model.lastInputOwner == owner,
                  ["applied", "accepted"].contains(result?["commandResult"]?.text("status") ?? result?.text("status") ?? ""),
                  let document = try? model.publicBody(text, recipients: recipients, revision: inputRevision, reply: reply, quotes: quotes), document == model.lastInputBody else { return }
            do { try restorePublicBody(model.readBody(owner)); inputIssue = nil } catch { inputIssue = error.localizedDescription }
        }
    }
    private func restorePublicBody(_ raw: String) throws {
        if raw.isEmpty { text = ""; recipients = []; reply = nil; quotes = []; removedQuote = nil; inputRevision += 1; return }
        guard let document = JSONValue.parse(raw), document["version"]?.rvInt == 1 else { throw RovaiHostFailure(code: "草稿格式无法读取，原内容仍保留。") }
        text = document.text("text"); recipients = Set((document["recipients"]?.rvArray ?? []).compactMap(\.stringValue)); inputRevision = document["revision"]?.rvInt ?? 0; reply = document["reply"] == .null ? nil : document["reply"]; quotes = document["quotes"]?.rvArray ?? []
    }
    private func changeRecipient(_ id: String) { guard !inputLocked else { return }; if recipients.contains(id) { recipients.remove(id) } else { recipients.insert(id) }; inputRevision += 1; persistPublicBody() }
    private func persistPublicBody() {
        guard bodyReady, bodyGeneration == model.connectionGeneration else { return }
        do {
            let document = try model.publicBody(text, recipients: recipients, revision: inputRevision, reply: reply, quotes: quotes)
            let receipt = model.lastResult?["commandResult"] ?? model.lastResult
            if model.lastOperation == .threadMessagesSend, model.lastInputOwner == owner, model.lastInputBody == document, ["applied", "accepted"].contains(receipt?.text("status") ?? "") { return }
            try model.saveBody(owner, body: document); inputIssue = nil
        } catch { inputIssue = error.localizedDescription }
    }
    private func sendPublicBody() {
        guard !inputLocked else { return }; let frozenText = text; let frozenRecipients = recipients; let lifetime = model.connectionGeneration
        do {
            let document = try model.publicBody(frozenText, recipients: frozenRecipients, revision: inputRevision, reply: reply, quotes: quotes); try model.saveBody(owner, body: document); inputIssue = nil
            Task { guard lifetime == model.connectionGeneration else { return }; await model.send(frozenText, recipients: frozenRecipients, document: document, expectedThreadId: id) }
        } catch { inputIssue = error.localizedDescription }
    }
}
private struct RovaiNewConversation: View {
    let model: RovaiWorkspaceModel; let mission: Bool; let onClose: () -> Void
    @State private var name = ""; @State private var detail = ""; @State private var path = ""; @State private var ids = Set<String>(); @State private var lead = ""; @State private var coordinated = false
    var body: some View { NavigationStack { Form { Section("内容") { TextField("名称", text: $name); if mission { TextField("描述", text: $detail, axis: .vertical).lineLimit(3...8) } }; Section("工作目录") { Picker("工程", selection: $path) { Text("快速对话").tag(""); ForEach((model.navigation["projects"]?.rvArray ?? []).map { RovaiRow(value: $0) }, id: \.value) { row in Text(row.value.text("name")).tag(row.value.text("projectPath")) } } }; Section("队员与负责人") { ForEach(model.members.filter { $0.text("presence") == "present" }.map { RovaiRow(value: $0) }) { row in Toggle(row.value.text("displayName"), isOn: Binding(get: { ids.contains(row.id) }, set: { selected in if selected { ids.insert(row.id) } else { ids.remove(row.id) }; if !ids.contains(lead) { lead = ids.sorted().first ?? "" } })) }; Picker("负责人", selection: $lead) { ForEach(model.members.filter { ids.contains($0.text("agentId")) }.map { RovaiRow(value: $0) }) { row in Text(row.value.text("displayName")).tag(row.id) } }; if !mission { Toggle("由负责人协调", isOn: $coordinated) } } }.navigationTitle(mission ? "新建使命" : "新建对话").toolbar { ToolbarItem(placement: .cancellationAction) { Button("取消", action: onClose) }; ToolbarItem(placement: .confirmationAction) { Button("创建") { Task { if mission { await model.command(.missionsCreate, value: .object(["title": .string(name), "description": .string(detail), "projectPath": .string(path), "projectBindingKind": .string(path.isEmpty ? "quick_chat" : "directory"), "memberAgentIds": .array(ids.sorted().map(JSONValue.string)), "defaultLeadAgentId": .string(lead), "tags": .array([])])) } else { await model.execute(.threadsCreate, params: .object(["commandId": .string(UUID().uuidString.lowercased()), "name": .string(name), "workspace": path.isEmpty ? .null : .object(["projectPath": .string(path)]), "memberAgentIds": .array(ids.sorted().map(JSONValue.string)), "defaultLeadAgentId": .string(lead), "collaborationMode": .string(coordinated ? "lead_coordinated" : "peer"), "activationState": .string("active")])) }; onClose() } }.disabled(ids.isEmpty || !ids.contains(lead) || mission && name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || model.pending != nil) } } } }
}
