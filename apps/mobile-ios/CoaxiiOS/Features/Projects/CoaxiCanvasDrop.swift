import Foundation

/// Original computeDropHint/applyDrop. Region membership edits never move domain conversations or files.
func canvasDropMutation(source: JSONValue,member: JSONValue? = nil,x: Double,y: Double,nodes: [JSONValue],memberTransferVersion: Int) throws -> JSONValue? {
    let data = source["data"] ?? .null,id = source.text("id"),reference = member ?? data["conversation"]
    func move() -> JSONValue {if let member {return .object(["type": .string("detach"),"regionId": .string(id),"conversation": member,"x": .number(x),"y": .number(y)])};return .object(["type": .string("move"),"moves": .array([.object(["nodeId": .string(id),"x": .number(x),"y": .number(y)])])])}
    guard member != nil || data.text("kind") == "conversation" && (data["objectReference"] == nil || data["objectReference"] == .null),let reference else {return move()}
    func n(_ node: JSONValue,_ key: String,_ fallback: Double) -> Double {if case .number(let value) = node["data"]?[key] {return value};return fallback}
    func hit(_ node: JSONValue) -> Bool {x+112>=n(node,"x",0) && x+112<=n(node,"x",0)+n(node,"width",320) && y+66>=n(node,"y",0) && y+66<=n(node,"y",0)+n(node,"height",220)}
    let region = nodes.last {candidate in ["folder","group","agent","custom"].contains(candidate["data"]?.text("kind") ?? "") && hit(candidate)}
    if let region,member != nil,region.text("id") == id {return nil}
    if let region,region["data"]?.text("kind") != "custom" {return move()}
    let key = CoaxiCanvasReferenceChoice(reference: reference,label: "").id
    let pin = region == nil ? nodes.last {candidate in let d = candidate["data"] ?? .null;return candidate.text("id") != id && d.text("kind") == "conversation" && (d["objectReference"] == nil || d["objectReference"] == .null) && d["conversation"]?.isObject == true && CoaxiCanvasReferenceChoice(reference: d["conversation"] ?? .null,label: "").id != key && hit(candidate)} : nil
    guard region != nil || pin != nil else {return move()}
    guard member == nil || memberTransferVersion == 1 else {throw CoaxiHostFailure(code: "当前Host尚未提供原子成员转移，请重新连接更新后的工作区")}
    var references = [reference],consume: [JSONValue] = []
    if let pin {references.insert(pin["data"]?["conversation"] ?? .null,at: 0);consume.append(pin["id"] ?? .null)}
    if member == nil {consume.append(source["id"] ?? .null)}
    var result: [String:JSONValue] = ["type": .string("group"),"targetNodeId": region?["id"] ?? .null,"newRegion": pin.map {.object(["kind": .string("custom"),"x": .number(n($0,"x",0)-12),"y": .number(n($0,"y",0)-52),"width": .number(484),"height": .number(196)])} ?? .null,"conversations": .array(references),"consumeNodeIds": .array(consume)]
    if let member {result["sourceMembers"] = .array([.object(["regionId": .string(id),"conversation": member])])}
    return .object(result)
}
/// Fit/packing/map hit tests use the current rendered region footprint without persisting a derived size.
func canvasPresentationNodes(_ nodes: [JSONValue],catalog: CoaxiCanvasCatalogSnapshot,expanded: Set<String>) -> [JSONValue] {
    nodes.map {node in var data = node["data"]?.rvObject ?? [:];let region = ["folder","group","agent","custom"].contains(data["kind"]?.stringValue ?? "");let height: Double
        if data["collapsed"] == .bool(true) {height = 40} else if region {height = canvasRegionHeight(.object(data),resolution: resolveCanvasRegion(node,rows: catalog.rows,catalogComplete: catalog.complete,groups: catalog.groups,folders: catalog.folders),expanded: expanded.contains(node.text("id")))} else {return node}
        data["height"] = .number(height);var result = node.rvObject;result["data"] = .object(data);return .object(result)
    }
}
