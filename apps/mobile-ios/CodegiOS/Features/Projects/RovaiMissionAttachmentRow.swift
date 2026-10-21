import SwiftUI

struct RovaiMissionAttachmentRow: View {
    let model: RovaiWorkspaceModel
    let mission: JSONValue
    let attachment: JSONValue
    let onOpen: () -> Void
    @State private var removal: JSONValue?
    var body: some View {
        HStack {
            Button(attachment.text("displayName"), action: onOpen)
            Spacer()
            Button("移除", role: .destructive) {
                removal = .object(["missionId": mission["missionId"] ?? .null, "attachmentRefId": attachment["id"] ?? .null, "expectedDetailsVersion": mission["detailsVersion"] ?? .null])
            }.disabled(model.pending != nil || model.busy)
        }.confirmationDialog("移除使命附件？", isPresented: Binding(get: {removal != nil}, set: {if !$0 {removal = nil}}), titleVisibility: .visible) {
            Button("移除引用", role: .destructive) {if let frozen = removal {Task {await model.command(.missionsAttachmentsRemove, value: frozen)}};removal = nil}.disabled(model.pending != nil || model.busy)
            Button("取消", role: .cancel) {removal = nil}
        } message: {Text("只移除此使命中的附件引用，保留原文件及已发布消息。使命已被其他端修改时拒绝覆盖。")}
    }
}
