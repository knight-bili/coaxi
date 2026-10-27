import SwiftUI
import ImageIO

func coaxiImageGroups(_ thread: JSONValue?, message: JSONValue) -> [JSONValue] {
    guard message.text("authorType") == "agent", message["withdrawn"]?.rvBool != true else { return [] }
    let run = message.text("sourceAgentRunId"); guard !run.isEmpty, (thread?["messages"]?.rvArray ?? []).last(where: { $0.text("authorType") == "agent" && $0.text("sourceAgentRunId") == run && $0["withdrawn"]?.rvBool != true })?.text("id") == message.text("id") else { return [] }
    return (thread?["agentRunImages"]?.rvArray ?? []).filter { $0.text("agentRunId") == run }
}
func coaxiOrphanImages(_ thread: JSONValue?) -> [JSONValue] {
    let publicRuns = Set((thread?["messages"]?.rvArray ?? []).filter { $0.text("authorType") == "agent" && $0["withdrawn"]?.rvBool != true }.map { $0.text("sourceAgentRunId") })
    return (thread?["agentRunImages"]?.rvArray ?? []).filter { group in !publicRuns.contains(group.text("agentRunId")) && (thread?["agentRuns"]?.rvArray ?? []).contains { $0.text("id") == group.text("agentRunId") && ["completed", "failed", "cancelled", "interrupted", "succeeded"].contains($0.text("status")) } }
}
struct CoaxiRuntimeImageGallery: View {
    let model: CoaxiWorkspaceModel
    let threadId: String
    let groups: [JSONValue]
    var body: some View { VStack(alignment: .leading, spacing: 8) { ForEach(groups, id: \.self) { group in ForEach((group["images"]?.rvArray ?? []).map { CoaxiRow(value: $0) }) { row in CoaxiRuntimeImage(model: model, threadId: threadId, image: row.value) } } } }
}
private struct CoaxiRuntimeImage: View {
    let model: CoaxiWorkspaceModel
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
        do { guard (image["byteSize"]?.rvInt ?? 0) <= 20 * 1024 * 1024 else { throw CoaxiHostFailure(code: "图片超过20MB，请在工程中查看") }; let content = try await model.query(.agentRunImagesRead, params: .object(["threadId": .string(threadId), "imageId": image["id"] ?? .null])); guard content.text("mediaType").hasPrefix("image/"), content.text("data").utf8.count <= 28 * 1024 * 1024, let data = Data(base64Encoded: content.text("data")) else { throw CoaxiHostFailure(code: "图片格式无法读取") }
            let result = await Task.detached(priority: .userInitiated) { () -> UIImage? in guard let source = CGImageSourceCreateWithData(data as CFData, nil), let thumbnail = CGImageSourceCreateThumbnailAtIndex(source, 0, [kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceThumbnailMaxPixelSize: 2048, kCGImageSourceCreateThumbnailWithTransform: true] as CFDictionary) else { return nil }; return UIImage(cgImage: thumbnail) }.value
            guard generation == model.connectionGeneration, !Task.isCancelled else { return }; guard let result else { throw CoaxiHostFailure(code: "图片无法解码") }; decoded = result; issue = nil
        } catch is CancellationError {} catch { if generation == model.connectionGeneration { issue = error.localizedDescription } }
    }
}

