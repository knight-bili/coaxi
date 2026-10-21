import SwiftUI
import ImageIO

func rovaiImageGroups(_ thread: JSONValue?, message: JSONValue) -> [JSONValue] {
    guard message.text("authorType") == "agent", message["withdrawn"]?.rvBool != true else { return [] }
    let run = message.text("sourceAgentRunId"); guard !run.isEmpty, (thread?["messages"]?.rvArray ?? []).last(where: { $0.text("authorType") == "agent" && $0.text("sourceAgentRunId") == run && $0["withdrawn"]?.rvBool != true })?.text("id") == message.text("id") else { return [] }
    return (thread?["agentRunImages"]?.rvArray ?? []).filter { $0.text("agentRunId") == run }
}
func rovaiOrphanImages(_ thread: JSONValue?) -> [JSONValue] {
    let publicRuns = Set((thread?["messages"]?.rvArray ?? []).filter { $0.text("authorType") == "agent" && $0["withdrawn"]?.rvBool != true }.map { $0.text("sourceAgentRunId") })
    return (thread?["agentRunImages"]?.rvArray ?? []).filter { group in !publicRuns.contains(group.text("agentRunId")) && (thread?["agentRuns"]?.rvArray ?? []).contains { $0.text("id") == group.text("agentRunId") && ["completed", "failed", "cancelled", "interrupted", "succeeded"].contains($0.text("status")) } }
}
struct RovaiRuntimeImageGallery: View {
    let model: RovaiWorkspaceModel
    let threadId: String
    let groups: [JSONValue]
    var body: some View { VStack(alignment: .leading, spacing: 8) { ForEach(groups, id: \.self) { group in ForEach((group["images"]?.rvArray ?? []).map { RovaiRow(value: $0) }) { row in RovaiRuntimeImage(model: model, threadId: threadId, image: row.value) } } } }
}
private struct RovaiRuntimeImage: View {
    let model: RovaiWorkspaceModel
    let threadId: String
    let image: JSONValue
    @State private var decoded: UIImage?
    @State private var issue: String?
    @State private var large = false
    @State private var retry = 0
    var body: some View {
        Group { if let decoded { Image(uiImage: decoded).resizable().scaledToFit().frame(maxHeight: 320).clipShape(RoundedRectangle(cornerRadius: 10)).onTapGesture { large = true } } else if issue != nil { Button("图片暂不可读，重试") { retry += 1 }.font(.caption).foregroundStyle(Theme.textSecondary) } else { ProgressView().frame(maxWidth: .infinity) } }
        .task(id: "\(threadId):\(image.text("id")):\(retry)") { await read() }
        .sheet(isPresented: $large) { NavigationStack { if let decoded { Image(uiImage: decoded).resizable().scaledToFit().padding(16).toolbar { ToolbarItem(placement: .cancellationAction) { Button("关闭") { large = false } } } } } }
    }
    private func read() async {
        let generation = model.connectionGeneration
        do { guard (image["byteSize"]?.rvInt ?? 0) <= 20 * 1024 * 1024 else { throw RovaiHostFailure(code: "图片超过20MB，请在工程中查看") }; let content = try await model.query(.agentRunImagesRead, params: .object(["threadId": .string(threadId), "imageId": image["id"] ?? .null])); guard content.text("mediaType").hasPrefix("image/"), content.text("data").utf8.count <= 28 * 1024 * 1024, let data = Data(base64Encoded: content.text("data")) else { throw RovaiHostFailure(code: "图片格式无法读取") }
            let result = await Task.detached(priority: .userInitiated) { () -> UIImage? in guard let source = CGImageSourceCreateWithData(data as CFData, nil), let thumbnail = CGImageSourceCreateThumbnailAtIndex(source, 0, [kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceThumbnailMaxPixelSize: 2048, kCGImageSourceCreateThumbnailWithTransform: true] as CFDictionary) else { return nil }; return UIImage(cgImage: thumbnail) }.value
            guard generation == model.connectionGeneration, !Task.isCancelled else { return }; guard let result else { throw RovaiHostFailure(code: "图片无法解码") }; decoded = result; issue = nil
        } catch is CancellationError {} catch { if generation == model.connectionGeneration { issue = error.localizedDescription } }
    }
}
