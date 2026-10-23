import SwiftUI

struct RovaiProjectNavigation<ThreadLink: View>: View {
    let model: RovaiWorkspaceModel
    let threadLink: (JSONValue) -> ThreadLink
    let onSelect: ((String) -> Void)?
    @Environment(\.scenePhase) private var scenePhase
    @State private var snapshot: JSONValue = .object(["projectGroups": .array([]),"projectOrder": .null])
    @State private var folders: [String:RovaiCanvasFolderRelation] = [:]
    @State private var collapsed = Set<String>()
    @State private var ready = false
    @State private var issue: String?
    @State private var managing = false
    @State private var refresh = 0
    private let owner = "project-navigation:groups"
    private var sections: [RovaiProjectNavigationSection] {projectNavigationSections(snapshot,projects: model.navigation["projects"]?.rvArray ?? [],folders: folders)}
    var body: some View {
        List {
            Section {Button("管理项目分组") {managing = true};if let issue {Text(issue).font(.caption).foregroundStyle(Theme.warning);Button("重新读取分组") {refresh += 1}}}
            Section("快速对话") {ForEach((model.navigation["quickChat"]?["recentThreads"]?.rvArray ?? []).map {RovaiRow(value: $0)}) {row in threadLink(row.value)}}
            ForEach(sections) {section in
                Section {
                    if section.groupId == nil || !collapsed.contains(section.groupId!) {
                        if section.groupId != nil && section.projects.isEmpty {Text("分组中暂无已登记的工程").font(.caption).foregroundStyle(Theme.textSecondary)}
                        ForEach(section.projects.map {RovaiRow(value: $0)},id: \.value) {project in
                            if section.groupId != nil {Text(project.value.text("name")).font(.subheadline.weight(.semibold))}
                            if folders[project.value.text("projectPath")]?.complete == false {Text("工作树关系待核对").font(.caption).foregroundStyle(Theme.textSecondary)}
                            ForEach((project.value["recentThreads"]?.rvArray ?? []).map {RovaiRow(value: $0)}) {row in threadLink(row.value)}
                            RovaiMoreThreads(model: model,path: project.value.text("projectPath"),total: project.value["totalCount"]?.rvInt ?? 0,displayed: Set((project.value["recentThreads"]?.rvArray ?? []).map {$0.text("id")}),onSelect: onSelect).id(project.value.text("projectPath"))
                        }
                    }
                } header: {
                    if let id = section.groupId {Button {toggle(id)} label: {HStack(spacing: 8) {Image(systemName: collapsed.contains(id) ? "chevron.right" : "chevron.down");Image(systemName: "folder").foregroundStyle(rovaiCanvasTint(section.color ?? "") ?? Theme.textSecondary);Text(section.name);Spacer();Text(String(section.projects.count)).foregroundStyle(Theme.textSecondary)}}.buttonStyle(.plain).disabled(!ready).accessibilityLabel((collapsed.contains(id) ? "展开分组：" : "收起分组：")+section.name)} else {Text(section.name)}
                }
            }
        }.scrollContentBackground(.hidden)
        .refreshable {await model.refresh();refresh += 1}
        .sheet(isPresented: $managing, onDismiss: { refresh += 1 }) { RovaiProjectGroups(model: model) }
        .task {restore()}
        .task(id: "\(model.connectionGeneration):\(model.connected):\(scenePhase):\(refresh)") {
            guard model.connected,scenePhase == .active else {return};let generation = model.connectionGeneration
            while !Task.isCancelled {
                do {
                    let next = try await readProjectGroups(model)
                    let roots = (model.navigation["projects"]?.rvArray ?? []).map {$0.text("projectPath")}+(next["projectGroups"]?.rvArray ?? []).flatMap {($0["projectPaths"]?.rvArray ?? []).compactMap(\.stringValue)}
                    let relations = try await readCanvasFolderRelations(model,roots: roots)
                    guard generation == model.connectionGeneration,!Task.isCancelled else {return}
                    folders = relations.mapValues {value in value}
                    snapshot = next;issue = nil
                } catch is CancellationError {return} catch {if generation == model.connectionGeneration {issue = error.localizedDescription}}
                do {try await Task.sleep(for: .seconds(20))} catch {return}
            }
        }
    }
    private func restore() {
        do {let raw = try model.readBody(owner);if !raw.isEmpty {guard let value = JSONValue.parse(raw),value["version"] == .number(1),case .array(let values) = value["collapsed"],values.allSatisfy({$0.stringValue != nil}) else {throw RovaiHostFailure(code: "项目分组显示偏好无法读取")};collapsed = Set(values.compactMap(\.stringValue))};ready = true} catch {issue = error.localizedDescription}
    }
    private func toggle(_ id: String) {
        guard ready else {return};var next = collapsed;if next.contains(id) {next.remove(id)} else {next.insert(id)}
        do {let value = JSONValue.object(["version": .number(1),"collapsed": .array(next.sorted().map(JSONValue.string))]);let body = String(decoding: try JSONEncoder().encode(value),as: UTF8.self);try model.saveBody(owner,body: body);collapsed = next} catch {issue = error.localizedDescription}
    }
}
