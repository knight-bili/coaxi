package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
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

private val monitoringMetrics=listOf("promptInputTotalTokens" to "输入Token","outputTokens" to "输出Token","uncachedInputTokens" to "未缓存输入","cacheReadTokens" to "缓存读取","cacheWriteTokens" to "缓存写入","reasoningOutputTokens" to "推理输出","cacheReadShare" to "缓存读取占比","requestCacheHitRate" to "请求缓存命中率")
private fun monitoringNumber(value:JsonElement?,percent:Boolean=false):String{val number=(value as?JsonPrimitive)?.doubleOrNull?:return "—";return if(percent)NumberFormat.getPercentInstance().apply{maximumFractionDigits=1}.format(number)else NumberFormat.getNumberInstance().apply{maximumFractionDigits=0}.format(number)}
private fun monitoringMoney(value:JsonElement?):String=value.coreArray().joinToString(" · "){it.coreText("currency")+" "+it.coreText("amount")+listOf(it.coreText("kind"),it.coreText("source")).filter{it.isNotEmpty()}.joinToString(" / ",prefix="（",postfix="）")}.ifEmpty{"—"}

/** Ports RuntimeMonitoring's sparse metrics and filters; the page never collects transcripts or bills. */
@Composable fun RovaiMonitoring(model:RovaiWorkspaceViewModel){
    var selected by remember{mutableIntStateOf(0)}
    Column(Modifier.fillMaxSize()){
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),horizontalArrangement=Arrangement.spacedBy(6.dp)){listOf("运行监控","用量看板","原生历史用量").forEachIndexed{index,title->FilterChip(selected=selected==index,onClick={selected=index},label={Text(title)})}}
        Box(Modifier.weight(1f)){when(selected){0->RovaiRuntimeMonitoring(model);1->RovaiUsageDashboard(model,false);else->RovaiUsageDashboard(model,true)}}
    }
}

