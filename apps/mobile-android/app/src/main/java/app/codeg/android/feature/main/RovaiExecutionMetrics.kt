package app.codeg.android.feature.main

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.json.*
import java.text.NumberFormat

/** Source execution metrics: one visible Thread reader, exact Run epochs, finite terminal tails. */
@Composable fun RovaiExecutionMetrics(model:RovaiWorkspaceViewModel,threadId:String,projection:JsonObject){
    val scope=rememberCoroutineScope();val lifecycle=LocalLifecycleOwner.current;val generation=model.connectionGeneration;val gate=remember(threadId,generation){Mutex()}
    var expanded by remember(threadId,generation){mutableStateOf(false)};var count by remember(threadId,generation){mutableStateOf(50)}
    var active by remember{mutableStateOf(lifecycle.lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED))};var metrics by remember(threadId,generation){mutableStateOf<JsonObject?>(null)};var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)}
    val all=projection["agentRuns"].coreArray();val rows=all.takeLast(count)
    val selected=(rows+all.filter{it.coreText("status")in listOf("running","waiting")}).distinctBy{it.coreText("id")}.takeLast(500)
    val identities=selected.map{Triple(it.coreText("id"),it.coreLong("executionEpoch"),it.coreText("status"))};val latestIds by rememberUpdatedState(identities)
    val live=selected.any{it.coreText("status")in listOf("running","waiting")}
    suspend fun load()=gate.withLock{
        if(!expanded||!active)return@withLock;val captured=latestIds;busy=true
        try{val result=model.query(RovaiOperation.MONITORING_EXECUTION,buildJsonObject{put("threadId",threadId);put("agentRunIds",JsonArray(captured.map{JsonPrimitive(it.first)}))}).coreObject();require(result.coreLong("schemaVersion")==1L){"执行用量格式不受支持"};if(captured==latestIds&&generation==model.connectionGeneration){metrics=result;issue=null}}
        catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}
    }
    DisposableEffect(lifecycle){val listener=LifecycleEventObserver{_,event->if(event==Lifecycle.Event.ON_STOP)active=false else if(event==Lifecycle.Event.ON_START)active=true};lifecycle.lifecycle.addObserver(listener);onDispose{lifecycle.lifecycle.removeObserver(listener)}}
    LaunchedEffect(expanded,active,identities,generation){if(expanded&&active){load();if(live)while(isActive){delay(4000);load()}else{var prior=0L;for(at in listOf(250L,1000L,4000L)){delay(at-prior);prior=at;load()}}}}
    val number=remember{NumberFormat.getNumberInstance().apply{maximumFractionDigits=0}}
    fun display(value:JsonElement?)=(value as?JsonPrimitive)?.longOrNull?.let{number.format(it)}?:"—"
    Column(verticalArrangement=Arrangement.spacedBy(8.dp)){
        Row{TextButton(onClick={expanded=!expanded}){Text(if(expanded)"收起执行用量与上下文" else "执行用量与上下文")};if(expanded)TextButton(enabled=!busy&&active,onClick={scope.launch{load()}}){Text("刷新")}}
        if(expanded){
            issue?.let{Text(it+if(metrics!=null)"；保留上次读取结果" else "",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.danger)}
            metrics?.get("sessions").coreArray().forEach{session->key(session.coreText("conversationId"),session.coreLong("sessionGeneration")){GlassRow(Modifier.fillMaxWidth()){
                val member=projection["members"].coreArray().firstOrNull{it.coreText("agentId")==session.coreText("agentId")};Text((member?.coreText("displayName")?:"队员")+" · 当前原生会话上下文",style=MaterialTheme.typography.titleSmall)
                val used=(session["usedTokens"]as?JsonPrimitive)?.longOrNull;val window=(session["windowTokens"]as?JsonPrimitive)?.longOrNull;val native=(session["nativeRatio"]as?JsonPrimitive)?.doubleOrNull
                val fraction=if(used!=null&&used>=0&&window!=null&&window>0&&used<=window)used.toDouble()/window else native?.takeIf{it.isFinite()&&it in 0.0..1.0}
                Text(display(session["usedTokens"])+" / "+display(session["windowTokens"])+" · "+(fraction?.let{NumberFormat.getPercentInstance().apply{maximumFractionDigits=1}.format(it)}?:"比例未知"))
                if(fraction!=null)LinearProgressIndicator(progress={fraction.toFloat()},modifier=Modifier.fillMaxWidth())
                Text(listOf(session.coreText("runtimeKind"),session.coreText("modelKey"),session.coreText("source"),session.coreText("observedAt")).filter{it.isNotEmpty()}.joinToString(" · "),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)
            }}}
            if(metrics!=null&&metrics?.get("sessions").coreArray().isEmpty())Text("当前原生会话上下文尚无有效观测",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)
            rows.forEach{run->key(run.coreText("id"),run.coreLong("executionEpoch")){GlassRow(Modifier.fillMaxWidth()){
                val usage=metrics?.get("runs").coreArray().firstOrNull{it.coreText("agentRunId")==run.coreText("id")&&it.coreLong("executionEpoch")==run.coreLong("executionEpoch")}
                val member=projection["members"].coreArray().firstOrNull{it.coreText("agentId")==run.coreText("agentId")}
                Text((member?.coreText("displayName")?:"队员")+" · "+run.coreText("status"),style=MaterialTheme.typography.titleSmall)
                listOf("promptInputTotalTokens" to "输入Token","outputTokens" to "输出Token","cacheReadTokens" to "缓存读取","cacheWriteTokens" to "缓存写入").forEach{(key,label)->Row{Text(label,Modifier.weight(1f));Text(display(usage?.get(key)))}}
                if(usage?.get("inputOutputComplete")!=JsonPrimitive(true))Text("输入/输出为部分统计或未知",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)
                Text("最近观测 · "+usage?.coreText("lastObservedAt").orEmpty().ifEmpty{"—"},style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)
            }}}
            if(rows.size<all.size&&count<500)TextButton(onClick={count=(count+50).coerceAtMost(500)}){Text("查看更多执行用量")}
            Text("不从比例推算Token数量；上下文与单次执行用量分别读取。历史稳定后停止轮询，关闭面板或退后台后停止读取。",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)
        }
    }
}
