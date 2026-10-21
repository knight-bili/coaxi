import SwiftUI
import UniformTypeIdentifiers

private struct FrozenNativeUpload { let threadId: String; let target: JSONValue; let revision: Int; let generation: UInt64 }
struct RovaiAttachmentPicker: View {
    let model: RovaiWorkspaceModel
    let threadId: String
    let target: JSONValue
    let revision: Int
    let disabled: Bool
    @State private var presented = false
    @State private var frozen: FrozenNativeUpload?
    var body: some View {
        Button { frozen = FrozenNativeUpload(threadId: threadId, target: target, revision: revision, generation: model.connectionGeneration); presented = true } label: { Label("添加附件", systemImage: "paperclip") }.disabled(disabled || frozen != nil)
        .fileImporter(isPresented: $presented, allowedContentTypes: [.item], allowsMultipleSelection: false) { result in
            let request = frozen; frozen = nil
            switch result {
            case .success(let urls): if let url = urls.first, let request { Task { await model.upload(url, threadId: request.threadId, target: request.target, revision: request.revision, expectedGeneration: request.generation) } }
            case .failure(let error): model.issue = error.localizedDescription
            }
        }
    }
}
