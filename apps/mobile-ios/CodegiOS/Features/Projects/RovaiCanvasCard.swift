import SwiftUI

func canvasRegionColumns(_ data: JSONValue,width: Double? = nil) -> Int {
    if let pinned = data["gridColumns"]?.rvInt,pinned>0 {return pinned}
    let original: Double;if case .number(let value) = data["width"] {original = value} else {original = 484}
    return max(1,Int(floor((max((width ?? original)-24,224)+12)/236)))
}
func canvasRegionVisible(_ data: JSONValue,resolution: RovaiCanvasRegionResolution,expanded: Bool) -> [RovaiCanvasRegionMember] {
    let columns = canvasRegionColumns(data),rows = data["gridRows"]?.rvInt ?? 0,cap = rows>0 ? rows*columns : 24
    return expanded ? resolution.members : Array(resolution.members.prefix(cap))
}
func canvasRegionHeight(_ data: JSONValue,resolution: RovaiCanvasRegionResolution,expanded: Bool) -> Double {
    let rows = Int(ceil(Double(canvasRegionVisible(data,resolution: resolution,expanded: expanded).count)/Double(canvasRegionColumns(data))))
    let stored: Double;if case .number(let value) = data["height"] {stored = value} else {stored = 196}
    return max(stored,Double(64+rows*132+max(0,rows-1)*12+36))
}

/// Original 224×132 summary, four-line title and model/project/branch evidence.
struct RovaiCanvasSummaryTile: View {
    let summary: RovaiCanvasSummary
    let color: String
    let onOpen: () -> Void
    var onExtra: (() -> Void)?
    var extraLabel = "编辑", disabled = false, catalogComplete = false, framed = true
    var titleOverride: String?
    var body: some View {
        VStack(alignment: .leading,spacing: 5) {
            HStack(spacing: 4) {
                if catalogComplete,let active = summary.active {Circle().fill(active ? Theme.accent : Theme.textTertiary).frame(width: 5,height: 5).accessibilityLabel(active ? "有进行中的任务" : "当前没有进行中的任务")}
                Text(summary.model ?? summary.kind).font(.system(size: 10)).lineLimit(1).foregroundStyle(Theme.textSecondary).help(summary.modelTitle ?? summary.kind)
                Spacer(minLength: 0)
                Button(action: onOpen) {Image(systemName: "arrow.up.left.and.arrow.down.right").font(.system(size: 11)).frame(width: 22,height: 22)}.accessibilityLabel("打开原会话").disabled(disabled || summary.available == false)
                if let onExtra {Button(action: onExtra) {Image(systemName: extraLabel == "拆出" ? "pin" : "pencil").font(.system(size: 11)).frame(width: 22,height: 22)}.accessibilityLabel(extraLabel).disabled(disabled)}
            }
            Text(titleOverride ?? summary.title).font(.system(size: 13,weight: .medium)).lineLimit(4).frame(maxWidth: .infinity,maxHeight: .infinity,alignment: .topLeading).foregroundStyle(summary.available == false ? Theme.textTertiary : Theme.textPrimary)
            HStack(spacing: 5) {
                if summary.available == false {Text("对话已结束或不可用").lineLimit(1)}
                else {
                    if let project = summary.project {Image(systemName: "folder");Text(project.split(whereSeparator: {$0 == "/" || $0 == "\\"}).last.map(String.init) ?? project).lineLimit(1)} else {Text(summary.kind == "私有对话" ? "独立私聊材料" : "共享会话").lineLimit(1)}
                    Spacer(minLength: 0)
                    if let branch = summary.branch {Image(systemName: "arrow.triangle.branch");Text(branch).lineLimit(1)}
                }
            }.font(.system(size: 10)).foregroundStyle(Theme.textTertiary)
        }.padding(10).buttonStyle(.plain).background {if framed {RoundedRectangle(cornerRadius: 12).fill(Theme.bgElevated).overlay {if let tint = rovaiCanvasTint(color) {RoundedRectangle(cornerRadius: 12).fill(tint.opacity(0.12))}}}}
        .clipShape(RoundedRectangle(cornerRadius: framed ? 12 : 16)).overlay {if framed {RoundedRectangle(cornerRadius: 12).stroke(Theme.surfaceStroke,lineWidth: 0.5)}}
    }
}

