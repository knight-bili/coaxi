import SwiftUI

struct RovaiWorkspaceRoot: View {
    let store: ServerStore
    @Binding var selectedServerID: UUID?
    let onManageServers: () -> Void
    @State private var model = RovaiWorkspaceModel()
    @State private var tab = 0
    @State private var create: String?
    @Environment(\.scenePhase) private var scenePhase
    private var profile: ServerProfile? { store.servers.first { $0.id == selectedServerID } ?? store.servers.first }
    var body: some View {
        TabView(selection: $tab) {
            tabPage("对话", icon: "message", value: 0) { threadList }
            tabPage("使命", icon: "flag", value: 1) { missionList }
            tabPage("队员", icon: "person.2", value: 2) { memberList }
            tabPage("画布", icon: "map", value: 3) { canvasList }
            tabPage("设置", icon: "gearshape", value: 4) { settings }
        }
        .task(id: "\(profile?.id.uuidString ?? ""):\(profile?.urlString ?? "")") { await model.bind(profile, store: store) }
        .onChange(of: scenePhase, initial: true) { _, phase in model.foreground(phase == .active) }
        .sheet(isPresented: Binding(get: { create != nil }, set: { if !$0 { create = nil } })) { RovaiNewConversation(model: model, mission: create == "mission") { create = nil } }
    }
    private func tabPage<Content: View>(_ title: String, icon: String, value: Int, @ViewBuilder content: () -> Content) -> some View {
        NavigationStack {
            VStack(spacing: 0) {
                if let issue = model.issue { Text(issue).font(.caption).foregroundStyle(Theme.warning).frame(maxWidth: .infinity, alignment: .leading).padding(12) }
                if model.pending != nil { HStack { Text("原操作待核对").font(.caption); Spacer(); Button("核对") { Task { await model.reconcile() } }; Button("重试原操作") { Task { await model.retry() } } }.padding(12).disabled(model.busy) }
                if model.connected { content() } else { ContentUnavailableView { Label("连接工作区", systemImage: "network") } description: { Text(profile?.name ?? "Rovai Workbench") } actions: { Button("连接") { Task { await model.authenticate() } }.buttonStyle(.borderedProminent).disabled(model.busy); Button("管理服务器", action: onManageServers) } }
            }
            .navigationTitle(title)
            .toolbar { ToolbarItem(placement: .topBarLeading) { Menu { ForEach(store.servers) { profile in Button(profile.name) { selectedServerID = profile.id } }; Divider(); Button("管理服务器", action: onManageServers) } label: { Label(profile?.name ?? "工作区", systemImage: "server.rack") } }; ToolbarItemGroup(placement: .topBarTrailing) { if value == 0 || value == 1 { Button { create = value == 1 ? "mission" : "thread" } label: { Image(systemName: "plus") }.disabled(!model.connected || model.pending != nil) }; Button { Task { await model.refresh() } } label: { Image(systemName: "arrow.clockwise") }.disabled(!model.connected) } }
            .navigationDestination(for: RovaiRoute.self) { route in switch route { case .thread(let id): RovaiThreadDetail(model: model, id: id); case .canvas(let id): RovaiCanvasView(model: model, id: id) } }
        }.tabItem { Label(title, systemImage: icon).environment(\.symbolVariants, .none) }.tag(value)
    }
    private var threadList: some View {
        List {
            Section("快速对话") { ForEach((model.navigation["quickChat"]?["recentThreads"]?.rvArray ?? []).map { RovaiRow(value: $0) }) { row in threadLink(row.value) } }
            ForEach((model.navigation["projects"]?.rvArray ?? []).map { RovaiRow(value: $0) }, id: \.value) { project in Section(project.value.text("name")) { ForEach((project.value["recentThreads"]?.rvArray ?? []).map { RovaiRow(value: $0) }) { row in threadLink(row.value) }; RovaiMoreThreads(model: model, path: project.value.text("projectPath"), total: project.value["totalCount"]?.rvInt ?? 0, displayed: Set((project.value["recentThreads"]?.rvArray ?? []).map { $0.text("id") })) } }
        }.scrollContentBackground(.hidden).refreshable { await model.refresh() }
    }
    private func threadLink(_ value: JSONValue) -> some View { NavigationLink(value: RovaiRoute.thread(value.text("id"))) { VStack(alignment: .leading, spacing: 6) { Text(value.text("title")).font(.headline); if let lead = value["defaultLead"] { Text(lead.text("displayName")).font(.caption).foregroundStyle(Theme.textSecondary) } } }.listRowBackground(Color.clear) }
    private var missionList: some View {
        List { ForEach(model.missions.map { RovaiRow(value: $0) }) { row in GlassRow { VStack(alignment: .leading, spacing: 8) { HStack { Text(row.value.text("title")).font(.headline); Spacer(); Text(row.value.text("status")).font(.caption).foregroundStyle(Theme.textSecondary) }; Text(row.value.text("description")).lineLimit(3).foregroundStyle(Theme.textSecondary); HStack { NavigationLink("打开对话", value: RovaiRoute.thread(row.value.text("threadId"))); if row.value["startAvailable"]?.rvBool == true { Button("开始使命") { Task { await model.command(.missionsStart, value: .object(["missionId": .string(row.id)])) } }.disabled(model.pending != nil) } } }.listRowBackground(Color.clear).listRowSeparator(.hidden) } }.scrollContentBackground(.hidden).refreshable { await model.refresh() }
    }
    private var memberList: some View { List { ForEach(model.members.map { RovaiRow(value: $0) }) { row in GlassRow { VStack(alignment: .leading, spacing: 8) { Text(row.value.text("displayName")).font(.headline); Text(row.value.text("teamRole")).font(.subheadline).foregroundStyle(Theme.textSecondary); Text(row.value.text("professionalResponsibilities")); Text(row.value["runtimeReadiness"]?.text("status") ?? "").font(.caption).foregroundStyle(Theme.accent) } }.listRowBackground(Color.clear).listRowSeparator(.hidden) } }.scrollContentBackground(.hidden) }
    private var canvasList: some View { List { ForEach(model.boards.map { RovaiRow(value: $0) }) { row in NavigationLink(value: RovaiRoute.canvas(row.id)) { GlassRow { HStack(spacing: 12) { Image(systemName: "map").font(.title2).foregroundStyle(Theme.textSecondary); VStack(alignment: .leading, spacing: 6) { Text(row.value.text("name").isEmpty ? "未命名画布" : row.value.text("name")).font(.headline); Text(row.value.text("description")).font(.caption).foregroundStyle(Theme.textSecondary) } } } }.listRowBackground(Color.clear).listRowSeparator(.hidden) } }.scrollContentBackground(.hidden) }
    private var settings: some View { List { NavigationLink { RovaiMcpSettingsView(model: model) } label: { Label("MCP连接", systemImage: "puzzlepiece.extension") }; NavigationLink { RovaiSkillSettingsView(model: model) } label: { Label("技能与专家库", systemImage: "graduationcap") }; Button("管理服务器", action: onManageServers); if let profile { LabeledContent("当前服务器", value: profile.displayHost) }; Button("退出当前连接", role: .destructive) { Task { await model.logout() } } }.scrollContentBackground(.hidden) }
}
enum RovaiRoute: Hashable { case thread(String), canvas(String) }

private struct RovaiMoreThreads: View {
    let model: RovaiWorkspaceModel; let path: String; let total: Int; let displayed: Set<String>
    @State private var rows: [JSONValue] = []; @State private var offset: Int? = 0; @State private var busy = false; @State private var issue: String?
    var body: some View { ForEach(rows.filter { !displayed.contains($0.text("id")) }.map { RovaiRow(value: $0) }) { row in NavigationLink(row.value.text("title"), value: RovaiRoute.thread(row.id)) }; if offset != nil && total > displayed.count { Button(busy ? "正在读取…" : "更多对话") { Task { busy = true; defer { busy = false }; do { let result = try await model.query(.navigationGroupThreads, params: .object(["projectPath": .string(path), "offset": .number(Double(offset ?? 0)), "limit": .number(50)])); rows += result["threads"]?.rvArray ?? []; if result["nextOffset"] == .null { offset = nil } else { offset = result["nextOffset"]?.rvInt } } catch { issue = error.localizedDescription } } }.disabled(busy) }; if let issue { Text(issue).font(.caption).foregroundStyle(Theme.warning) } }
}
private struct RovaiThreadDetail: View {
    let model: RovaiWorkspaceModel; let id: String
    @State private var engineering = false
    @State private var privateChat = false
    @State private var text = ""; @State private var recipients = Set<String>(); @State private var submitted: String?
    var body: some View {
        VStack(spacing: 0) {
            ScrollView { LazyVStack(alignment: .leading, spacing: 14) { RovaiThreadControls(model: model, threadId: id); ForEach((model.thread?["messages"]?.rvArray ?? []).map { RovaiRow(value: $0) }) { row in GlassRow(isSelected: row.value.text("authorType") == "user") { VStack(alignment: .leading, spacing: 8) { Text(row.value.text("authorDisplayName").isEmpty ? row.value.text("authorType") == "user" ? "你" : row.value.text("authorId") : row.value.text("authorDisplayName")).font(.caption.weight(.semibold)).foregroundStyle(Theme.textSecondary); MarkdownContent(raw: row.value.text("body")) } } }; ForEach((model.thread?["agentRuns"]?.rvArray ?? []).filter { ["running", "queued", "waiting_approval", "waiting_user"].contains($0.text("status")) }.map { RovaiRow(value: $0) }) { row in HStack { Text(row.value.text("agentId")).font(.caption); Spacer(); Button("停止") { Task { await model.command(.agentRunsCancel, value: .object(["threadId": .string(id), "agentRunId": .string(row.id), "expectedVersion": row.value["version"] ?? .number(0)])) } }.disabled(model.pending != nil) } }.padding(16) }
            ScrollView(.horizontal, showsIndicators: false) { HStack { ForEach((model.thread?["members"]?.rvArray ?? []).map { RovaiRow(value: $0) }) { row in Button { if recipients.contains(row.id) { recipients.remove(row.id) } else { recipients.insert(row.id) } } label: { Text(row.value.text("displayName")).font(.caption).padding(.horizontal, 12).padding(.vertical, 8).background(Capsule().fill(recipients.contains(row.id) ? Theme.accent.opacity(0.22) : Theme.accent.opacity(0.06))) } } }.padding(.horizontal, 16) }
            HStack(alignment: .bottom, spacing: 10) { TextField("发送消息…", text: $text, axis: .vertical).lineLimit(1...6).padding(12).background(RoundedRectangle(cornerRadius: 18).fill(Theme.bgElevated)).disabled(model.pending != nil); Button { submitted = text; Task { await model.send(text, recipients: recipients) } } label: { Image(systemName: "paperplane").font(.title3).padding(12) }.disabled(text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || model.pending != nil || model.busy) }.padding(12)
        }.navigationTitle(model.thread?["thread"]?.text("title") ?? "对话")
        .toolbar { ToolbarItem(placement: .topBarTrailing) { Button { privateChat = true } label: { Image(systemName: "person.bubble") }; Button { engineering = true } label: { Image(systemName: "folder") } } }
        .sheet(isPresented: $privateChat) { RovaiPrivateConversationView(model: model, threadId: id) }
        .sheet(isPresented: $engineering) { RovaiEngineeringView(model: model, threadId: id) }
        .task(id: id) { await model.open(id) }.onDisappear { if model.threadId == id { Task { await model.open(nil) } } }
        .onChange(of: model.lastResult) { _, result in if model.lastOperation == .threadMessagesSend, result?["commandResult"]?.text("status") == "applied", text == submitted { text = ""; submitted = nil } }
    }
}
private struct RovaiNewConversation: View {
    let model: RovaiWorkspaceModel; let mission: Bool; let onClose: () -> Void
    @State private var name = ""; @State private var detail = ""; @State private var path = ""; @State private var ids = Set<String>(); @State private var lead = ""; @State private var coordinated = false
    var body: some View { NavigationStack { Form { Section("内容") { TextField("名称", text: $name); if mission { TextField("描述", text: $detail, axis: .vertical).lineLimit(3...8) } }; Section("工作目录") { Picker("工程", selection: $path) { Text("快速对话").tag(""); ForEach((model.navigation["projects"]?.rvArray ?? []).map { RovaiRow(value: $0) }, id: \.value) { row in Text(row.value.text("name")).tag(row.value.text("projectPath")) } } }; Section("队员与负责人") { ForEach(model.members.filter { $0.text("presence") == "present" }.map { RovaiRow(value: $0) }) { row in Toggle(row.value.text("displayName"), isOn: Binding(get: { ids.contains(row.id) }, set: { selected in if selected { ids.insert(row.id) } else { ids.remove(row.id) }; if !ids.contains(lead) { lead = ids.sorted().first ?? "" } })) }; Picker("负责人", selection: $lead) { ForEach(model.members.filter { ids.contains($0.text("agentId")) }.map { RovaiRow(value: $0) }) { row in Text(row.value.text("displayName")).tag(row.id) } }; if !mission { Toggle("由负责人协调", isOn: $coordinated) } } }.navigationTitle(mission ? "新建使命" : "新建对话").toolbar { ToolbarItem(placement: .cancellationAction) { Button("取消", action: onClose) }; ToolbarItem(placement: .confirmationAction) { Button("创建") { Task { if mission { await model.command(.missionsCreate, value: .object(["title": .string(name), "description": .string(detail), "projectPath": .string(path), "projectBindingKind": .string(path.isEmpty ? "quick_chat" : "directory"), "memberAgentIds": .array(ids.sorted().map(JSONValue.string)), "defaultLeadAgentId": .string(lead), "tags": .array([])])) } else { await model.execute(.threadsCreate, params: .object(["commandId": .string(UUID().uuidString.lowercased()), "name": .string(name), "workspace": path.isEmpty ? .null : .object(["projectPath": .string(path)]), "memberAgentIds": .array(ids.sorted().map(JSONValue.string)), "defaultLeadAgentId": .string(lead), "collaborationMode": .string(coordinated ? "lead_coordinated" : "peer"), "activationState": .string("active")])) }; onClose() } }.disabled(ids.isEmpty || !ids.contains(lead) || mission && name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || model.pending != nil) } } } }
}
