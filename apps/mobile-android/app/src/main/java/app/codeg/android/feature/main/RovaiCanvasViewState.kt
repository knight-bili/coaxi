package app.codeg.android.feature.main

import kotlinx.serialization.json.*
import kotlin.math.*

/** Advisory UI state only, under the authenticated Host/editor and Core board UUID. */
data class RovaiCanvasViewport(val x:Float=0f,val y:Float=0f,val zoom:Float=1f,val minimap:Boolean=true,val expandedRegions:List<String> = emptyList()){
    fun document(board:String)=buildJsonObject{put("version",1);put("boardId",board);putJsonObject("viewport"){put("x",x);put("y",y);put("zoom",zoom)};put("minimap",minimap);put("expandedRegions",JsonArray(expandedRegions.filter{it.isNotBlank()}.distinct().map(::JsonPrimitive)))}.toString()
    companion object {
        fun parse(raw: String, board: String): RovaiCanvasViewport {
            if (raw.isEmpty()) return RovaiCanvasViewport()

            val value = Json.parseToJsonElement(raw).coreObject()
            require(value["version"] == JsonPrimitive(1) && value.coreText("boardId") == board) {
                "原画布视图记录无法读取"
            }
            val viewport = value["viewport"].coreObject()

            fun number(key: String): Float =
                (viewport[key] as? JsonPrimitive)
                    ?.takeUnless { it.isString }
                    ?.floatOrNull
                    ?.takeIf { it.isFinite() }
                    ?: error("原画布视图位置无效")

            return RovaiCanvasViewport(
                x = number("x"),
                y = number("y"),
                zoom = number("zoom").coerceIn(0.1f, 2f),
                minimap = value["minimap"] != JsonPrimitive(false),
                expandedRegions = (value["expandedRegions"] as? JsonArray)
                    ?.mapNotNull { item -> (item as? JsonPrimitive)?.contentOrNull?.takeIf { it.isNotBlank() } }
                    ?.distinct()
                    .orEmpty()
            )
        }
    }
}
data class RovaiCanvasBounds(val left:Float,val top:Float,val right:Float,val bottom:Float){val width:Float get()=max(1f,right-left);val height:Float get()=max(1f,bottom-top)}
fun canvasBounds(nodes:List<JsonObject>):RovaiCanvasBounds{
    if(nodes.isEmpty())return RovaiCanvasBounds(0f,0f,1200f,720f)
    fun n(node:JsonObject,key:String,fallback:Float)=(node["data"].coreObject()[key]as?JsonPrimitive)?.floatOrNull?.takeIf{it.isFinite()}?:fallback
    return RovaiCanvasBounds(nodes.minOf{n(it,"x",0f)},nodes.minOf{n(it,"y",0f)},nodes.maxOf{n(it,"x",0f)+n(it,"width",320f)},nodes.maxOf{n(it,"y",0f)+if(it["data"].coreObject()["collapsed"]==JsonPrimitive(true))40f else n(it,"height",220f)})
}
fun canvasFit(nodes:List<JsonObject>,width:Float,height:Float):RovaiCanvasViewport{
    val bounds=canvasBounds(nodes);val zoom=min(max(1f,width-96f)/bounds.width,max(1f,height-96f)/bounds.height).coerceIn(.1f,2f)
    return RovaiCanvasViewport((width-bounds.width*zoom)/2-bounds.left*zoom,(height-bounds.height*zoom)/2-bounds.top*zoom,zoom)
}
/** Original packLayout shelf order: height descending, then stable node ID; 48 gap, 2400 row width. */
fun canvasPackedMoves(nodes:List<JsonObject>):JsonArray{
    fun n(node:JsonObject,key:String,fallback:Double)=(node["data"].coreObject()[key]as?JsonPrimitive)?.doubleOrNull?:fallback
    fun height(node:JsonObject)=if(node["data"].coreObject()["collapsed"]==JsonPrimitive(true))40.0 else n(node,"height",220.0)
    var x=0.0;var y=0.0;var shelf=0.0
    return JsonArray(nodes.sortedWith(compareByDescending<JsonObject>{height(it)}.thenBy{it.coreText("id")}).mapNotNull{node->val width=n(node,"width",320.0);val height=height(node);if(x>0&&x+width>2400){y+=shelf+48;x=0.0;shelf=0.0};val move=if(n(node,"x",0.0)!=x||n(node,"y",0.0)!=y)buildJsonObject{put("nodeId",node.coreText("id"));put("x",x);put("y",y)}else null;x+=width+48;shelf=max(shelf,height);move})
}
