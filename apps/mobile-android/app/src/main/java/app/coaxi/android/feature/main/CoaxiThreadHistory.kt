package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import app.coaxi.android.core.designsystem.component.GlassRow
import app.coaxi.android.core.designsystem.theme.CoaxiTheme
import app.coaxi.android.core.network.CoaxiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*

@Composable fun CoaxiThreadHistory(model:CoaxiWorkspaceViewModel,threadId:String,anchor:String?=null,onClose:()->Unit){
    val generation=model.connectionGeneration;val scope=rememberCoroutineScope();val listState=rememberLazyListState()
    var rows by remember{mutableStateOf<List<JsonObject>>(emptyList())};var before by remember{mutableStateOf<Long?>(null)}
    var watermark by remember{mutableLongStateOf(0)};var query by remember{mutableStateOf("")};var frozenQuery by remember{mutableStateOf("")}
    var matchIndex by remember{mutableIntStateOf(0)};var total by remember{mutableIntStateOf(0)};var focused by remember{mutableStateOf(anchor)}
    var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)}
    suspend fun around(id:String){val value=model.query(CoaxiOperation.THREAD_MESSAGES_AROUND,buildJsonObject{put("threadId",threadId);put("messageId",id)}).coreObject();require(generation==model.connectionGeneration);rows=value["messages"].coreArray();focused=id;before=rows.minOfOrNull{it.coreLong("sequence")};watermark=value.coreLong("throughGlobalSequence");if(value["sourceAvailable"]!=JsonPrimitive(true))issue="原消息已不可用，已有引用快照仍保留"}
    suspend fun page(cursor:Long){val value=model.query(CoaxiOperation.THREAD_MESSAGES_PAGE,buildJsonObject{put("threadId",threadId);put("beforeSequence",cursor);put("throughGlobalSequence",watermark);put("limit",50)}).coreObject();require(generation==model.connectionGeneration);rows=(value["messages"].coreArray()+rows).distinctBy{it.coreText("id")}.sortedBy{it.coreLong("sequence")}.take(200);before=(value["nextBeforeSequence"] as?JsonPrimitive)?.longOrNull;watermark=value.coreLong("throughGlobalSequence")}
    suspend fun find(text:String,index:Int){val value=model.query(CoaxiOperation.THREAD_MESSAGES_FIND,buildJsonObject{put("threadId",threadId);put("query",text);put("selectedMatchIndex",index)}).coreObject();require(generation==model.connectionGeneration);frozenQuery=text;total=value.coreLong("totalMatchCount").toInt();matchIndex=value.coreLong("selectedMatchIndex").toInt();val match=value["match"].coreObject();if(match.coreText("messageId").isNotEmpty())around(match.coreText("messageId"))else{rows=emptyList();before=null;focused=null}}
    fun work(block:suspend ()->Unit){if(busy)return;busy=true;issue=null;scope.launch{try{block()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}
    LaunchedEffect(threadId,anchor){busy=true;try{if(anchor!=null)around(anchor)else page(9007199254740991)}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    LaunchedEffect(focused,rows){val index=rows.indexOfFirst{it.coreText("id")==focused};if(index>=0)listState.scrollToItem(index+if(before!=null&&before!!>0)1 else 0)}
    Dialog(onDismissRequest=onClose,properties=DialogProperties(usePlatformDefaultWidth=false)){Surface(Modifier.fillMaxSize(),color=CoaxiTheme.colors.bgElevated){Column(Modifier.fillMaxSize().padding(12.dp),verticalArrangement=Arrangement.spacedBy(8.dp)){
        Row{Text(stringResource(R.string.coaxi_fusion_d13a766345fcad91),Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(enabled=!busy,onClick={work{rows=emptyList();before=null;focused=null;frozenQuery="";watermark=0;page(9007199254740991)}}){Text(stringResource(R.string.coaxi_fusion_569af1a47dbdf053))};TextButton(onClick=onClose){Text(stringResource(R.string.coaxi_fusion_3fd47edce45b3603))}}
        Row{OutlinedTextField(query,{query=it},Modifier.weight(1f),label={Text(stringResource(R.string.coaxi_fusion_675cdf65d059a70b))},singleLine=true);TextButton(enabled=!busy&&query.isNotBlank(),onClick={work{find(query,0)}}){Text(stringResource(R.string.coaxi_fusion_44ce7ae909bbb28b))}}
        if(frozenQuery.isNotEmpty())Row{Text("${if(total==0)0 else matchIndex+1} / $total",Modifier.weight(1f));TextButton(enabled=!busy&&matchIndex>0,onClick={work{find(frozenQuery,matchIndex-1)}}){Text(stringResource(R.string.coaxi_fusion_b7e4aa2f061b3967))};TextButton(enabled=!busy&&matchIndex+1<total,onClick={work{find(frozenQuery,matchIndex+1)}}){Text(stringResource(R.string.coaxi_fusion_72ac8841c5f4b21f))}}
        issue?.let{Text(it,color=CoaxiTheme.colors.danger)}
        LazyColumn(Modifier.weight(1f),state=listState,verticalArrangement=Arrangement.spacedBy(10.dp)){
            before?.takeIf{it>0}?.let{cursor->item{TextButton(enabled=!busy,onClick={work{page(cursor)}}){Text(stringResource(R.string.coaxi_fusion_37781f751fc36540))}}}
            items(rows,key={it.coreText("id")}){message->GlassRow(Modifier.fillMaxWidth(),selected=message.coreText("id")==focused){Text(message.coreText("authorDisplayName").ifEmpty{message.coreText("authorType")},style=MaterialTheme.typography.labelMedium);Text(message.coreText("createdAt"),style=MaterialTheme.typography.labelSmall,color=CoaxiTheme.colors.textSecondary);CoaxiMessageBody(model,threadId,message)}}
            if(!busy&&rows.isEmpty())item{Text(stringResource(R.string.coaxi_fusion_35e9e493e85d6aca),color=CoaxiTheme.colors.textSecondary)}
        }
    }}}
}

@Composable fun CoaxiPublicMessageOrigin(model:CoaxiWorkspaceViewModel,threadId:String,messageId:String,label:String){
    var open by remember(threadId,messageId){mutableStateOf(false)}
    TextButton(onClick={open=true}){Text(label)}
    if(open)CoaxiThreadHistory(model,threadId,messageId){open=false}
}
