package app.codeg.android.feature.main

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
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*
import java.text.NumberFormat
import java.time.*

private val usageRanges=listOf("7d" to "近7天","30d" to "近30天","90d" to "近90天","thisMonth" to "本月","thisYear" to "今年","all" to "全部","custom" to "自选日期")
private fun usageNumber(value:JsonElement?):String=(value as?JsonPrimitive)?.doubleOrNull?.takeIf{it.isFinite()}?.let{NumberFormat.getNumberInstance().apply{maximumFractionDigits=0}.format(it)}?:"—"

/** Core 分时事实和冻结原生历史使用独立口径；沿用原生监控的 GlassRow、趋势与导出方式。 */
@Composable fun RovaiUsageDashboard(model:RovaiWorkspaceViewModel,history:Boolean){
    val coroutine=rememberCoroutineScope();val context=androidx.compose.ui.platform.LocalContext.current
    val generation=model.connectionGeneration
    var range by remember(history){mutableStateOf("30d")};var bucket by remember{mutableStateOf("day")};var project by remember{mutableStateOf("")};var runtime by remember{mutableStateOf("")};var selectedModel by remember{mutableStateOf("")}
    var from by remember{mutableStateOf("")};var to by remember{mutableStateOf("")};var issue by remember{mutableStateOf<String?>(null)}
    var snapshot by remember(history,generation){mutableStateOf<JsonObject?>(null)};var loadedQuery by remember(history,generation){mutableStateOf<JsonObject?>(null)};var busy by remember{mutableStateOf(false)};var refresh by remember{mutableIntStateOf(0)};var exportBytes by remember{mutableStateOf<ByteArray?>(null)}
    val now=remember(history,refresh){ZonedDateTime.now()};val offset=now.offset.totalSeconds/60
    val query=remember(history,range,bucket,project,runtime,selectedModel,from,to,now){runCatching{
        if(history){
            val tomorrow=now.toLocalDate().plusDays(1).atStartOfDay(now.zone)
            val end=if(range=="custom")LocalDate.parse(to).plusDays(1).atStartOfDay(now.zone)else tomorrow
            val start=when(range){"7d"->tomorrow.minusDays(7);"30d"->tomorrow.minusDays(30);"90d"->tomorrow.minusDays(90);"thisMonth"->now.withDayOfMonth(1).toLocalDate().atStartOfDay(now.zone);"thisYear"->now.withDayOfYear(1).toLocalDate().atStartOfDay(now.zone);"custom"->LocalDate.parse(from).atStartOfDay(now.zone);else->null}
            require(start==null||start<end){"开始日期必须早于结束日期"}
            buildJsonObject{put("operation","report");putJsonObject("filter"){if(start!=null)put("start",start.toInstant().toString());if(range!="all")put("end",end.toInstant().toString());put("bucket",bucket);put("tzOffsetMinutes",offset);put("comparePrevious",true);if(runtime.isNotBlank())putJsonArray("agentTypes"){add(runtime)};if(selectedModel.isNotBlank())putJsonArray("models"){add(selectedModel)}};if(project.isNotBlank())putJsonArray("projectRoots"){add(project)}}
        }else buildJsonObject{put("range",range);put("bucket",bucket);put("tzOffsetMinutes",offset);if(range=="custom"){val a=LocalDate.parse(from);val b=LocalDate.parse(to);require(a<=b){"开始日期不能晚于结束日期"};put("start",a.toString());put("end",b.toString())};if(project.isNotBlank())put("projectRoot",project);if(runtime.isNotBlank())put("runtimeKind",runtime);if(selectedModel.isNotBlank())put("modelKey",selectedModel)}
    }}
    val capturedQuery=query.getOrNull()
    LaunchedEffect(capturedQuery,generation,refresh){
        snapshot=null;loadedQuery=null;issue=query.exceptionOrNull()?.message
        val captured=capturedQuery?:return@LaunchedEffect;busy=true
        try{delay(250);val value=model.query(if(history)RovaiOperation.HISTORY_USAGE else RovaiOperation.MONITORING_REPORT,captured).coreObject();ensureActive();if(generation==model.connectionGeneration){require(value.coreLong("schemaVersion")==1L){"用量格式不受支持"};snapshot=value;loadedQuery=captured;issue=null}}
        catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}
    }
    val exporter=rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/json")){uri->val bytes=exportBytes;exportBytes=null;if(uri!=null&&bytes!=null)coroutine.launch{try{withContext(Dispatchers.IO){context.contentResolver.openOutputStream(uri,"wt")?.use{it.write(bytes)}?:error("无法写入导出位置")}}catch(error:Exception){issue=error.message}}}
    val visible=snapshot.takeIf{loadedQuery==capturedQuery};val report=if(history)visible?.get("report").coreObject()else visible
    val totals=report?.get("totals").coreObject();val totalKey=if(history)"total_tokens"else"knownTotalTokens"
    LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
        item{Row{Text(if(history)"原生历史用量"else"用量看板",Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(enabled=!busy,onClick={refresh++}){Text("刷新")};TextButton(enabled=visible!=null&&exportBytes==null,onClick={val value=visible?:return@TextButton;exportBytes=buildJsonObject{put("query",capturedQuery!!);put("snapshot",value)}.toString().toByteArray(Charsets.UTF_8);exporter.launch(if(history)"Codeg-Rovai-native-usage.json"else"Codeg-Rovai-activity.json")}){Text("导出")}}}
        item{Row(Modifier.horizontalScroll(rememberScrollState()),horizontalArrangement=Arrangement.spacedBy(6.dp)){usageRanges.forEach{(key,label)->FilterChip(selected=range==key,onClick={range=key;bucket=if(key in listOf("all","thisYear"))"month"else if(key=="90d")"week"else"day"},label={Text(label)})}}}
        if(range=="custom")item{Row(horizontalArrangement=Arrangement.spacedBy(8.dp)){OutlinedTextField(from,{from=it},Modifier.weight(1f),label={Text("开始 YYYY-MM-DD")},singleLine=true);OutlinedTextField(to,{to=it},Modifier.weight(1f),label={Text("结束 YYYY-MM-DD")},singleLine=true)}}
        item{Column(verticalArrangement=Arrangement.spacedBy(6.dp)){OutlinedTextField(project,{project=it},Modifier.fillMaxWidth(),label={Text("工程路径 留空全部")},singleLine=true);Row(horizontalArrangement=Arrangement.spacedBy(8.dp)){OutlinedTextField(runtime,{runtime=it},Modifier.weight(1f),label={Text("智能体")},singleLine=true);OutlinedTextField(selectedModel,{selectedModel=it},Modifier.weight(1f),label={Text("模型")},singleLine=true)};Row(horizontalArrangement=Arrangement.spacedBy(6.dp)){listOf("day" to "按天","week" to "按周","month" to "按月").forEach{(key,label)->FilterChip(selected=bucket==key,onClick={bucket=key},label={Text(label)})}}}}
        issue?.let{item{Text(it,color=CodegTheme.colors.danger)}}
        if(busy)item{LinearProgressIndicator(Modifier.fillMaxWidth())}
        if(report!=null&&visible!=null){
            item{GlassRow(Modifier.fillMaxWidth()){Text("已报告 Token",style=MaterialTheme.typography.labelMedium);Text(usageNumber(totals[totalKey]),style=MaterialTheme.typography.headlineLarge);val metrics=if(history)listOf("input_tokens" to "输入","output_tokens" to "输出","cache_read_tokens" to "缓存读取","cache_creation_tokens" to "缓存写入","conversation_count" to "会话","turn_count" to "用量记录")else listOf("inputTokens" to "输入","outputTokens" to "输出","cacheReadTokens" to "缓存读取","cacheWriteTokens" to "缓存写入","runs" to "运行","partialObservations" to "未完整报告次数");metrics.forEach{(key,label)->Row{Text(label,Modifier.weight(1f));Text(usageNumber(totals[key]))}}}}
            item{RovaiActivityTrend(report["series"].coreArray(),history)}
            val rows=if(history)listOf("by_folder" to "工程排行","by_agent" to "智能体排行","by_model" to "模型排行")else listOf("byProject" to "工程排行","byRuntime" to "智能体排行","byModel" to "模型排行")
            rows.forEach{(key,title)->item{Text(title,style=MaterialTheme.typography.titleSmall)};itemsIndexed(report[key].coreArray()){index,row->GlassRow(Modifier.fillMaxWidth()){Row{Text("${index+1}  "+row.coreText(if(history)"label"else"key").let{if(it=="__other__")"其他"else it.ifEmpty{"未记录"}},Modifier.weight(1f),maxLines=2);Text(usageNumber(row[totalKey]))}}}}
            if(history){item{Text("会话榜",style=MaterialTheme.typography.titleSmall)};itemsIndexed(report["top_conversations"].coreArray()){_,row->GlassRow(Modifier.fillMaxWidth()){Text(row.coreText("title").ifEmpty{"未命名会话"});Text(row.coreText("agent_type")+" · "+usageNumber(row["total_tokens"]),color=CodegTheme.colors.textSecondary)}};item{Text("只统计已导入的冻结快照；不与融合运行数据相加。还有 ${visible["missingIndex"].coreArray().size} 份未索引，需在桌面同步。整会话回退 ${visible.coreLong("sessionTotalFallbacks")} 份，已排除同源旧快照 ${visible.coreLong("supersededSnapshots")} 份。",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}}
            else item{Text("来源：融合 Core；采集开始 ${visible.coreText("recordingStartedAt")}；分时精度 ${visible.coreLong("resolutionSeconds")} 秒，保留 ${visible.coreLong("retentionDays")} 天。未知不等于零，不代表账单。",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}
        }
    }
}

@Composable private fun RovaiActivityTrend(points:List<JsonObject>,history:Boolean){
    var index by remember(points){mutableIntStateOf((points.size-1).coerceAtLeast(0))};val accent=CodegTheme.colors.accent;val ink=CodegTheme.colors.textPrimary
    val total=if(history)"total_tokens"else"knownTotalTokens";val cache=if(history)"cache_read_tokens"else"cacheReadTokens"
    val selected=points.getOrNull(index)
    Column(verticalArrangement=Arrangement.spacedBy(8.dp)){Text("用量趋势",style=MaterialTheme.typography.titleSmall);Text((selected?.coreText(if(history)"bucket_key"else"key")?:"暂无记录")+" · "+usageNumber(selected?.get(total)),style=MaterialTheme.typography.bodySmall)
        Canvas(Modifier.fillMaxWidth().height(120.dp).semantics{contentDescription="缓存和其他已报告Token趋势，下方滑块选择时间点"}){
            val chunks=if(points.size<=48)points.map{listOf(it)}else points.chunked((points.size+47)/48)
            val values=chunks.map{chunk->val sum=chunk.sumOf{(it[total]as?JsonPrimitive)?.doubleOrNull?:0.0};val read=chunk.sumOf{row->if(!history&&row["inputTokens"]==JsonNull)0.0 else (row[cache]as?JsonPrimitive)?.doubleOrNull?:0.0}.coerceIn(0.0,sum);read to (sum-read)}
            val maximum=values.maxOfOrNull{it.first+it.second}?.coerceAtLeast(1.0)?:1.0;val width=if(values.isEmpty())0f else size.width/values.size
            values.forEachIndexed{i,(read,fresh)->val cached=(read/maximum*size.height).toFloat();val other=(fresh/maximum*size.height).toFloat();drawRect(accent,Offset(i*width,size.height-cached),Size((width-2).coerceAtLeast(1f),cached));drawRect(ink,Offset(i*width,size.height-cached-other),Size((width-2).coerceAtLeast(1f),other))}
        }
        if(points.size>1)Slider(index.toFloat(),{index=it.toInt().coerceIn(points.indices)},valueRange=0f..(points.size-1).toFloat())
        Text("缓存读取 · 其他已报告 Token",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)
    }
}
