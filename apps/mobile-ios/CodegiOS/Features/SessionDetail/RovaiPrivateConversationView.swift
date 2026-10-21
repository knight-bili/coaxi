import SwiftUI

struct RovaiPrivateConversationView: View {
    let model: RovaiWorkspaceModel
    let threadId: String
    var childId: String? = nil
    var initialConversationId: String? = nil
    @Environment(\.dismiss) private var dismiss
    @State private var conversations: [JSONValue] = []
    @State private var selected: String?
    @State private var snapshot: JSONValue = .object([:])
    @State private var text = ""
    @State private var bodyReady = false
    @State private var retained: [JSONValue] = []
    @State private var cursor: String?
    @State private var issue: String?
    @State private var busy = false
    @State private var ending = false
    @State private var openingCommand: String?
    @State private var quoteSelection: QuoteSource?
    @State private var removedQuoteId: String?
    private struct QuoteSource: Identifiable {let id: String;let conversationId: String;let revision: Int}
    private var child: Bool { childId != nil }
    private var owner: String { "private:\(threadId):\(selected ?? ""):\(child ? "delegation_child" : "single_chat")" }
    private var disabled: Bool { busy || model.busy || model.pending != nil || !bodyReady }
    private var get: RovaiOperation { child ? .delegationsConversationGet : .singleChatGet }
    private var send: RovaiOperation { child ? .delegationsMessageSend : .singleChatSend }
    private var edit: RovaiOperation { child ? .delegationsPendingInputsEdit : .singleChatPendingInputsEdit }
    private var mutate: RovaiOperation { child ? .delegationsComposerMutate : .singleChatComposerDraftMutate }
    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                if let issue { Text(issue).font(.caption).foregroundStyle(Theme.warning).padding(.horizontal, 16) }
                if model.pending != nil { HStack { Text("原操作待核对").font(.caption); Spacer(); Button("核对") { Task { await model.reconcile() } }; Button("重试原操作") { Task { await model.retry() } } }.padding(12).disabled(model.busy) }
                if selected == nil { chooser } else { transcript; composer }
            }.navigationTitle(child ? "独立帮手" : "队员私聊")
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("关闭") { close() } }; ToolbarItem(placement: .topBarTrailing) { Button("刷新") { Task { await load(); await readRetained() } }.disabled(busy) } }
        }.interactiveDismissDisabled(true)
        .task { if let childId { selected = childId } else if let initialConversationId { selected = initialConversationId } else { await load() } }
        .task(id: selected) { bodyReady = false; snapshot = .object([:]); retained = []; removedQuoteId = nil; quoteSelection = nil; if selected != nil { do { text = try model.readBody(owner); bodyReady = true; await load(); await readRetained() } catch { issue = error.localizedDescription } } }
        .onChange(of: text) { _, value in guard bodyReady else { return }; do { try model.saveBody(owner, body: value) } catch { issue = error.localizedDescription } }
        .onChange(of: model.thread) { _, _ in Task { await load() } }
        .onChange(of: model.lastUploadResult) { _, _ in Task { await load(); await readRetained() } }
        .onChange(of: model.lastResult) { _, value in
            let receipt = value?["commandResult"] ?? value
            if !child, model.lastOperation == .singleChatOpen, receipt?.text("commandId") == openingCommand, receipt?.text("status") == "applied", let id = receipt?["payload"]?["conversationId"]?.stringValue { selected = id }
            else { Task { await load(); await readRetained(); if let operation = model.lastOperation, [send, edit, mutate].contains(operation), model.pending == nil, selected != nil { do { text = try model.readBody(owner) } catch { issue = error.localizedDescription } } } }
        }
        .sheet(item: $quoteSelection) {source in RovaiQuoteSelector(model: model, threadId: threadId, messageId: source.id, onChosen: {_ in false}, conversationId: source.conversationId, onSelection: {selection in
            guard !disabled, selected == source.conversationId, snapshot["conversation"]?.text("status") == "active" else {return false}
            do {try model.saveBody(owner, body: text)} catch {issue = error.localizedDescription;return false}
            let value = JSONValue.object(["threadId": .string(threadId), "conversationId": .string(source.conversationId), "composerScope": .null, "expectedDraftRevision": .number(Double(source.revision)), "action": .object(["type": .string("quote"), "action": .object(["type": .string("add"), "selection": selection])])])
            Task {await model.command(mutate, value: value)};return true
        })}
        .confirmationDialog("结束当前私聊？", isPresented: $ending, titleVisibility: .visible) { Button("结束会话", role: .destructive) { Task { var value: [String: JSONValue] = ["conversationId": .string(selected ?? "")]; if !child { value["threadId"] = .string(threadId) }; await model.command(child ? .delegationsConversationEnd : .singleChatEnd, value: .object(value)) } } } message: { Text("保留消息和历史记录；正在执行的工作由Core处理结束。") }
    }
    private var chooser: some View {
        List {
            Section("队员") { ForEach((model.thread?["members"]?.rvArray ?? []).filter { $0.text("membershipStatus") == "active" && $0.text("profilePresence") == "present" }.map { RovaiRow(value: $0) }) { row in Button(row.value.text("displayName")) { Task { let id = UUID().uuidString.lowercased(); openingCommand = id; await model.execute(.singleChatOpen, params: .object(["commandId": .string(id), "command": .object(["threadId": .string(threadId), "agentId": .string(row.id)])])) } }.disabled(model.pending != nil || model.busy) } }
            Section("已有私聊") { ForEach(conversations.map { RovaiRow(value: $0) }) { row in Button { selected = row.id } label: { VStack(alignment: .leading, spacing: 5) { Text(model.members.first { $0.text("agentId") == row.value.text("agentId") }?.text("displayName") ?? row.value.text("agentId")); Text(row.value.text("updatedAt")).font(.caption).foregroundStyle(Theme.textSecondary) } } } }
        }.scrollContentBackground(.hidden)
    }
    private var transcript: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 12) {
                HStack { if !child { Button("其他私聊") { do { try model.saveBody(owner, body: text); selected = nil } catch { issue = error.localizedDescription } } }; RovaiAttachmentPicker(model: model, threadId: threadId, target: .object(["kind": .string(child ? "delegation_child" : "single_chat"), "conversationId": .string(selected ?? ""), "composerScope": .null]), revision: snapshot["draft"]?["revision"]?.rvInt ?? 0, disabled: disabled || snapshot["conversation"]?.text("status") != "active"); Spacer(); Button("结束会话", role: .destructive) { ending = true }.disabled(disabled) }
                if let selected { RovaiInteractionCards(model: model, conversationId: selected); RovaiDelegationList(model: model, threadId: threadId, parentConversationId: selected) }
                ForEach((snapshot["messages"]?.rvArray ?? []).map { RovaiRow(value: $0) }) { row in
                    GlassRow(isSelected: row.value.text("authorType") == "user") { VStack(alignment: .leading, spacing: 8) { Text(row.value.text("authorType") == "user" ? "你" : row.value.text("authorType")).font(.caption).foregroundStyle(Theme.textSecondary); RovaiMessageBody(model: model, threadId: threadId, message: row.value, conversationId: selected, child: child); Button("引用一段") {if let selected {quoteSelection = QuoteSource(id: row.id, conversationId: selected, revision: snapshot["draft"]?["revision"]?.rvInt ?? 0)}}.font(.caption).disabled(disabled || snapshot["conversation"]?.text("status") != "active") } }
                }
                ForEach((snapshot["agentRuns"]?.rvArray ?? []).map { RovaiRow(value: $0) }) { row in run(row.value) }
                ForEach((snapshot["pendingInputs"]?["items"]?.rvArray ?? []).map { RovaiRow(value: $0) }) { row in pending(row.value) }
                if let session = snapshot["pendingInputs"]?["editSession"], !session.text("editToken").isEmpty, let selected, let item = (snapshot["pendingInputs"]?["items"]?.rvArray ?? []).first(where: {$0.text("id") == session.text("pendingInputId")}) {RovaiPendingEditor(model: model, threadId: threadId, conversationId: selected, child: child, item: item, session: session, messages: snapshot["messages"]?.rvArray ?? [])}
                ForEach(retained.map { RovaiRow(value: $0) }) { row in retainedInput(row.value) }
                if let cursor { Button("更多保留输入") { Task { await readRetained(after: cursor) } } }
                RovaiComposerQuotes(quotes: snapshot["draft"]?["quotes"]?.rvArray ?? [], enabled: !disabled && snapshot["conversation"]?.text("status") == "active") {id in removedQuoteId = id;Task {await model.command(mutate, value: mutation(.object(["type": .string("quote"), "action": .object(["type": .string("remove"), "quoteId": .string(id)])])))}}
                if let removedQuoteId {Button("撤销移除引用") {Task {await model.command(mutate, value: mutation(.object(["type": .string("quote"), "action": .object(["type": .string("restore"), "quoteId": .string(removedQuoteId)])])))};self.removedQuoteId = nil}.font(.caption).disabled(disabled || snapshot["conversation"]?.text("status") != "active")}
                ForEach(snapshot["draft"]?["attachments"]?.rvArray ?? [], id: \.self) { attachment in HStack { Text("附件 · \(attachment.text("displayName"))").font(.caption); Spacer(); Button("移除") { Task { await model.command(mutate, value: mutation(.object(["type": .string("remove_attachment"), "attachmentRefId": attachment["id"] ?? .null]))) } }.disabled(disabled) }
            }.padding(16)
        }
    }
    private func run(_ value: JSONValue) -> some View {
        GlassRow { VStack(alignment: .leading, spacing: 8) { RovaiExecutionHistory(model: model, threadId: threadId, run: value, observed: snapshot["executionEvidence"]?.rvArray ?? []); Text(value.text("status")).font(.caption); if !value.text("lastErrorCode").isEmpty { Text(value.text("lastErrorCode")).foregroundStyle(Theme.warning) }; if ["running", "queued", "waiting"].contains(value.text("status")) { Button("停止") { Task { await model.command(.agentRunsCancel, value: .object(["threadId": .string(threadId), "agentRunId": value["id"] ?? .null, "expectedVersion": value["version"] ?? .number(0)])) } }.disabled(disabled) } } }
    }
    private func pending(_ value: JSONValue) -> some View {
        GlassRow { VStack(alignment: .leading, spacing: 8) { Text("排队输入").font(.headline); MarkdownContent(raw: value.text("body")); Text("\(value["attachments"]?.rvArray.count ?? 0) 个附件 · \(value["quotes"]?.rvArray.count ?? 0) 条引用").font(.caption); Button("编辑") {Task {await model.command(edit, value: pendingCommand(value, action: .object(["type": .string("begin")])))} }.disabled(disabled)
            Button("移回输入框（保留当前输入）") { Task { do { try model.saveBody(owner, body: text); await model.executePrivate(edit, command: pendingCommand(value, action: .object(["type": .string("return_to_composer"), "expectedDraftRevision": snapshot["draft"]?["revision"] ?? .number(0), "targetBody": .string(text)])), owner: owner, body: text, replace: true) } catch { issue = error.localizedDescription } } }.disabled(disabled)
            Button("删除排队消息", role: .destructive) { Task { await model.command(edit, value: pendingCommand(value, action: .object(["type": .string("delete")]))) } }.disabled(disabled)
        } }
    }
    private func retainedInput(_ value: JSONValue) -> some View {
        GlassRow { VStack(alignment: .leading, spacing: 8) { Text("保留的完整输入").font(.headline); MarkdownContent(raw: value.text("body")); Text("\(value["attachments"]?.rvArray.count ?? 0) 个附件 · \(value["quotes"]?.rvArray.count ?? 0) 条引用").font(.caption)
            Button("恢复到输入框（保留当前输入）") { Task { do { try model.saveBody(owner, body: text); await model.executePrivate(mutate, command: mutation(.object(["type": .string("swap_retained"), "retainedInputId": value["id"] ?? .null, "expectedRetainedRevision": value["revision"] ?? .number(0), "currentBody": .string(text)])), owner: owner, body: text, replace: true) } catch { issue = error.localizedDescription } } }.disabled(disabled)
        } }
    }
    private var composer: some View {
        HStack(alignment: .bottom, spacing: 10) {
            TextField("发送私聊消息…", text: $text, axis: .vertical).lineLimit(1...6).padding(12).background(RoundedRectangle(cornerRadius: 18).fill(Theme.bgElevated)).disabled(disabled || snapshot["conversation"]?.text("status") != "active")
            Button("发送") { Task { do { try model.saveBody(owner, body: text); var value: [String: JSONValue] = ["conversationId": .string(selected ?? ""), "body": .string(text.trimmingCharacters(in: .whitespacesAndNewlines)), "composerScope": .null, child ? "expectedRevision" : "draftRevision": snapshot["draft"]?["revision"] ?? .number(0)]; if !child { value["threadId"] = .string(threadId) }; await model.executePrivate(send, command: .object(value), owner: owner, body: text) } catch { issue = error.localizedDescription } } }.disabled(disabled || text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || snapshot["conversation"]?.text("status") != "active")
        }.padding(12)
    }
    private func pendingCommand(_ input: JSONValue, action: JSONValue) -> JSONValue {
        let session = snapshot["pendingInputs"]?["editSession"]
        var value: [String: JSONValue] = ["conversationId": .string(selected ?? ""), "composerScope": .null, "pendingInputId": input["id"] ?? .null, "expectedRevision": input["revision"] ?? .number(0), "editToken": session?.text("pendingInputId") == input.text("id") ? session?["editToken"] ?? .null : .null, "action": action]; if !child { value["threadId"] = .string(threadId) }; return .object(value)
    }
    private func mutation(_ action: JSONValue) -> JSONValue { .object(["threadId": .string(threadId), "conversationId": .string(selected ?? ""), "composerScope": .null, "expectedDraftRevision": snapshot["draft"]?["revision"] ?? .number(0), "action": action]) }
    private func load() async {
        guard !busy else { return }; busy = true; defer { busy = false }; let target = selected
        do {
            if !child { conversations = try await model.query(.singleChatList, params: .object(["threadId": .string(threadId)])).rvArray }
            if let target { let value = try await model.query(get, params: .object(["conversationId": .string(target), "composerScope": .null])); guard value["conversation"]?.text("id") == target, value["conversation"]?.text("threadId") == threadId, value["draft"]?["scopeVersion"]?.rvInt == 1, value["draft"]?.text("scopeKey").isEmpty == true, value["draft"]?["composerScope"] == .null else { issue = "该私聊材料不属于当前输入框"; return }; if selected == target { snapshot = value } }; issue = nil
        } catch is CancellationError {} catch { issue = error.localizedDescription }
    }
    private func readRetained(after: String? = nil) async {
        guard let selected else { return }
        do { var params: [String: JSONValue] = ["conversationId": .string(selected), "limit": .number(50)]; if child { params["composerScope"] = .null } else { params["threadId"] = .string(threadId) }; if let after { params["cursor"] = .string(after) }; let page = try await model.query(child ? .delegationsComposerRecovery : .singleChatComposerDraftRecovery, params: .object(params)); let inputs = (page["items"]?.rvArray ?? []).filter { $0.text("kind") == "retained" }.compactMap { $0["input"] }.filter { $0.text("conversationId") == selected && $0.text("scopeKey").isEmpty }; guard self.selected == selected else { return }; retained = after == nil ? inputs : retained + inputs; cursor = page["nextCursor"]?.stringValue } catch is CancellationError {} catch { issue = error.localizedDescription }
    }
    private func close() { do { if selected != nil && bodyReady { try model.saveBody(owner, body: text) }; dismiss() } catch { issue = error.localizedDescription } }
}
