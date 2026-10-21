import SwiftUI

/// canvas-file.ts: a literal path is encoded component by component, preserving the exact Host target.
func canvasFileSource(_ node: JSONValue) throws -> JSONValue {
    let data = node["data"] ?? .null, thread = data["fileReference"]?.text("threadId") ?? "", path = data.text("path")
    guard data.text("kind") == "file", !thread.isEmpty, thread == thread.trimmingCharacters(in: .whitespacesAndNewlines), thread.utf8.count <= 128, !thread.contains("\0"), !path.isEmpty, !path.contains("\0") else {throw RovaiHostFailure(code: "文件节点尚未绑定原工程会话，请先选择来源")}
    let windows = path.range(of: "^[A-Za-z]:[\\\\/]", options: .regularExpression) != nil, unc = path.hasPrefix("\\\\") || path.hasPrefix("//")
    let normalized = windows || unc ? path.replacingOccurrences(of: "\\", with: "/") : path
    guard windows || unc || normalized.hasPrefix("/") else {throw RovaiHostFailure(code: "文件节点必须保存绝对路径")}
    let allowed = CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~")
    let parts = try normalized.components(separatedBy: "/").enumerated().map {index, part -> String in
        if windows && index == 0 {return part}
        guard let encoded = part.addingPercentEncoding(withAllowedCharacters: allowed) else {throw RovaiHostFailure(code: "文件路径无法编码")};return encoded
    }
    let encoded = parts.joined(separator: "/"), reference = unc ? encoded : "file://" + (windows ? "/" : "") + encoded
    guard reference.utf8.count <= 4096 else {throw RovaiHostFailure(code: "文件引用过长")}
    return .object(["kind": .string("camp_workspace"), "threadId": .string(thread), "rawReference": .string(reference)])
}

struct RovaiCanvasFileNode: View {
    let model: RovaiWorkspaceModel
    let node: JSONValue
    @Environment(\.dismiss) private var dismiss
    @State private var source: JSONValue?, issue: String?
    var body: some View {
        Group {
            if let source {if rovaiOfficeFile(node["data"]?.text("path") ?? "") {RovaiOfficePreview(model: model, threadId: source.text("threadId"), path: node["data"]?.text("path") ?? "")} else {RovaiFilePreview(model: model, source: source)}}
            else {NavigationStack {ContentUnavailableView("文件暂不可用", systemImage: "doc.badge.ellipsis", description: Text(issue ?? "正在核对原文件引用…")).toolbar {Button("关闭") {dismiss()}}}}
        }.task {do {source = try canvasFileSource(node);issue = nil} catch {issue = error.localizedDescription}}
    }
}