struct RovaiCanvasCard: View {
    let value: JSONValue
    let zoom: CGFloat
    let offset: CGSize
    let disabled: Bool
    let onOpen: () -> Void
    let onEdit: () -> Void
    let onMove: (Double,Double,Int) -> Void
    var isSelected = false
    var boardRevision = 0
    var onResize: ((Double,Double,Int) -> Void)?
    var catalog = RovaiCanvasCatalogSnapshot()
    var profiles: [JSONValue] = []
    var expanded = false
    var onExpand: () -> Void = {}
    var onMember: (JSONValue) -> Void = {_ in}
    var onDetach: (JSONValue) -> Void = {_ in}
    var onMemberMove: (JSONValue,Double,Double,Int) -> Void = {_,_,_,_ in}
    @GestureState private var drag = CGSize.zero
    @GestureState private var resizing = CGSize.zero
    @State private var moveBase: CGPoint?
    @State private var sizeBase: CGSize?
    @State private var moveRevision: Int?
    @State private var sizeRevision: Int?
    private var data: JSONValue {value["data"] ?? .null}
    private var kind: String {data.text("kind")}
    private var region: Bool {["folder","group","agent","custom"].contains(kind)}
    private var plain: Bool {kind == "conversation" && (data["objectReference"] == nil || data["objectReference"] == .null)}
    private var resolution: RovaiCanvasRegionResolution {resolveCanvasRegion(value,rows: catalog.rows,catalogComplete: catalog.complete,groups: catalog.groups,folders: catalog.folders)}
    private var group: JSONValue? {catalog.groups.first {$0.text("id") == data.text("folderGroupId")}}
    private var color: String {data.text("color").isEmpty ? group?.text("color") ?? "" : data.text("color")}
    private var title: String {
        if !data.text("title").isEmpty {return data.text("title")}
        if let group,!group.text("name").isEmpty {return group.text("name")}
        if let member = profiles.first(where: {$0.text("agentId") == data.text("agentId")}),!member.text("displayName").isEmpty {return member.text("displayName")}
        let path = data.text("path").isEmpty ? data.text("folderPath") : data.text("path")
        return path.split(whereSeparator: {$0 == "/" || $0 == "\\"}).last.map(String.init) ?? rovaiCanvasKindNames[kind] ?? "卡片"
    }
    private var height: Double {region ? canvasRegionHeight(data,resolution: resolution,expanded: expanded) : number("height",fallback: 220)}
    private var minimum: CGSize {switch kind {case "file": return CGSize(width: 240,height: 160);case "terminal": return CGSize(width: 260,height: 140);case "note": return CGSize(width: 140,height: 96);case "folder","group","agent","custom": return CGSize(width: 260,height: 160);default: return CGSize(width: 48,height: 48)}}
    private func number(_ key: String,fallback: Double) -> Double {if case .number(let value) = data[key] {return value};return fallback}
    private var moving: some Gesture {DragGesture(coordinateSpace: .global).onChanged {_ in guard !disabled else {return};if moveBase == nil {moveBase = CGPoint(x: number("x",fallback: 0),y: number("y",fallback: 0));moveRevision = boardRevision}}.updating($drag) {value,state,_ in if !disabled {state = value.translation}}.onEnded {gesture in if !disabled,let moveBase,let moveRevision {onMove(Double(moveBase.x+gesture.translation.width/zoom),Double(moveBase.y+gesture.translation.height/zoom),moveRevision)};moveBase = nil;moveRevision = nil}}
    var body: some View {
        VStack(alignment: .leading,spacing: 0) {
            if plain {RovaiCanvasSummaryTile(summary: canvasSummary(data["conversation"] ?? .null,catalog: catalog,profiles: profiles,fallbackTitle: title),color: color,onOpen: onOpen,onExtra: onEdit,disabled: disabled,catalogComplete: catalog.complete,framed: false,titleOverride: data.text("title").isEmpty ? nil : data.text("title")).gesture(moving)}
            else {
                HStack(spacing: 6) {Image(systemName: kind == "terminal" ? "terminal" : kind == "note" ? "note.text" : "square.stack");Text(title).font(.system(size: 13,weight: .semibold)).lineLimit(1);Spacer();if region {Text("\(resolution.members.count)").font(.caption2).foregroundStyle(Theme.textSecondary)};Button(action: onOpen) {Image(systemName: "arrow.up.right.square")}}.padding(.horizontal,12).frame(height: 40).contentShape(Rectangle()).gesture(moving)
                if data["collapsed"] != .bool(true) {
                    if region {regionGrid}
                    else {
                        if !data.text("content").isEmpty {Text(data.text("content")).font(.subheadline).lineLimit(8).padding(12)}
                        if !data.text("path").isEmpty {Text(data.text("path")).font(.caption).foregroundStyle(Theme.textSecondary).lineLimit(2).padding(12)}
                        if let ref = data["objectReference"],ref.isObject {Text(ref.text("taskId").isEmpty ? ref.text("missionId") : ref.text("taskId")).font(.caption).padding(12);Button("读取详情",action: onOpen).padding(.horizontal,12)}
                    }
                    Spacer(minLength: 0)
                    if isSelected,kind != "conversation",onResize != nil {resizeHandle}
                }
            }
        }.frame(width: min(20000,max(Double(minimum.width),number("width",fallback: 320)+Double(resizing.width/zoom))),height: data["collapsed"] == .bool(true) ? 40 : max(Double(minimum.height),height+Double(resizing.height/zoom)))
        .background {RoundedRectangle(cornerRadius: 16).fill(Theme.bgElevated).overlay {if let tint = rovaiCanvasTint(color) {RoundedRectangle(cornerRadius: 16).fill(tint.opacity(0.12))}}}
        .clipShape(RoundedRectangle(cornerRadius: 16)).hairlineBorder(16).overlay {if isSelected {RoundedRectangle(cornerRadius: 16).stroke(Theme.accent,lineWidth: 2)}}
        .contextMenu {Button("编辑卡片属性",systemImage: "pencil",action: onEdit)}
        .scaleEffect(zoom,anchor: .topLeading).offset(x: offset.width+number("x",fallback: 0)*zoom+drag.width,y: offset.height+number("y",fallback: 0)*zoom+drag.height)
        .onChange(of: disabled) {_,value in if value {moveBase = nil;moveRevision = nil;sizeBase = nil;sizeRevision = nil}}
    }
    @ViewBuilder private var regionGrid: some View {
        let visible = canvasRegionVisible(data,resolution: resolution,expanded: expanded),columns = canvasRegionColumns(data),rows = Int(ceil(Double(visible.count)/Double(columns)))
        VStack(alignment: .leading,spacing: 12) {
            ForEach(0..<rows,id: \.self) {row in HStack(spacing: 12) {
                ForEach(Array(visible.dropFirst(row*columns).prefix(columns).enumerated()),id: \.element.id) {column,member in
                    RovaiCanvasDraggableMember(member: member,summary: canvasSummary(member.reference,catalog: catalog,profiles: profiles,fallbackTitle: member.label),color: color,zoom: zoom,revision: boardRevision,origin: CGPoint(x: number("x",fallback: 0)+12+Double(column*236),y: number("y",fallback: 0)+52+Double(row*144)),disabled: disabled || member.available != true,catalogComplete: catalog.complete,onOpen: {onMember(member.reference)},onDetach: {onDetach(member.reference)},onMove: {x,y,captured in onMemberMove(member.reference,x,y,captured)}).frame(width: 224,height: 132)
                }
            }}

            if resolution.complete,visible.isEmpty {Text(kind == "custom" ? "拖入对话卡片进行分组" : "还没有关联对话").font(.caption).foregroundStyle(Theme.textSecondary)}
            let cap = (data["gridRows"]?.rvInt ?? 0)>0 ? (data["gridRows"]?.rvInt ?? 0)*columns : 24
            if resolution.members.count>cap {Button(expanded ? "收起成员" : "显示全部 \(resolution.members.count) 个成员",action: onExpand).font(.caption)}
            if let issue = resolution.issue {Text(issue).font(.caption2).foregroundStyle(Theme.textTertiary)}
        }.padding(12).frame(maxWidth: .infinity,alignment: .topLeading)
    }
    private var resizeHandle: some View {HStack {Spacer();Image(systemName: "arrow.down.right").font(.caption).foregroundStyle(Theme.textSecondary).frame(width: 32,height: 32).contentShape(Rectangle()).accessibilityLabel("调整卡片尺寸").gesture(DragGesture(coordinateSpace: .global).onChanged {_ in guard !disabled else {return};if sizeBase == nil {sizeBase = CGSize(width: number("width",fallback: 320),height: height);sizeRevision = boardRevision}}.updating($resizing) {value,state,_ in if !disabled {state = value.translation}}.onEnded {gesture in if !disabled,let sizeBase,let sizeRevision {onResize?(Double(min(20000,max(minimum.width,sizeBase.width+gesture.translation.width/zoom))),Double(min(20000,max(minimum.height,sizeBase.height+gesture.translation.height/zoom))),sizeRevision)};sizeBase = nil;sizeRevision = nil})}}
}

