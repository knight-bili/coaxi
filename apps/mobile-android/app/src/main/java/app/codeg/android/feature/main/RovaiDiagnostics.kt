package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*

private fun diagnosticStatus(value:String)=when(value){"ok"->"正常";"attention"->"需要处理";else->"暂时无法确认"}

/** DiagnosticsCenter's source checks, grouped status and centrally redacted v5 export. */
@Composable fun RovaiDiagnostics(model:RovaiWorkspaceViewModel){
    val state=model.state.collectAsState().value;val scope=rememberCoroutineScope();val context=LocalContext.current;val generation=model.connectionGeneration
    var report by remember{mutableStateOf<JsonObject?>(null)};var filter by remember{mutableStateOf("")};var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)}
    var exportBytes by remember{mutableStateOf<ByteArray?>(null)};var repair by remember{mutableStateOf(false)};var settings by remember{mutableStateOf<String?>(null)}
    val blocked=busy||state.busy||state.pending!=null
    suspend fun load(){busy=true;try{val value=model.query(RovaiOperation.DIAGNOSTICS_CHECK).coreObject();require(generation==model.connectionGeneration){"工作区已切换"};require(value.coreLong("schemaVersion")==1L){"诊断格式不受支持"};report=value;issue=null}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    val export=rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/json")){uri->val bytes=exportBytes;exportBytes=null;if(uri!=null&&bytes!=null)scope.launch{try{withContext(Dispatchers.IO){context.contentResolver.openOutputStream(uri,"wt")?.use{it.write(bytes)}?:error("无法写入导出位置")}}catch(error:Exception){issue=error.message}}}
    fun exportReport(){if(blocked)return;busy=true;scope.launch{try{val value=model.query(RovaiOperation.DIAGNOSTICS_EXPORT).coreObject();require(generation==model.connectionGeneration);require(value.coreText("format")=="rovai-diagnostics-v5"){"诊断导出格式不受支持"};exportBytes=value.toString().toByteArray(Charsets.UTF_8);export.launch("Codeg-Rovai-diagnostics.json")}catch(error:Exception){issue=error.message}finally{busy=false}}}
    LaunchedEffect(generation){load()}
    if(settings!=null){Column(Modifier.fillMaxSize()){TextButton(onClick={settings=null;scope.launch{load()}}){Text(stringResource(R.string.rovai_fusion_f28eb7ebae884586))};Box(Modifier.weight(1f)){if(settings=="mcp")RovaiMcpSettings(model)else RovaiRuntimeSettings(model)}};return}
    LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
        item{Row{Text(stringResource(R.string.rovai_fusion_00e884d505858e4c),Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(enabled=!blocked,onClick={scope.launch{load()}}){Text(if(busy)"正在读取…" else "重新自检")};TextButton(enabled=!blocked&&exportBytes==null,onClick={exportReport()}){Text(stringResource(R.string.rovai_fusion_2155ff58d68abafc))}}}
        item{Text(stringResource(R.string.rovai_fusion_4cc16b1ca583c82e),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}
        issue?.let{item{Text(it+(if(report!=null)"；保留上次读取结果" else ""),color=CodegTheme.colors.danger)}}
        report?.let{value->
            item{val summary=value["summary"].coreObject();Text("正常 ${summary.coreLong("ok")} · 需要处理 ${summary.coreLong("attention")} · 无法确认 ${summary.coreLong("unknown")}");Text(stringResource(R.string.rovai_fusion_6a17a7589cbf4a37)+value.coreText("checkedAt"),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}
            item{Row(Modifier.horizontalScroll(rememberScrollState()),horizontalArrangement=Arrangement.spacedBy(8.dp)){listOf("" to "全部","attention" to "需要处理","unknown" to "无法确认","ok" to "正常").forEach{(key,label)->FilterChip(selected=filter==key,onClick={filter=key},label={Text(label)})}}}
            listOf("local_dependencies" to "本地依赖","managed_content" to "受管内容","agent_runtimes" to "智能体").forEach{(group,title)->
                val checks=value["checks"].coreArray().filter{it.coreText("group")==group&&(filter.isEmpty()||it.coreText("status")==filter)}
                if(checks.isNotEmpty())item{Text(title,style=MaterialTheme.typography.titleSmall)}
                items(checks,key={it.coreText("id")}){check->GlassRow(Modifier.fillMaxWidth()){
                    var expanded by remember(check.coreText("id")){mutableStateOf(check.coreText("status")!="ok")}
                    Row{Text(check.coreText("label"),Modifier.weight(1f),style=MaterialTheme.typography.titleSmall);Text(diagnosticStatus(check.coreText("status")),style=MaterialTheme.typography.labelSmall,color=if(check.coreText("status")=="attention")CodegTheme.colors.danger else CodegTheme.colors.textSecondary)}
                    Text(check.coreText("detail"));if(check["stale"]==JsonPrimitive(true))Text(stringResource(R.string.rovai_fusion_4a6765caf9f1dc45),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)
                    TextButton(onClick={expanded=!expanded}){Text(if(expanded)"收起详情" else "查看详情")}
                    if(expanded){Text(check.coreText("code"),style=MaterialTheme.typography.labelSmall);Text(check.coreText("observedAt"),style=MaterialTheme.typography.labelSmall,color=CodegTheme.colors.textSecondary);check["facts"].coreArray().forEach{fact->Text(fact.coreText("key")+" · "+fact.coreText("value"),style=MaterialTheme.typography.bodySmall)}}
                    if(check.coreText("status")!="ok")when{
                        check.coreText("id")=="mcp-config"&&check.coreText("code")=="mcp_config_permissions_too_broad"->TextButton(enabled=!blocked,onClick={repair=true}){Text(stringResource(R.string.rovai_fusion_8c55ac3cecf70b45))}
                        check.coreText("id")=="mcp-config"->TextButton(onClick={settings="mcp"}){Text(stringResource(R.string.rovai_fusion_c6a75925a46c5a78))}
                        check.coreText("subjectKind")=="runtime"&&check.coreText("status")=="unknown"&&check.coreText("subjectId").isNotEmpty()->TextButton(enabled=!blocked,onClick={busy=true;scope.launch{try{model.query(RovaiOperation.RUNTIME_PRODUCT_CHECK,buildJsonObject{put("runtimeKind",check.coreText("subjectId"))});require(generation==model.connectionGeneration);load()}catch(error:Exception){issue=error.message}finally{busy=false}}}){Text(stringResource(R.string.rovai_fusion_6dd2a066dcd28b7d))}
                        check.coreText("subjectKind")=="runtime"->TextButton(onClick={settings="runtime"}){Text(stringResource(R.string.rovai_fusion_32cb9584ed8b797e))}
                        check.coreText("id")=="legacy-skill-entries"->Text(stringResource(R.string.rovai_fusion_95a00f274994a29a),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)
                        else->TextButton(enabled=!blocked,onClick={exportReport()}){Text(stringResource(R.string.rovai_fusion_628fddd936637f02))}
                    }
                }}
            }
        }
    }
    if(repair)AlertDialog(onDismissRequest={if(!busy)repair=false},title={Text(stringResource(R.string.rovai_fusion_8b999d49460b1353))},text={Text(stringResource(R.string.rovai_fusion_926c3165fc67d157))},confirmButton={TextButton(enabled=!blocked,onClick={busy=true;scope.launch{try{model.query(RovaiOperation.MCP_CONFIG_REPAIRPERMISSIONS);require(generation==model.connectionGeneration);repair=false;load()}catch(error:Exception){issue=error.message}finally{busy=false}}}){Text(stringResource(R.string.rovai_fusion_efd747465b82cd8c))}},dismissButton={TextButton(enabled=!busy,onClick={repair=false}){Text(stringResource(R.string.rovai_fusion_2cd0f3be8738a86c))}})
}
