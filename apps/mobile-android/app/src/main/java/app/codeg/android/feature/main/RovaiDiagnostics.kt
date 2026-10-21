package app.codeg.android.feature.main

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
    if(settings!=null){Column(Modifier.fillMaxSize()){TextButton(onClick={settings=null;scope.launch{load()}}){Text("返回诊断")};Box(Modifier.weight(1f)){if(settings=="mcp")RovaiMcpSettings(model)else RovaiRuntimeSettings(model)}};return}
    LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
        item{Row{Text("诊断中心",Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(enabled=!blocked,onClick={scope.launch{load()}}){Text(if(busy)"正在读取…" else "重新自检")};TextButton(enabled=!blocked&&exportBytes==null,onClick={exportReport()}){Text("导出诊断")}}}
        item{Text("读取主机的依赖、受管内容和智能体状态。导出由Core统一脱敏，不包含消息、记忆正文、附件正文或工具输出。",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}
        issue?.let{item{Text(it+(if(report!=null)"；保留上次读取结果" else ""),color=CodegTheme.colors.danger)}}
        report?.let{value->
            item{val summary=value["summary"].coreObject();Text("正常 ${summary.coreLong("ok")} · 需要处理 ${summary.coreLong("attention")} · 无法确认 ${summary.coreLong("unknown")}");Text("读取时间 · "+value.coreText("checkedAt"),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}
            item{Row(Modifier.horizontalScroll(rememberScrollState()),horizontalArrangement=Arrangement.spacedBy(8.dp)){listOf("" to "全部","attention" to "需要处理","unknown" to "无法确认","ok" to "正常").forEach{(key,label)->FilterChip(selected=filter==key,onClick={filter=key},label={Text(label)})}}}
            listOf("local_dependencies" to "本地依赖","managed_content" to "受管内容","agent_runtimes" to "智能体").forEach{(group,title)->
                val checks=value["checks"].coreArray().filter{it.coreText("group")==group&&(filter.isEmpty()||it.coreText("status")==filter)}
                if(checks.isNotEmpty())item{Text(title,style=MaterialTheme.typography.titleSmall)}
                items(checks,key={it.coreText("id")}){check->GlassRow(Modifier.fillMaxWidth()){
                    var expanded by remember(check.coreText("id")){mutableStateOf(check.coreText("status")!="ok")}
                    Row{Text(check.coreText("label"),Modifier.weight(1f),style=MaterialTheme.typography.titleSmall);Text(diagnosticStatus(check.coreText("status")),style=MaterialTheme.typography.labelSmall,color=if(check.coreText("status")=="attention")CodegTheme.colors.danger else CodegTheme.colors.textSecondary)}
                    Text(check.coreText("detail"));if(check["stale"]==JsonPrimitive(true))Text("这是上次成功记录，当前状态尚未重新确认。",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)
                    TextButton(onClick={expanded=!expanded}){Text(if(expanded)"收起详情" else "查看详情")}
                    if(expanded){Text(check.coreText("code"),style=MaterialTheme.typography.labelSmall);Text(check.coreText("observedAt"),style=MaterialTheme.typography.labelSmall,color=CodegTheme.colors.textSecondary);check["facts"].coreArray().forEach{fact->Text(fact.coreText("key")+" · "+fact.coreText("value"),style=MaterialTheme.typography.bodySmall)}}
                    if(check.coreText("status")!="ok")when{
                        check.coreText("id")=="mcp-config"&&check.coreText("code")=="mcp_config_permissions_too_broad"->TextButton(enabled=!blocked,onClick={repair=true}){Text("修复MCP文件权限")}
                        check.coreText("id")=="mcp-config"->TextButton(onClick={settings="mcp"}){Text("前往MCP设置")}
                        check.coreText("subjectKind")=="runtime"&&check.coreText("status")=="unknown"&&check.coreText("subjectId").isNotEmpty()->TextButton(enabled=!blocked,onClick={busy=true;scope.launch{try{model.query(RovaiOperation.RUNTIME_PRODUCT_CHECK,buildJsonObject{put("runtimeKind",check.coreText("subjectId"))});require(generation==model.connectionGeneration);load()}catch(error:Exception){issue=error.message}finally{busy=false}}}){Text("重新检测智能体")}
                        check.coreText("subjectKind")=="runtime"->TextButton(onClick={settings="runtime"}){Text("前往智能体设置")}
                        check.coreText("id")=="legacy-skill-entries"->Text("旧入口需在主机桌面的诊断中心核对归属后清理。",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)
                        else->TextButton(enabled=!blocked,onClick={exportReport()}){Text("导出诊断JSON")}
                    }
                }}
            }
        }
    }
    if(repair)AlertDialog(onDismissRequest={if(!busy)repair=false},title={Text("修复主机MCP配置文件权限？")},text={Text("仅让原Core修复受管配置文件的访问权限，不更改服务器定义、队员选择或模型输入。完成后重新读取诊断，不能把调用完成当作问题已恢复。")},confirmButton={TextButton(enabled=!blocked,onClick={busy=true;scope.launch{try{model.query(RovaiOperation.MCP_CONFIG_REPAIRPERMISSIONS);require(generation==model.connectionGeneration);repair=false;load()}catch(error:Exception){issue=error.message}finally{busy=false}}}){Text("修复权限")}},dismissButton={TextButton(enabled=!busy,onClick={repair=false}){Text("取消")}})
}
