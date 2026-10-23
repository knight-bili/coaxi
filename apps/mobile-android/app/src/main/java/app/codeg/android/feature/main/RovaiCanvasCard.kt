package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.detectDragGesturesAfterLongPress
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.codeg.android.core.designsystem.theme.CodegTheme
import kotlinx.serialization.json.*
import kotlin.math.*

private fun JsonObject.canvasFloat(key:String,fallback:Float=0f)=(get(key)as?JsonPrimitive)?.floatOrNull?:fallback
fun canvasRegionColumns(data:JsonObject,width:Float=data.canvasFloat("width",484f)):Int=data.coreLong("gridColumns").toInt().takeIf{it>0}?:max(1,floor((max(width-24,224f)+12)/236).toInt())
fun canvasRegionVisible(data:JsonObject,resolution:RovaiCanvasRegionResolution,expanded:Boolean):List<RovaiCanvasRegionMember>{val columns=canvasRegionColumns(data);val cap=if(data.coreLong("gridRows")>0)data.coreLong("gridRows").toInt()*columns else 24;return if(expanded)resolution.members else resolution.members.take(cap)}
fun canvasRegionHeight(data:JsonObject,resolution:RovaiCanvasRegionResolution,expanded:Boolean):Float{val count=canvasRegionVisible(data,resolution,expanded).size;val rows=ceil(count.toFloat()/canvasRegionColumns(data)).toInt();return max(data.canvasFloat("height",196f),64f+rows*132f+max(0,rows-1)*12f+36f)}