func coaxiAttachmentSource(_ threadId: String, message: JSONValue, attachment: JSONValue, conversationId: String?, child: Bool) -> JSONValue {
    var locator: [String: JSONValue] = ["threadId": .string(threadId), "attachmentRefId": attachment["id"] ?? .null, "owner": .string(conversationId == nil ? "message" : child ? "delegation_child_message" : "single_chat_message")]
    if let conversationId { locator["conversationId"] = .string(conversationId); locator["conversationMessageId"] = message["id"] ?? .null } else { locator["messageId"] = message["id"] ?? .null }
    return .object(["kind": .string("attachment"), "threadId": .string(threadId), "locator": .object(locator)])
}
struct CoaxiMessageBody: View {
    let model: CoaxiWorkspaceModel
    let threadId: String
    let message: JSONValue
    var conversationId: String? = nil
    var child = false
    @State private var generatedCard = false
    private var hasCard: Bool {message.text("authorType") == "agent" && message["withdrawn"]?.rvBool != true && message.text("body").range(of: "(?m)(`{3,}|~{3,})spec(?:[ \\t].*)?$", options: .regularExpression) != nil}
    private var user: Bool { message.text("authorType") == "user" }
    private var pictures: some View { ForEach((message["attachments"]?.rvArray ?? []).filter { $0.text("previewKind") == "image" }.map { CoaxiRow(value: $0) }) { row in CoaxiAttachmentImage(model: model, source: coaxiAttachmentSource(threadId, message: message, attachment: row.value, conversationId: conversationId, child: child), attachment: row.value, user: user) } }
    var body: some View { VStack(alignment: .leading, spacing: 8) { if user { pictures; CoaxiMessageArtifacts(model: model, threadId: threadId, message: message, conversationId: conversationId, child: child) }; MarkdownContent(raw: message["withdrawn"]?.rvBool == true ? "消息已撤回" : message.text("body"));if hasCard {Button("查看交互卡片") {generatedCard = true}.font(.caption)}; if !user { CoaxiMessageArtifacts(model: model, threadId: threadId, message: message, conversationId: conversationId, child: child); pictures } }.sheet(isPresented: $generatedCard) {CoaxiGeneratedCard(model: model, threadId: threadId, message: message, conversationId: conversationId, child: child)} }
}
private struct CoaxiAttachmentImage: View {
    let model: CoaxiWorkspaceModel
    let source: JSONValue
    let attachment: JSONValue
    let user: Bool
    @State private var decoded: UIImage?
    @State private var issue: String?
    @State private var retry = 0
    @State private var expanded = false
    var body: some View {
        Group { if let decoded { if user { Image(uiImage: decoded).resizable().scaledToFill().frame(width: 72, height: 72).clipped().clipShape(RoundedRectangle(cornerRadius: 10)).onTapGesture { expanded = true } } else { Image(uiImage: decoded).resizable().scaledToFit().frame(maxHeight: 320).clipShape(RoundedRectangle(cornerRadius: 10)).onTapGesture { expanded = true } } } else if issue != nil { Button("图片暂不可读，重试") { retry += 1 }.font(.caption).foregroundStyle(Theme.textSecondary) } else { ProgressView() } }
        .task(id: "\(source.hashValue):\(retry)") { await read() }
        .sheet(isPresented: $expanded) { CoaxiFilePreview(model: model, source: source) }
    }
    private func read() async {
        let generation = model.connectionGeneration; var handle: JSONValue?
        do { guard !["missing", "unreadable", "kind_changed"].contains(attachment.text("availability")) else { throw CoaxiHostFailure(code: "图片来源不可用") }; let opened = try await model.files("open", params: source); handle = opened["file"]?["handleId"]; guard opened.text("kind") == "file_preview", let file = opened["file"], file.text("kind") == "image", (file["size"]?.rvInt ?? 0) <= 20 * 1024 * 1024 else { throw CoaxiHostFailure(code: "图片无法预览") }; let payload = try await model.fileBytes("readBinary", params: .object(["handleId": handle ?? .null, "expectedGeneration": file["contentGeneration"] ?? .null])); let data = payload.data
            let image = await Task.detached(priority: .userInitiated) { () -> UIImage? in guard let source = CGImageSourceCreateWithData(data as CFData, nil), let thumbnail = CGImageSourceCreateThumbnailAtIndex(source, 0, [kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceThumbnailMaxPixelSize: 2048, kCGImageSourceCreateThumbnailWithTransform: true] as CFDictionary) else { return nil }; return UIImage(cgImage: thumbnail) }.value
            guard generation == model.connectionGeneration, !Task.isCancelled else { return }; guard let image else { throw CoaxiHostFailure(code: "图片无法解码") }; decoded = image; issue = nil
        } catch is CancellationError {} catch { if generation == model.connectionGeneration { issue = error.localizedDescription } }
        if let handle, generation == model.connectionGeneration { _ = try? await model.files("release", params: .object(["handleId": handle])) }
    }
}
