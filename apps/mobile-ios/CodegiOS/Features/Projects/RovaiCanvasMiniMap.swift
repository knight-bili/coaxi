import SwiftUI

/// Original CanvasDock proportion and matching map/zoom width, in immutable board units.
struct RovaiCanvasMiniMap: View {
    let nodes: [JSONValue]
    let viewport: RovaiCanvasViewport
    let stage: CGSize
    let onCenter: (Double,Double) -> Void
    private var extent: RovaiCanvasBounds {
        let bounds = canvasBounds(nodes)
        return .init(left: min(bounds.left,-viewport.x/viewport.zoom),top: min(bounds.top,-viewport.y/viewport.zoom),right: max(bounds.right,(Double(stage.width)-viewport.x)/viewport.zoom),bottom: max(bounds.bottom,(Double(stage.height)-viewport.y)/viewport.zoom))
    }
    var body: some View {
        GeometryReader {geometry in
            let bounds = extent, scale = min((Double(geometry.size.width)-16)/bounds.width,(Double(geometry.size.height)-16)/bounds.height)
            let px = (Double(geometry.size.width)-bounds.width*scale)/2, py = (Double(geometry.size.height)-bounds.height*scale)/2
            Canvas {context,_ in
                for node in nodes {
                    let data = node["data"] ?? .null
                    let region = ["folder","group","agent","custom"].contains(data.text("kind"))
                    let rect = CGRect(x: px+(number(data,"x",0)-bounds.left)*scale,y: py+(number(data,"y",0)-bounds.top)*scale,width: max(2,number(data,"width",320)*scale),height: max(2,(data["collapsed"] == .bool(true) ? 40 : number(data,"height",220))*scale))
                    context.fill(Path(rect),with: .color((rovaiCanvasTint(data.text("color")) ?? Theme.textTertiary).opacity(region ? 0.14 : 0.35)))
                }
                let visible = CGRect(x: px+(-viewport.x/viewport.zoom-bounds.left)*scale,y: py+(-viewport.y/viewport.zoom-bounds.top)*scale,width: Double(stage.width)/viewport.zoom*scale,height: Double(stage.height)/viewport.zoom*scale)
                context.stroke(Path(visible),with: .color(Theme.accent.opacity(0.5)),lineWidth: 1.5)
            }.contentShape(Rectangle()).gesture(SpatialTapGesture().onEnded {event in onCenter((Double(event.location.x)-px)/scale+bounds.left,(Double(event.location.y)-py)/scale+bounds.top)})
        }.frame(width: 180,height: 135).background(Theme.bgElevated,in: RoundedRectangle(cornerRadius: 14)).hairlineBorder(14).accessibilityLabel("画布小地图，轻点定位")
    }
    private func number(_ data: JSONValue,_ key: String,_ fallback: Double) -> Double {if case .number(let value) = data[key] {return value};return fallback}
}
