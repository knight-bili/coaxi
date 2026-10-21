package app.codeg.android.feature.main

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import app.codeg.android.core.designsystem.theme.CodegTheme
import kotlinx.serialization.json.*
import kotlin.math.*

/** CanvasDock: map and live zoom share one width; stored board coordinates remain unchanged. */
@Composable fun RovaiCanvasMiniMap(nodes:List<JsonObject>,viewport:RovaiCanvasViewport,stage:Size,onCenter:(Float,Float)->Unit){
    val color=CodegTheme.colors;val bounds=canvasBounds(nodes)
    val left=min(bounds.left,-viewport.x/viewport.zoom);val top=min(bounds.top,-viewport.y/viewport.zoom);val right=max(bounds.right,(stage.width-viewport.x)/viewport.zoom);val bottom=max(bounds.bottom,(stage.height-viewport.y)/viewport.zoom)
    Canvas(Modifier.width(180.dp).height(135.dp).background(color.bgElevated,RoundedCornerShape(14.dp)).border(1.dp,color.surfaceStroke,RoundedCornerShape(14.dp)).semantics{contentDescription="画布小地图，轻点定位"}.pointerInput(left,top,right,bottom){detectTapGestures{point->val scale=min((size.width-16f)/(right-left).coerceAtLeast(1f),(size.height-16f)/(bottom-top).coerceAtLeast(1f));val px=(size.width-(right-left)*scale)/2;val py=(size.height-(bottom-top)*scale)/2;onCenter((point.x-px)/scale+left,(point.y-py)/scale+top)}}){
        val scale=min((size.width-16f)/(right-left).coerceAtLeast(1f),(size.height-16f)/(bottom-top).coerceAtLeast(1f));val px=(size.width-(right-left)*scale)/2;val py=(size.height-(bottom-top)*scale)/2
        fun n(data:JsonObject,key:String,fallback:Float)=(data[key]as?JsonPrimitive)?.floatOrNull?:fallback
        nodes.forEach{node->val data=node["data"].coreObject();drawRect((rovaiCanvasTint(data.coreText("color"))?:color.textTertiary).copy(alpha=if(data.coreText("kind")in listOf("folder","group","agent","custom"))0.14f else .35f),Offset(px+(n(data,"x",0f)-left)*scale,py+(n(data,"y",0f)-top)*scale),Size(max(2f,n(data,"width",320f)*scale),max(2f,(if(data["collapsed"]==JsonPrimitive(true))52f else n(data,"height",220f))*scale)))}
        drawRect(color.accent.copy(alpha=.5f),Offset(px+(-viewport.x/viewport.zoom-left)*scale,py+(-viewport.y/viewport.zoom-top)*scale),Size(stage.width/viewport.zoom*scale,stage.height/viewport.zoom*scale),style=Stroke(1.5f))
    }
}
