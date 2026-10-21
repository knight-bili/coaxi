import SwiftUI

/// Target controls for the original per-container layout; groups stay at the top level.
struct RovaiProjectLayoutRow: View {
    let entry: RovaiProjectLayoutItem
    let layout: RovaiProjectLayout
    let groups: [JSONValue]
    let folders: [String:RovaiCanvasFolderRelation]
    let blocked: Bool
    let onMove: (RovaiProjectLayoutItem,RovaiProjectLayoutLocation) -> Void
    private var location: RovaiProjectLayoutLocation? {layout.locate(entry)}
    private var project: Bool {entry.kind == "project"}
    private var relation: RovaiCanvasFolderRelation? {folders[entry.target]}
    private var disabled: Bool {blocked || project && (relation?.complete != true || relation?.parentPath != nil)}
    var body: some View {
        if let location {VStack(alignment: .leading,spacing: 5) {
            let count = location.groupId.map {layout.members[$0]?.count ?? 0} ?? layout.top.count
            HStack(spacing: 8) {
                Text(project ? entry.target : groups.first {$0.text("id") == entry.target}?.text("name") ?? "原分组暂不可用").font(project ? .caption : .headline).lineLimit(2)
                Spacer()
                Button {onMove(entry,.init(groupId: location.groupId,index: location.index-1))} label: {Label("上移",systemImage: "arrow.up")}.labelStyle(.iconOnly).disabled(disabled || location.index == 0)
                Button {onMove(entry,.init(groupId: location.groupId,index: location.index+1))} label: {Label("下移",systemImage: "arrow.down")}.labelStyle(.iconOnly).disabled(disabled || location.index>=count-1)
            }
            if project {
                Picker("所属分组",selection: Binding(get: {location.groupId ?? ""},set: {target in guard target != (location.groupId ?? "") else {return};onMove(entry,.init(groupId: target.isEmpty ? nil : target,index: target.isEmpty ? layout.top.count : layout.members[target]?.count ?? 0))})) {Text("顶层").tag("");ForEach(groups.map(RovaiRow.init(value:))) {row in Text(row.value.text("name")).tag(row.id)}}.disabled(disabled)
                if relation?.parentPath != nil {Text("此工作树跟随主工程归组。").font(.caption).foregroundStyle(Theme.textSecondary)}
            }
        }.padding(.leading,location.groupId == nil ? 0 : 20)}
    }
}
