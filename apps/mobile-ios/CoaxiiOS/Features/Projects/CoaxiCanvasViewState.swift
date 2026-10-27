import Foundation

/// Advisory presentation facts; scoped to the authenticated Host/editor and Core board UUID.
struct CoaxiCanvasViewport: Equatable {
    var x: Double = 0, y: Double = 0, zoom: Double = 1
    var minimap = true
    var expandedRegions: [String] = []
    func document(board: String) throws -> String {String(decoding: try JSONEncoder().encode(JSONValue.object(["version": .number(1), "boardId": .string(board), "viewport": .object(["x": .number(x), "y": .number(y), "zoom": .number(zoom)]), "minimap": .bool(minimap),"expandedRegions": .array(Array(Set(expandedRegions.filter {!$0.isEmpty})).sorted().map(JSONValue.string))])), as: UTF8.self)}
    static func parse(_ raw: String, board: String) throws -> CoaxiCanvasViewport {
        if raw.isEmpty {return .init()}
        guard let value = JSONValue.parse(raw), value["version"] == .number(1), value.text("boardId") == board, case .number(let x) = value["viewport"]?["x"], case .number(let y) = value["viewport"]?["y"], case .number(let zoom) = value["viewport"]?["zoom"], x.isFinite, y.isFinite, zoom.isFinite else {throw CoaxiHostFailure(code: "原画布视图记录无法读取")}
        return .init(x: x, y: y, zoom: min(2,max(0.1,zoom)), minimap: value["minimap"] != .bool(false),expandedRegions: Array(Set((value["expandedRegions"]?.rvArray ?? []).compactMap(\.stringValue).filter {!$0.isEmpty})).sorted())
    }
}
struct CoaxiCanvasBounds {let left: Double, top: Double, right: Double, bottom: Double;var width: Double {max(1,right-left)};var height: Double {max(1,bottom-top)}}
func canvasBounds(_ nodes: [JSONValue]) -> CoaxiCanvasBounds {
    guard !nodes.isEmpty else {return .init(left: 0,top: 0,right: 1200,bottom: 720)}
    func n(_ node: JSONValue,_ key: String,_ fallback: Double) -> Double {if case .number(let value) = node["data"]?[key],value.isFinite {return value};return fallback}
    return .init(left: nodes.map {n($0,"x",0)}.min() ?? 0, top: nodes.map {n($0,"y",0)}.min() ?? 0, right: nodes.map {n($0,"x",0)+n($0,"width",320)}.max() ?? 1200, bottom: nodes.map {n($0,"y",0)+($0["data"]?["collapsed"] == .bool(true) ? 40 : n($0,"height",220))}.max() ?? 720)
}
func canvasFit(_ nodes: [JSONValue],width: Double,height: Double) -> CoaxiCanvasViewport {
    let bounds = canvasBounds(nodes), zoom = min(2,max(0.1,min(max(1,width-96)/bounds.width,max(1,height-96)/bounds.height)))
    return .init(x: (width-bounds.width*zoom)/2-bounds.left*zoom,y: (height-bounds.height*zoom)/2-bounds.top*zoom,zoom: zoom)
}
/// Original packLayout: tallest first, stable ID tie, 48-point gap and 2400 board-unit shelf.
func canvasPackedMoves(_ nodes: [JSONValue]) -> [JSONValue] {
    func n(_ node: JSONValue,_ key: String,_ fallback: Double) -> Double {if case .number(let value) = node["data"]?[key] {return value};return fallback}
    func height(_ node: JSONValue) -> Double {node["data"]?["collapsed"] == .bool(true) ? 40 : n(node,"height",220)}
    var x = 0.0,y = 0.0,shelf = 0.0,moves: [JSONValue] = []
    for node in nodes.sorted(by: {a,b in height(a) == height(b) ? a.text("id") < b.text("id") : height(a)>height(b)}) {
        let width = n(node,"width",320), h = height(node)
        if x>0,x+width>2400 {y += shelf+48;x = 0;shelf = 0}
        if n(node,"x",0) != x || n(node,"y",0) != y {moves.append(.object(["nodeId": node["id"] ?? .null,"x": .number(x),"y": .number(y)]))}
        x += width+48;shelf = max(shelf,h)
    }
    return moves
}
