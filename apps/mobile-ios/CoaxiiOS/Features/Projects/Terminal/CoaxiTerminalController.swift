import SwiftUI
import SwiftTerm
import Observation

@MainActor @Observable
final class CoaxiTerminalController {
    let view: SwiftTerm.TerminalView
    var snapshot: JSONValue?
    var issue: String?
    var busy = false
    var pending: JSONValue?
    var unknown = false
    @ObservationIgnored private let client: CoaxiHostClient
    @ObservationIgnored private let target: JSONValue
    @ObservationIgnored private let delegate: CoaxiTerminalIO
    @ObservationIgnored private var reader: Task<Void, Never>?
    @ObservationIgnored private var writer: Task<Void, Never>?
    @ObservationIgnored private var queue: [(String, Data)] = []
    @ObservationIgnored private var displayed: Int = 0
    @ObservationIgnored private var generation = ""
    @ObservationIgnored private var retired = false
    init(client: CoaxiHostClient, boardId: String, nodeId: String) {
        self.client = client; self.target = .object(["boardId": .string(boardId), "nodeId": .string(nodeId)])
        self.view = SwiftTerm.TerminalView(frame: .zero, font: UIFont.monospacedSystemFont(ofSize: 13, weight: .regular))
        self.delegate = CoaxiTerminalIO(); delegate.owner = self; view.terminalDelegate = delegate; view.inputAccessoryView = TerminalKeyBar(terminalView: view)
    }
    func visible(_ value: Bool) { reader?.cancel(); guard value, !retired else { return }; reader = Task { while !Task.isCancelled { do { try await read(); try await Task.sleep(for: snapshot?.text("phase") == "running" ? .milliseconds(500) : .seconds(2)) } catch is CancellationError { return } catch { issue = error.localizedDescription; try? await Task.sleep(for: .seconds(3)) } } } }
    func retire() { retired = true; reader?.cancel(); writer?.cancel(); queue.removeAll(); view.resignFirstResponder() }
    private func read() async throws {
        let next = try await client.request(.canvasTerminalsSnapshot, params: .object(["target": target, "generation": snapshot?["generation"] ?? .null, "afterSeq": .number(Double(displayed))]))
        guard !retired else { return }; let incoming = next.text("generation")
        if incoming != generation || next["gap"]?.rvBool == true { view.feed(text: "\u{1b}[2J\u{1b}[H"); displayed = 0; generation = incoming }
        for chunk in next["chunks"]?.rvArray ?? [] { let sequence = chunk["seq"]?.rvInt ?? 0; if sequence > displayed, let bytes = Data(base64Encoded: chunk.text("bytesBase64")) { view.feed(byteArray: ArraySlice(Array(bytes))); displayed = sequence } }
        snapshot = next
        if let pending, let receipt = next["inputs"]?.rvArray.first(where: { $0.text("inputId") == pending.text("inputId") }) { if receipt.text("status") == "written" { self.pending = nil; unknown = false; sendNext() } else if receipt.text("status") == "not_written" { unknown = true; issue = "终端未写入这份输入，请核对后重试原输入。" } }
    }
    func start() async { guard let snapshot, !busy, !retired else { return }; await action(.canvasTerminalsStart, params: .object(["target": target, "coreGeneration": .string(snapshot.text("coreGeneration")), "requestId": .string(UUID().uuidString.lowercased()), "expectedGeneration": snapshot["generation"] ?? .null, "cols": .number(80), "rows": .number(24)])) }
    func stop() async { guard let snapshot, !snapshot.text("generation").isEmpty else { return }; await action(.canvasTerminalsStop, params: .object(["target": target, "generation": snapshot["generation"] ?? .null])) }
    private func action(_ operation: CoaxiOperation, params: JSONValue) async { guard !busy, !retired else { return }; busy = true; defer { busy = false }; do { _ = try await client.request(operation, params: params); try await read() } catch { issue = error.localizedDescription } }
    func receive(_ bytes: Data) { guard !retired, snapshot?.text("phase") == "running", !unknown else { issue = "终端尚未准备好，输入未发送。"; return }; guard queue.reduce(0, { $0 + $1.1.count }) + bytes.count <= 64 * 1024 else { issue = "输入队列已满，请稍后继续。"; return }; let generation = snapshot?.text("generation") ?? ""; for offset in stride(from: 0, to: bytes.count, by: 8192) { queue.append((generation, bytes.subdata(in: offset..<min(offset + 8192, bytes.count)))) }; sendNext() }
    private func sendNext() {
        guard pending == nil, writer == nil, !retired, !queue.isEmpty, let snapshot else { return }
        let item = queue.removeFirst(); guard item.0 == snapshot.text("generation") else { issue = "终端已经重启，旧输入已停止发送。"; queue.removeAll(); return }
        let request: JSONValue = .object(["target": target, "generation": .string(item.0), "inputId": .string(UUID().uuidString.lowercased()), "expectedInputSequence": snapshot["inputSequence"] ?? .number(0), "bytesBase64": .string(item.1.base64EncodedString())]); pending = request
        writer = Task { do { let result = try await client.request(.canvasTerminalsWrite, params: request); if result.text("status") == "written" { var next = self.snapshot?.rvObject ?? [:]; next["inputSequence"] = result["sequence"]; self.snapshot = .object(next); self.pending = nil }; writer = nil; if pending == nil { sendNext() } } catch is CancellationError { writer = nil } catch { writer = nil; unknown = true; issue = error.localizedDescription } }
    }
    func resize(cols: Int, rows: Int) { guard !retired, snapshot?.text("phase") == "running", let generation = snapshot?["generation"] else { return }; Task { do { _ = try await client.request(.canvasTerminalsResize, params: .object(["target": target, "generation": generation, "cols": .number(Double(min(max(cols, 1), 1000))), "rows": .number(Double(min(max(rows, 1), 1000)))])) } catch { issue = error.localizedDescription } } }
    func reconcile() async { do { try await read() } catch { issue = error.localizedDescription } }
    func retry() async { guard let pending else { return }; await action(.canvasTerminalsWrite, params: pending) }
}
private final class CoaxiTerminalIO: NSObject, TerminalViewDelegate {
    weak var owner: CoaxiTerminalController?
    func send(source: SwiftTerm.TerminalView, data: ArraySlice<UInt8>) { MainActor.assumeIsolated { owner?.receive(Data(data)) } }
    func sizeChanged(source: SwiftTerm.TerminalView, newCols: Int, newRows: Int) { MainActor.assumeIsolated { owner?.resize(cols: newCols, rows: newRows) } }
    func setTerminalTitle(source: SwiftTerm.TerminalView, title: String) {}
    func hostCurrentDirectoryUpdate(source: SwiftTerm.TerminalView, directory: String?) {}
    func scrolled(source: SwiftTerm.TerminalView, position: Double) {}
    func requestOpenLink(source: SwiftTerm.TerminalView, link: String, params: [String: String]) {}
    func bell(source: SwiftTerm.TerminalView) {}
    func clipboardCopy(source: SwiftTerm.TerminalView, content: Data) {}
    func iTermContent(source: SwiftTerm.TerminalView, content: ArraySlice<UInt8>) {}
    func rangeChanged(source: SwiftTerm.TerminalView, startY: Int, endY: Int) {}
}
struct CoaxiTerminalView: View {
    let controller: CoaxiTerminalController
    @Environment(\.dismiss) private var dismiss
    @Environment(\.scenePhase) private var scenePhase
    var body: some View { NavigationStack { VStack(spacing: 0) { HStack { Text(controller.snapshot?.text("phase") ?? "正在读取终端…").font(.caption); Spacer(); Button("启动") { Task { await controller.start() } }.disabled(controller.busy || controller.snapshot?["cleanupComplete"]?.rvBool == false); Button("停止") { Task { await controller.stop() } }.disabled(controller.busy || (controller.snapshot?.text("generation") ?? "").isEmpty) }.padding(12); if let issue = controller.issue { Text(issue).font(.caption).foregroundStyle(Theme.warning).padding(8) }; if controller.unknown { HStack { Button("核对输入") { Task { await controller.reconcile() } }; Button("重试原输入") { Task { await controller.retry() } } } }; CoaxiTerminalSurface(terminal: controller.view); Text("关闭界面会保留终端进程；需要结束时点击停止。").font(.caption2).foregroundStyle(Theme.textSecondary).padding(8) }.navigationTitle("终端").toolbar { ToolbarItem(placement: .confirmationAction) { Button("关闭") { dismiss() } } }.onAppear { controller.visible(true) }.onDisappear { controller.visible(false) }.onChange(of: scenePhase) { _, phase in controller.visible(phase == .active) } } }
}
private struct CoaxiTerminalSurface: UIViewRepresentable {
    let terminal: SwiftTerm.TerminalView
    func makeUIView(context: Context) -> SwiftTerm.TerminalView { terminal }
    func updateUIView(_ uiView: SwiftTerm.TerminalView, context: Context) {}
}
