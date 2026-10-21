import SwiftUI

/// Original native folder drill-in, editor and Git confirmation presentation.
/// The selected Core Thread owns the filesystem and Git scope on the Host.
struct RovaiEngineeringView: View {
    let model: RovaiWorkspaceModel
    let threadId: String
    @Environment(\.dismiss) private var dismiss
    @State private var tab = 0
    @State private var office: String?
    @State private var directory = ""
    @State private var entries: [JSONValue] = []
    @State private var after: String?
    @State private var file: JSONValue?
    @State private var draft = ""
    @State private var baseline = ""
    @State private var busy = false
    @State private var issue: String?
    @State private var gitView = "status"
    @State private var gitOutput = ""
    @State private var action = "stage"
    @State private var paths = ""
    @State private var target = ""
    @State private var message = ""
    @State private var mode = "mixed"
    @State private var plan: GitPlan?
    @State private var discard = false
    @State private var leaving = false
    private var dirty: Bool { file != nil && draft != baseline }
    private var scope: JSONValue { .object(["threadId": .string(threadId)]) }
    private struct GitPlan: Identifiable { let id = UUID(); let arguments: JSONValue; let preview: JSONValue; let commandId = UUID().uuidString.lowercased() }
    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                Picker("工程", selection: $tab) { Text("文件").tag(0); Text("Git").tag(1) }.pickerStyle(.segmented).padding(12).disabled(dirty)
                if let issue { Text(issue).font(.caption).foregroundStyle(Theme.warning).frame(maxWidth: .infinity, alignment: .leading).padding(12) }
                if tab == 0 { files } else { git }
            }
            .navigationTitle(file?.text("path").components(separatedBy: "/").last ?? (tab == 0 ? "工程文件" : "Git"))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .topBarLeading) { Button("关闭") { if dirty { leaving = true; discard = true } else { dismiss() } } }; ToolbarItem(placement: .topBarTrailing) { Button { Task { await work { if tab == 0 { await readDirectory() } else { await readGit() } } } } label: { Image(systemName: "arrow.clockwise") }.disabled(busy || dirty) } }
            .task(id: directory) { await work { await readDirectory() } }
            .task(id: tab) { if tab == 1 { await work { await readGit() } } }
            .confirmationDialog("放弃尚未保存的修改？", isPresented: $discard, titleVisibility: .visible) { Button("放弃修改", role: .destructive) { file = nil; if leaving { dismiss() } }; Button("继续编辑", role: .cancel) {} }
            .sheet(item: $plan) { selected in NavigationStack { Form { Section("即将执行") { Text(selected.preview["arguments"]?.rvArray.compactMap { $0.stringValue }.joined(separator: " ") ?? "").font(.system(.caption, design: .monospaced)); if !selected.preview.text("cleanPreview").isEmpty { Text(selected.preview.text("cleanPreview")) } }; Button("确认执行") { Task { var args = selected.arguments.rvObject; args["commandId"] = .string(selected.commandId); args["expectedRevision"] = selected.preview["revision"]; args["confirmed"] = .bool(true); await model.execute(.workbenchGitExecute, params: .object(args)); plan = nil; if model.pending == nil { await work { await readGit() } } } }.disabled(model.pending != nil || model.busy) }.navigationTitle("确认 Git 操作").toolbar { Button("取消") { plan = nil } } } }
        }.interactiveDismissDisabled(dirty || busy || model.pending != nil)
        .sheet(isPresented: Binding(get: { office != nil }, set: { if !$0 { office = nil } })) { if let office { RovaiOfficePreview(model: model, threadId: threadId, path: office) } }
    }
    @ViewBuilder private var files: some View {
        if let selected = file {
            VStack(spacing: 8) {
                Text(selected.text("path")).font(.system(.caption, design: .monospaced)).foregroundStyle(Theme.textSecondary).frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 12)
                TextEditor(text: $draft).font(.system(.body, design: .monospaced)).scrollContentBackground(.hidden).padding(12).disabled(model.pending != nil)
                HStack {
                    Button("保存") { let sent = draft; Task { await model.execute(.workbenchFilesMutate, params: .object(["scope": scope, "commandId": .string(UUID().uuidString.lowercased()), "action": .string("save"), "path": .string(selected.text("path")), "text": .string(sent), "expectedDigest": .string(selected.text("digest"))])); if model.pending == nil, model.lastOperation == .workbenchFilesMutate, model.lastResult?["applied"] == .bool(true) { baseline = sent; var value = selected.rvObject; value["digest"] = model.lastResult?["digest"]; file = .object(value) } } }.buttonStyle(.borderedProminent).disabled(!dirty || model.pending != nil || busy)
                    Button("关闭文件") { if dirty { leaving = false; discard = true } else { file = nil } }
                    Spacer()
                    Button("核对外部修改") { Task { await work { do { let current = try await model.query(.workbenchFilesRead, params: .object(["scope": scope, "path": .string(selected.text("path"))])); issue = current.text("digest") == selected.text("digest") ? "宿主文件版本未变化。" : "宿主文件已变化；你的编辑仍保留，请核对后另存或重新读取。" } catch { issue = error.localizedDescription } } } }.disabled(busy)
                }.padding(12)
            }
        } else {
            List {
                Section(directory.isEmpty ? "工程根目录" : directory) {
                    if !directory.isEmpty { Button { directory = (directory as NSString).deletingLastPathComponent } label: { Label("上一级", systemImage: "arrow.up") } }
                    ForEach(entries, id: \.self) { entry in Button { if entry["directory"] == .bool(true) { directory = entry.text("path") } else if rovaiOfficeFile(entry.text("path")) { office = entry.text("path") } else { Task { await work { do { var value = try await model.query(.workbenchFilesRead, params: .object(["scope": scope, "path": .string(entry.text("path"))])).rvObject; value["path"] = entry["path"]; file = .object(value); draft = file?.text("text") ?? ""; baseline = draft } catch { issue = error.localizedDescription } } } } } label: { HStack(spacing: 12) { Image(systemName: entry["directory"] == .bool(true) ? "folder" : "doc.text").foregroundStyle(Theme.textSecondary); Text(entry.text("name")).foregroundStyle(Theme.textPrimary); Spacer(); if entry["directory"] == .bool(true) { Image(systemName: "chevron.right").font(.caption).foregroundStyle(Theme.textSecondary) } } }.disabled(busy || entry["link"] == .bool(true)).listRowBackground(Color.clear) }
                    if let after { Button("加载更多") { Task { await work { await readDirectory(after: after) } } }.disabled(busy) }
                }
            }.scrollContentBackground(.hidden)
        }
    }
    private var git: some View {
        VStack(spacing: 0) {
            ScrollView(.horizontal) { HStack { ForEach(["status", "diff", "diff_staged", "log", "branches", "stash", "remotes", "worktrees", "reflog", "tags"], id: \.self) { value in Button(value) { gitView = value; Task { await work { await readGit() } } }.buttonStyle(.bordered).tint(gitView == value ? Theme.accent : Theme.textSecondary) } }.padding(.horizontal, 12) }.scrollIndicators(.hidden)
            ScrollView { Text(gitOutput.replacingOccurrences(of: "\0", with: "\n").replacingOccurrences(of: "\u{1e}", with: "\n\n")).font(.system(.caption, design: .monospaced)).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading).padding(12) }.frame(maxHeight: .infinity)
            Form {
                Picker("操作", selection: $action) { ForEach(["stage", "unstage", "commit", "fetch", "pull", "push", "checkout", "branch_create", "merge", "rebase", "merge_continue", "merge_abort", "rebase_continue", "rebase_abort", "stash_save", "stash_pop", "stash_apply", "reset", "cherry_pick", "revert"], id: \.self) { Text($0).tag($0) } }
                TextField("文件路径，每行一项", text: $paths, axis: .vertical).lineLimit(1...3).autocorrectionDisabled()
                TextField("目标分支或提交", text: $target).autocorrectionDisabled().textInputAutocapitalization(.never)
                TextField("提交或暂存说明", text: $message)
                if action == "reset" { Picker("重置模式", selection: $mode) { ForEach(["soft", "mixed", "hard"], id: \.self) { Text($0).tag($0) } } }
                Button("预览 Git 操作") { Task { await work { do { var args: [String: JSONValue] = ["scope": scope, "action": .string(action), "paths": .array(paths.components(separatedBy: .newlines).filter { !$0.isEmpty }.map { .string($0) })]; if !target.isEmpty { args["target"] = .string(target) }; if !message.isEmpty { args["message"] = .string(message) }; if action == "reset" { args["mode"] = .string(mode) }; let value = JSONValue.object(args); let preview = try await model.query(.workbenchGitPreview, params: value); plan = GitPlan(arguments: value, preview: preview) } catch { issue = error.localizedDescription } } } }.disabled(busy || model.pending != nil)
            }.frame(maxHeight: 320).scrollContentBackground(.hidden)
        }
    }
    private func work(_ block: () async -> Void) async { guard !busy else { return }; busy = true; issue = nil; defer { busy = false }; await block() }
    private func readDirectory(after: String? = nil) async {
        do { var args: [String: JSONValue] = ["scope": scope, "path": .string(directory)]; if let after { args["after"] = .string(after) }; let value = try await model.query(.workbenchFilesList, params: .object(args)); let next = value["entries"]?.rvArray ?? []; entries = after == nil ? next : entries + next; self.after = value["next"]?.stringValue } catch { issue = error.localizedDescription }
    }
    private func readGit() async {
        do { var args: [String: JSONValue] = ["scope": scope, "action": .string(gitView)]; if !target.isEmpty { args["target"] = .string(target) }; let value = try await model.query(.workbenchGitRead, params: .object(args)); gitOutput = value.text("output"); if value["success"] == .bool(false) { issue = value.text("error") } } catch { issue = error.localizedDescription }
    }
}
