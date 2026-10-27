import SwiftUI

struct CoaxiPublicAttachments: View {
    let model: CoaxiWorkspaceModel
    let threadId: String
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            CoaxiAttachmentPicker(model: model, threadId: threadId, target: .object(["kind": .string("camp")]), revision: 0, disabled: model.pending != nil || model.busy || !model.publicSourcesReady)
            ForEach(model.publicSources, id: \.self) { source in
                GlassRow { HStack { Image(systemName: "paperclip").foregroundStyle(Theme.textSecondary); Text(source.text("displayName")).font(.subheadline); Spacer(); Button("移除") { model.removePublicSource(source.text("id")) }.disabled(model.pending != nil || model.busy) } }
            }
        }
    }
}