/** Original 224×132 summary, four-line title, model evidence and compact project/branch footer. */
@Composable fun RovaiCanvasSummaryTile(summary:RovaiCanvasSummary,color:String?,modifier:Modifier=Modifier,onOpen:()->Unit,onExtra:(()->Unit)?=null,extraLabel:String="编辑",disabled:Boolean=false,catalogComplete:Boolean=false,framed:Boolean=true,titleOverride:String?=null){
    val colors=CodegTheme.colors
    Column(modifier.clip(RoundedCornerShape(12.dp)).background((rovaiCanvasTint(color)?.copy(alpha=.12f)?.compositeOver(colors.bgElevated))?:colors.bgElevated).border(if(framed)1.dp else 0.dp,if(framed)colors.surfaceStroke else androidx.compose.ui.graphics.Color.Transparent,RoundedCornerShape(12.dp)).padding(10.dp)){
        Row(Modifier.fillMaxWidth(),horizontalArrangement=Arrangement.spacedBy(4.dp)){
            if(catalogComplete&&summary.active!=null)Text("●",color=if(summary.active)colors.accent else colors.textTertiary,style=MaterialTheme.typography.labelSmall)
            Text(summary.model?:summary.kind,Modifier.weight(1f),style=MaterialTheme.typography.labelSmall,maxLines=1,overflow=TextOverflow.Ellipsis,color=colors.textSecondary)
            TextButton(enabled=!disabled&&summary.available!=false,onClick=onOpen,contentPadding=PaddingValues(0.dp),modifier=Modifier.size(24.dp)){Text("↗")}
            if(onExtra!=null)TextButton(enabled=!disabled,onClick=onExtra,contentPadding=PaddingValues(0.dp),modifier=Modifier.height(24.dp)){Text(extraLabel,style=MaterialTheme.typography.labelSmall)}
        }
        Text(titleOverride?:summary.title,Modifier.weight(1f),style=MaterialTheme.typography.bodySmall,maxLines=4,overflow=TextOverflow.Ellipsis,color=if(summary.available==false)colors.textTertiary else colors.textPrimary)
        Row(Modifier.fillMaxWidth(),horizontalArrangement=Arrangement.spacedBy(4.dp)){
            Text(if(summary.available==false)"对话已结束或不可用"else summary.project?.trimEnd('/','\\')?.split('/','\\')?.lastOrNull()?:if(summary.kind=="私有对话")"独立私聊材料"else"共享会话",Modifier.weight(1f),style=MaterialTheme.typography.labelSmall,maxLines=1,overflow=TextOverflow.Ellipsis,color=colors.textTertiary)
            if(summary.available!=false&&summary.branch!=null)Text(summary.branch,Modifier.widthIn(max=88.dp),style=MaterialTheme.typography.labelSmall,maxLines=1,overflow=TextOverflow.Ellipsis,color=colors.textSecondary)
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable fun RovaiCanvasCard(node:JsonObject,zoom:Float,offset:Offset,boardRevision:Long,disabled:Boolean,isSelected:Boolean,catalog:RovaiCanvasCatalogSnapshot,profiles:List<JsonObject>,expanded:Boolean,onExpand:()->Unit,onOpen:()->Unit,onEdit:()->Unit,onMember:(JsonObject)->Unit,onDetach:(JsonObject)->Unit,onMutation:(JsonObject,Long)->Unit,onMemberMove:(JsonObject,Float,Float,Long)->Unit){
    val data=node["data"].coreObject();val id=node.coreText("id");val kind=data.coreText("kind");val density=LocalDensity.current.density;val colors=CodegTheme.colors;val region=kind in listOf("folder","group","agent","custom");val resolution=resolveCanvasRegion(node,catalog.rows,catalog.complete,catalog.groups,catalog.folders)
    val x=data.canvasFloat("x");val y=data.canvasFloat("y");val width=data.canvasFloat("width",320f);val height=if(region)canvasRegionHeight(data,resolution,expanded)else data.canvasFloat("height",220f);val collapsed=data["collapsed"]==JsonPrimitive(true)
    var drag by remember(id,x,y){mutableStateOf(Offset.Zero)};var resize by remember(id,width,height){mutableStateOf(Offset.Zero)};var dragRevision by remember(id){mutableStateOf<Long?>(null)};var resizeRevision by remember(id){mutableStateOf<Long?>(null)};val latestRevision by rememberUpdatedState(boardRevision)
    val minWidth=when(kind){"file"->240f;"terminal","folder","group","agent","custom"->260f;"note"->140f;else->48f};val minHeight=when(kind){"file","folder","group","agent","custom"->160f;"terminal"->140f;"note"->96f;else->48f}
    val handle=Modifier.pointerInput(id,disabled,zoom,x,y){detectDragGestures(onDragStart={dragRevision=if(disabled)null else latestRevision},onDragCancel={drag=Offset.Zero;dragRevision=null},onDragEnd={val revision=dragRevision;if(!disabled&&revision!=null)onMutation(buildJsonObject{put("type","move");putJsonArray("moves"){add(buildJsonObject{put("nodeId",id);put("x",x+drag.x);put("y",y+drag.y)})}},revision);drag=Offset.Zero;dragRevision=null}){change,delta->if(dragRevision!=null){change.consume();drag+=delta/(density*zoom)}}}
    val group=catalog.groups.firstOrNull{it.coreText("id")==data.coreText("folderGroupId")};val tint=data.coreText("color").ifEmpty{group?.coreText("color").orEmpty()};val plain=kind=="conversation"&&data["objectReference"] !is JsonObject
    Column(Modifier.graphicsLayer{translationX=offset.x+(x+drag.x)*density*zoom;translationY=offset.y+(y+drag.y)*density*zoom;scaleX=zoom;scaleY=zoom;transformOrigin=androidx.compose.ui.graphics.TransformOrigin(0f,0f)}.size((width+resize.x).coerceIn(minWidth,20000f).dp,if(collapsed)40.dp else (height+resize.y).coerceAtLeast(minHeight).dp).clip(RoundedCornerShape(16.dp)).background(rovaiCanvasTint(tint)?.copy(alpha=.12f)?.compositeOver(colors.bgElevated)?:colors.bgElevated).border(if(isSelected)2.dp else 1.dp,if(isSelected)colors.accent else colors.surfaceStroke,RoundedCornerShape(16.dp)).combinedClickable(onClick=onOpen,onLongClick=onEdit,onLongClickLabel="编辑卡片属性")){
        if(plain){RovaiCanvasSummaryTile(canvasSummary(data["conversation"].coreObject(),catalog,profiles,data.coreText("title")),tint,Modifier.fillMaxSize().then(handle),onOpen=onOpen,onExtra=onEdit,disabled=disabled,catalogComplete=catalog.complete,framed=false,titleOverride=data.coreText("title").takeIf{it.isNotEmpty()})}else{
            Row(Modifier.fillMaxWidth().height(40.dp).then(handle).padding(horizontal=12.dp),horizontalArrangement=Arrangement.spacedBy(6.dp)){
                Text(data.coreText("title").ifEmpty{group?.coreText("name").orEmpty()}.ifEmpty{profiles.firstOrNull{it.coreText("agentId")==data.coreText("agentId")}?.coreText("displayName").orEmpty()}.ifEmpty{rovaiCanvasKindNames[kind]?:"卡片"},Modifier.weight(1f),style=MaterialTheme.typography.titleSmall,maxLines=1,overflow=TextOverflow.Ellipsis)
                if(region)Text(resolution.members.size.toString(),style=MaterialTheme.typography.labelSmall,color=colors.textSecondary)
                TextButton(onClick=onOpen,contentPadding=PaddingValues(0.dp),modifier=Modifier.size(24.dp)){Text("↗")}
            }
            if(!collapsed){
                if(region){
                    val visible=canvasRegionVisible(data,resolution,expanded);val columns=canvasRegionColumns(data)
                    Column(Modifier.fillMaxWidth().padding(12.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
                        visible.chunked(columns).forEachIndexed{row,members->Row(horizontalArrangement=Arrangement.spacedBy(12.dp)){members.forEachIndexed{column,member->
                            var translation by remember(id,member.key){mutableStateOf(Offset.Zero)};var captured by remember(id,member.key){mutableStateOf<Long?>(null)}
                            val gesture=Modifier.pointerInput(id,member.key,disabled,zoom,x,y){detectDragGesturesAfterLongPress(onDragStart={captured=if(disabled||member.available!=true)null else latestRevision},onDragCancel={translation=Offset.Zero;captured=null},onDragEnd={val revision=captured;if(!disabled&&revision!=null)onMemberMove(member.reference,x+12+column*236+translation.x,y+52+row*144+translation.y,revision);translation=Offset.Zero;captured=null}){change,delta->if(captured!=null){change.consume();translation+=delta/(density*zoom)}}}
                            RovaiCanvasSummaryTile(canvasSummary(member.reference,catalog,profiles,member.label),tint,Modifier.size(224.dp,132.dp).graphicsLayer{translationX=translation.x*density;translationY=translation.y*density}.then(gesture),onOpen={onMember(member.reference)},onExtra={onDetach(member.reference)},extraLabel="拆出",disabled=disabled||member.available!=true,catalogComplete=catalog.complete)
                        }}}

                        if(resolution.complete&&visible.isEmpty())Text(if(kind=="custom")"拖入对话卡片进行分组"else"还没有关联对话",style=MaterialTheme.typography.bodySmall,color=colors.textSecondary)
                        val cap=if(data.coreLong("gridRows")>0)data.coreLong("gridRows").toInt()*columns else 24
                        if(resolution.members.size>cap)TextButton(onClick=onExpand){Text(if(expanded)"收起成员"else"显示全部 ${resolution.members.size} 个成员")}
                        resolution.issue?.let{Text(it,style=MaterialTheme.typography.labelSmall,color=colors.textTertiary)}
                    }
                }else{
                    if(data.coreText("content").isNotEmpty())Text(data.coreText("content"),Modifier.padding(12.dp),maxLines=8,overflow=TextOverflow.Ellipsis,color=colors.textSecondary)
                    if(data.coreText("path").isNotEmpty())Text(data.coreText("path"),Modifier.padding(12.dp),maxLines=2,overflow=TextOverflow.Ellipsis,color=colors.textSecondary)
                    if(data["objectReference"]is JsonObject){val ref=data["objectReference"].coreObject();Text(ref.coreText("taskId").ifEmpty{ref.coreText("missionId")},Modifier.padding(12.dp),style=MaterialTheme.typography.bodySmall);TextButton(onClick=onOpen){Text(stringResource(R.string.rovai_fusion_e4c16a6055c3f3bd))}}
                }
                Spacer(Modifier.weight(1f))
                if(isSelected&&kind!="conversation")Row(Modifier.fillMaxWidth(),horizontalArrangement=Arrangement.End){Text("↘",Modifier.size(32.dp).pointerInput(id,disabled,zoom,width,height){detectDragGestures(onDragStart={resizeRevision=if(disabled)null else latestRevision},onDragCancel={resize=Offset.Zero;resizeRevision=null},onDragEnd={val revision=resizeRevision;if(!disabled&&revision!=null)onMutation(buildJsonObject{put("type","update");put("nodeId",id);putJsonObject("patch"){put("width",(width+resize.x).coerceIn(minWidth,20000f));put("height",(height+resize.y).coerceIn(minHeight,20000f))}},revision);resize=Offset.Zero;resizeRevision=null}){change,delta->if(resizeRevision!=null){change.consume();resize+=delta/(density*zoom)}}}.padding(6.dp),color=colors.textSecondary)}
            }
        }
    }
}