private struct RovaiCanvasDraggableMember: View {
    let member: RovaiCanvasRegionMember
    let summary: RovaiCanvasSummary
    let color: String
    let zoom: CGFloat
    let revision: Int
    let origin: CGPoint
    let disabled: Bool
    let catalogComplete: Bool
    let onOpen: () -> Void
    let onDetach: () -> Void
    let onMove: (Double,Double,Int) -> Void
    @GestureState private var translation = CGSize.zero
    @State private var captured: Int?
    @State private var start: CGPoint?
    @State private var startZoom: CGFloat = 1
    private var dragging: some Gesture {
        LongPressGesture(minimumDuration: 0.35).sequenced(before: DragGesture(coordinateSpace: .global)).onChanged {value in
            guard !disabled else {return}
            switch value {case .first(true): captured = revision;start = origin;startZoom = zoom
            case .second(true,_): if captured == nil {captured = revision;start = origin;startZoom = zoom}
            default: break}
        }.updating($translation) {value,state,_ in if !disabled,case .second(true,let drag?) = value {state = drag.translation}}
        .onEnded {value in defer {captured = nil;start = nil};guard !disabled,let captured,let start,case .second(true,let drag?) = value else {return};onMove(Double(start.x+drag.translation.width/startZoom),Double(start.y+drag.translation.height/startZoom),captured)}
    }
    var body: some View {
        RovaiCanvasSummaryTile(summary: summary,color: color,onOpen: onOpen,onExtra: onDetach,extraLabel: "拆出",disabled: disabled,catalogComplete: catalogComplete)
            .offset(x: translation.width/zoom,y: translation.height/zoom).gesture(dragging,including: disabled ? .none : .all)
            .onChange(of: disabled) {_,value in if value {captured = nil;start = nil}}
            .onChange(of: zoom) {_,_ in captured = nil;start = nil}
    }
}