@Composable private fun RovaiRuntimeMonitoring(model:RovaiWorkspaceViewModel){
    val scope=rememberCoroutineScope();val context=LocalContext.current;val lifecycle=LocalLifecycleOwner.current
    val generation=model.connectionGeneration;val gate=remember{Mutex()}
    var range by remember{mutableStateOf("24h")};var runtime by remember{mutableStateOf("")};var provider by remember{mutableStateOf("")};var selectedModel by remember{mutableStateOf("")};var costKind by remember{mutableStateOf("")}
    var snapshot by remember{mutableStateOf<JsonObject?>(null)};var loadedFilter by remember{mutableStateOf<JsonObject?>(null)};var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)};var exportBytes by remember{mutableStateOf<ByteArray?>(null)}
    var active by remember{mutableStateOf(lifecycle.lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED))}
    val filter=buildJsonObject{put("range",range);if(runtime.isNotEmpty())put("runtimeKind",runtime);if(provider.isNotEmpty())put("providerKey",provider);if(selectedModel.isNotEmpty())put("modelKey",selectedModel);if(costKind.isNotEmpty())put("costKind",costKind)}
    val currentFilter by rememberUpdatedState(filter)
    suspend fun load()=gate.withLock{
        if(!active)return@withLock
        val captured=currentFilter;busy=true
        try{val value=model.query(RovaiOperation.MONITORING_SNAPSHOT,captured).coreObject();if(captured!=currentFilter||generation!=model.connectionGeneration)return@withLock;require(value.coreLong("schemaVersion")==2L){"运行监控格式不受支持"};snapshot=value;loadedFilter=captured;issue=null}
        catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}
    }
    DisposableEffect(lifecycle){val observer=LifecycleEventObserver{_,event->if(event==Lifecycle.Event.ON_STOP)active=false else if(event==Lifecycle.Event.ON_START)active=true};lifecycle.lifecycle.addObserver(observer);onDispose{lifecycle.lifecycle.removeObserver(observer)}}
    LaunchedEffect(filter,active,generation){if(active){while(isActive){load();delay(12000)}}}
    val exporter=rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/json")){uri->val bytes=exportBytes;exportBytes=null;if(uri!=null&&bytes!=null)scope.launch{try{withContext(Dispatchers.IO){context.contentResolver.openOutputStream(uri,"wt")?.use{it.write(bytes)}?:error("无法写入导出位置")}}catch(error:Exception){issue=error.message}}}
    val visible=snapshot.takeIf{loadedFilter==filter}
    LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
        item{Row{Text(stringResource(R.string.rovai_fusion_65e09e4dfa428480),Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(enabled=!busy&&active,onClick={scope.launch{load()}}){Text(if(busy)"正在读取…" else "刷新")};TextButton(enabled=visible!=null&&exportBytes==null,onClick={val captured=visible?:return@TextButton;exportBytes=buildJsonObject{put("filter",filter);put("snapshot",captured)}.toString().toByteArray(Charsets.UTF_8);exporter.launch("Codeg-Rovai-usage.json")}){Text(stringResource(R.string.rovai_fusion_76420433f22d1c80))}}}
        item{Row(Modifier.horizontalScroll(rememberScrollState()),horizontalArrangement=Arrangement.spacedBy(8.dp)){listOf("24h" to "24小时","7d" to "7天","30d" to "30天").forEach{(id,label)->FilterChip(selected=range==id,onClick={range=id},label={Text(label)})}}}
        item{Row(Modifier.horizontalScroll(rememberScrollState()),horizontalArrangement=Arrangement.spacedBy(8.dp)){
            MonitoringFilter("智能体",runtime,(snapshot?.get("byRuntime").coreArray().map{it.coreText("runtimeKind")}+runtime).filter{it.isNotEmpty()}.distinct()){runtime=it}
            MonitoringFilter("Provider",provider,(snapshot?.get("byModel").coreArray().map{it.coreText("providerKey")}+provider).filter{it.isNotEmpty()}.distinct()){provider=it}
            MonitoringFilter("模型",selectedModel,(snapshot?.get("byModel").coreArray().map{it.coreText("modelKey")}+selectedModel).filter{it.isNotEmpty()}.distinct()){selectedModel=it}
            MonitoringFilter("成本",costKind,listOf("model_call","turn","run","session")){costKind=it}
        }}
        issue?.let{message->item{Text(message+(visible?.let{"；显示截至${it["range"].coreObject().coreText("to")}的数据"}?:""),color=CodegTheme.colors.danger)}}
        if(visible==null)item{Text(if(busy)"正在汇总已保存的用量…" else "暂无可显示的数据",color=CodegTheme.colors.textSecondary)}
        visible?.let{value->
            item{Text("${value["range"].coreObject().coreText("from")} — ${value["range"].coreObject().coreText("to")}",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}
            item{GlassRow(Modifier.fillMaxWidth()){
                val summary=value["summary"].coreObject();monitoringMetrics.forEach{(key,label)->Row(Modifier.fillMaxWidth()){Text(label,Modifier.weight(1f));Text(monitoringNumber(summary[key],key in listOf("cacheReadShare","requestCacheHitRate")))}}
                Text(stringResource(R.string.rovai_fusion_cbc87f47c67378d0)+monitoringMoney(summary["cost"].coreObject()["run"]),style=MaterialTheme.typography.bodySmall)
            }}
            item{RovaiUsageTrend(value["trend"].coreArray())}
            item{Text(stringResource(R.string.rovai_fusion_ebefc591a101b140),style=MaterialTheme.typography.titleSmall);Text(stringResource(R.string.rovai_fusion_01bbdc2f71904ea6),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary);GlassRow{(monitoringMetrics.filter{it.first!="cacheReadShare"}+listOf("cost" to "成本")).forEach{(key,label)->val coverage=value["coverage"].coreObject()[key].coreObject();Row{Text(label,Modifier.weight(1f));Text("${coverage.coreLong("observedRuns")}/${coverage.coreLong("eligibleRuns")}")}}}}
            listOf("byRuntime" to "按智能体","byModel" to "按模型").forEach{(key,label)->
                item{Text(label,style=MaterialTheme.typography.titleSmall)}
                itemsIndexed(value[key].coreArray()){_,row->GlassRow(Modifier.fillMaxWidth()){
                    Text(listOf(row.coreText("runtimeKind"),row.coreText("providerKey"),row.coreText("modelKey")).filter{it.isNotEmpty()}.joinToString(" · "),style=MaterialTheme.typography.titleSmall)
                    monitoringMetrics.forEach{(metric,title)->Row{Text(title,Modifier.weight(1f));Text(monitoringNumber(row[metric],metric in listOf("cacheReadShare","requestCacheHitRate")))}}
                    Text(monitoringMoney(row["cost"]));val coverage=row["coverage"].coreObject();Text("覆盖率 ${coverage.coreLong("observedRuns")}/${coverage.coreLong("eligibleRuns")}",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)
                }}
            }
            val cost=value["summary"].coreObject()["cost"]as?JsonObject
            if(cost!=null)item{GlassRow(Modifier.fillMaxWidth()){Text(stringResource(R.string.rovai_fusion_50b0f11ab55c7e34),style=MaterialTheme.typography.titleSmall);Text("Run · "+monitoringMoney(cost["run"]));Text("Provider · "+monitoringMoney(cost["reconciliation"]));Text(stringResource(R.string.rovai_fusion_246c88ad5c14dda3)+monitoringMoney(cost["difference"]));Text(stringResource(R.string.rovai_fusion_cf2c4c620589f4a7)+cost.coreText("latestReconciledAt").ifEmpty{"—"});Text(stringResource(R.string.rovai_fusion_9f6806f424c4f915),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}}
            item{Text(stringResource(R.string.rovai_fusion_8a151f46d42d67cf)+value["collection"].coreObject().coreText("startedAt"),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}
        }
    }
}

@Composable private fun MonitoringFilter(label:String,value:String,options:List<String>,onChange:(String)->Unit){var open by remember{mutableStateOf(false)};Box{OutlinedButton(onClick={open=true}){Text(label+" · "+value.ifEmpty{"全部"})};DropdownMenu(expanded=open,onDismissRequest={open=false}){DropdownMenuItem(text={Text(stringResource(R.string.rovai_fusion_5c55a67935af8f45))},onClick={onChange("");open=false});options.sorted().forEach{option->DropdownMenuItem(text={Text(option)},onClick={onChange(option);open=false})}}}}

@Composable private fun RovaiUsageTrend(points:List<JsonObject>){
    var metric by remember{mutableStateOf("promptInputTotalTokens")};var index by remember(points){mutableStateOf((points.size-1).coerceAtLeast(0))}
    val choices=monitoringMetrics.filter{it.first in listOf("promptInputTotalTokens","outputTokens","cacheReadTokens","cacheWriteTokens")}
    val accent=CodegTheme.colors.accent;val grid=CodegTheme.colors.hairline
    Column(verticalArrangement=Arrangement.spacedBy(8.dp)){
        Text(stringResource(R.string.rovai_fusion_6a81178c663f5b3c),style=MaterialTheme.typography.titleSmall)
        Row(Modifier.horizontalScroll(rememberScrollState()),horizontalArrangement=Arrangement.spacedBy(6.dp)){choices.forEach{(key,label)->FilterChip(selected=metric==key,onClick={metric=key},label={Text(label)})}}
        val selected=points.getOrNull(index)
        Text((selected?.coreText("bucketStartAt")?:"暂无时间点")+" · "+monitoringNumber(selected?.get(metric)),style=MaterialTheme.typography.bodySmall)
        Canvas(Modifier.fillMaxWidth().height(180.dp).semantics{contentDescription="用量趋势；下方滑块可选择时间点，缺失观测不连线。"}){
            val max=(points.mapNotNull{(it[metric]as?JsonPrimitive)?.doubleOrNull}.maxOrNull()?:0.0).coerceAtLeast(1.0)
            repeat(5){tick->val y=size.height*tick/4;drawLine(grid,androidx.compose.ui.geometry.Offset(0f,y),androidx.compose.ui.geometry.Offset(size.width,y))}
            val path=Path();var started=false
            points.forEachIndexed{i,point->val number=(point[metric]as?JsonPrimitive)?.doubleOrNull;if(number==null)started=false else{val x=if(points.size>1)size.width*i/(points.size-1)else size.width/2;val y=size.height*(1-number/max).toFloat();if(started)path.lineTo(x,y)else path.moveTo(x,y);started=true;drawCircle(accent,2.dp.toPx(),androidx.compose.ui.geometry.Offset(x,y))}}
            drawPath(path,accent,style=Stroke(2.dp.toPx()))
        }
        if(points.size>1)Slider(value=index.toFloat(),onValueChange={index=it.toInt().coerceIn(points.indices)},valueRange=0f..(points.size-1).toFloat(),modifier=Modifier.semantics{contentDescription="趋势时间点"})
        Text(stringResource(R.string.rovai_fusion_107af4daf34aa1da),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)
    }
}
