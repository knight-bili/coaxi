package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import app.coaxi.android.core.designsystem.theme.CoaxiTheme
import kotlinx.serialization.json.*

private data class MissionMentionRange(val start:Int,val end:Int,val agentId:String)
/** Metadata originates only in Core segments or an explicit member picker. Pasted @names stay text. */
private data class MissionDescriptionDocument(val text:String,val mentions:List<MissionMentionRange>){
    fun content():JsonArray {
        val parts=mutableListOf<JsonElement>();var cursor=0
        for(mention in mentions.sortedBy{it.start}){require(mention.start>=cursor&&mention.end<=text.length);if(mention.start>cursor)parts+=buildJsonObject{put("kind","text");put("text",text.substring(cursor,mention.start))};parts+=buildJsonObject{put("kind","member_mention");put("agentId",mention.agentId)};cursor=mention.end}
        if(cursor<text.length||parts.isEmpty())parts+=buildJsonObject{put("kind","text");put("text",text.substring(cursor))}
        return JsonArray(parts)
    }
    fun snap(offset:Int):Int {val clamped=offset.coerceIn(0,text.length);val token=mentions.firstOrNull{clamped>it.start&&clamped<it.end}?:return clamped;return if(clamped-token.start<(token.end-token.start)/2)token.start else token.end}
    fun edit(next:String):Pair<MissionDescriptionDocument,Int>{
        var start=0;while(start<text.length&&start<next.length&&text[start]==next[start])start++
        var suffix=0;while(suffix<text.length-start&&suffix<next.length-start&&text[text.lastIndex-suffix]==next[next.lastIndex-suffix])suffix++
        val oldEnd=text.length-suffix;val inserted=next.substring(start,next.length-suffix)
        val touched=mentions.filter{if(start==oldEnd)start>it.start&&start<it.end else it.start<oldEnd&&it.end>start}
        val from=minOf(start,touched.minOfOrNull{it.start}?:start);val to=maxOf(oldEnd,touched.maxOfOrNull{it.end}?:oldEnd)
        val changed=text.substring(0,from)+inserted+text.substring(to);val delta=inserted.length-(to-from)
        val remaining=mentions.filter{it !in touched}.map{if(it.start>=to)it.copy(start=it.start+delta,end=it.end+delta)else it}
        return MissionDescriptionDocument(changed,remaining) to (from+inserted.length)
    }
    fun insert(selection:TextRange,id:String,label:String):Pair<MissionDescriptionDocument,Int>{
        val from=snap(minOf(selection.start,selection.end));val to=snap(maxOf(selection.start,selection.end));val marker="@$label"
        val delta=marker.length-(to-from);val remaining=mentions.filter{it.end<=from||it.start>=to}.map{if(it.start>=to)it.copy(start=it.start+delta,end=it.end+delta)else it}
        return MissionDescriptionDocument(text.substring(0,from)+marker+text.substring(to),(remaining+MissionMentionRange(from,from+marker.length,id)).sortedBy{it.start}) to (from+marker.length)
    }
    companion object {
        fun read(content:JsonArray,names:Map<String,String>):MissionDescriptionDocument{
            val raw=StringBuilder();val tokens=mutableListOf<MissionMentionRange>()
            for(value in content){val segment=value.coreObject();when(segment.coreText("kind")){
                "text"->raw.append(segment.coreText("text"))
                "member_mention"->{val id=segment.coreText("agentId");require(id.isNotBlank());val start=raw.length;raw.append("@").append(names[id]?:"不可用队员");tokens+=MissionMentionRange(start,raw.length,id)}
                else->throw IllegalArgumentException("使命描述包含当前编辑器不支持的内容；原草稿保留")
            }};return MissionDescriptionDocument(raw.toString(),tokens)
        }
    }
}

/** Native counterpart of MissionDescriptionComposer: inline, indivisible member mentions. */
@Composable fun CoaxiMissionDescriptionInput(value:JsonArray,members:List<JsonObject>,disabled:Boolean,onChange:(JsonArray)->Unit){
    val accent=CoaxiTheme.colors.accent
    val names=members.associate{it.coreText("agentId") to it.coreText("displayName")}
    val initial=remember{runCatching{MissionDescriptionDocument.read(value,names)}}
    var document by remember{mutableStateOf(initial.getOrElse{MissionDescriptionDocument("",emptyList())})};var field by remember{mutableStateOf(TextFieldValue(document.text))};var picker by remember{mutableStateOf(false)}
    var issue by remember{mutableStateOf(initial.exceptionOrNull()?.message)}
    fun annotate(doc:MissionDescriptionDocument):AnnotatedString=buildAnnotatedString{append(doc.text);doc.mentions.forEach{addStyle(SpanStyle(color=accent),it.start,it.end)}}
    fun commit(doc:MissionDescriptionDocument,cursor:Int){document=doc;field=TextFieldValue(annotate(doc),TextRange(cursor.coerceIn(0,doc.text.length)));onChange(doc.content())}
    LaunchedEffect(value,names){try{val next=MissionDescriptionDocument.read(value,names);if(next!=document){document=next;field=TextFieldValue(annotate(next),TextRange(next.snap(field.selection.end)))}}catch(error:Exception){issue=error.message}}
    Column(verticalArrangement=Arrangement.spacedBy(6.dp)){
        OutlinedTextField(value=field,onValueChange={next->if(next.text==document.text){field=next.copy(annotatedString=annotate(document),selection=TextRange(document.snap(next.selection.start),document.snap(next.selection.end)))}else{val (changed,cursor)=document.edit(next.text);document=changed;field=if(changed.text==next.text)next.copy(annotatedString=annotate(changed),selection=TextRange(changed.snap(next.selection.start),changed.snap(next.selection.end)))else TextFieldValue(annotate(changed),TextRange(cursor));onChange(changed.content())}},modifier=Modifier.fillMaxWidth(),label={Text(stringResource(R.string.coaxi_fusion_96a28af327e29417))},enabled=!disabled&&issue==null,minLines=3,maxLines=8)
        TextButton(enabled=!disabled&&issue==null,onClick={picker=true}){Text(stringResource(R.string.coaxi_fusion_1e2d3d2a02fb53e8))}
        issue?.let{Text(it,color=CoaxiTheme.colors.danger,style=MaterialTheme.typography.bodySmall)}
    }
    if(picker)AlertDialog(onDismissRequest={picker=false},title={Text(stringResource(R.string.coaxi_fusion_1e2d3d2a02fb53e8))},text={LazyColumn(Modifier.heightIn(max=360.dp)){items(members.filter{it.coreText("presence")=="present"},key={it.coreText("agentId")}){member->TextButton(enabled=!disabled,onClick={val (changed,cursor)=document.insert(field.selection,member.coreText("agentId"),member.coreText("displayName"));commit(changed,cursor);picker=false}){Text(member.coreText("displayName"))}}}},confirmButton={TextButton(onClick={picker=false}){Text(stringResource(R.string.coaxi_fusion_3fd47edce45b3603))}})
}

fun missionDescriptionMemberIds(value:JsonArray):Set<String> =value.map{it.coreObject()}.filter{it.coreText("kind")=="member_mention"}.map{it.coreText("agentId")}.toSet()
fun missionDescriptionDisplay(value:JsonArray,members:List<JsonObject>):String=MissionDescriptionDocument.read(value,members.associate{it.coreText("agentId") to it.coreText("displayName")}).text
