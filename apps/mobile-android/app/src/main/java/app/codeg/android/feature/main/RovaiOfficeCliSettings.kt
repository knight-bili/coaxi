package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*
import java.util.UUID

/** Original OfficeCLI detection/managed install UI, operating on the connected Host. */
@Composable fun RovaiOfficeCliSettings(model:RovaiWorkspaceViewModel){
    val coroutine=rememberCoroutineScope();val generation=model.connectionGeneration;val owner="office-cli-operation"
    var info by remember(generation){mutableStateOf<JsonObject?>(null)};var task by remember(generation){mutableStateOf<JsonObject?>(null)};var busy by remember{mutableStateOf(false)};var confirm by remember{mutableStateOf<String?>(null)};var issue by remember{mutableStateOf<String?>(null)};var expanded by remember{mutableStateOf(false)}
    suspend fun call(operation:String,extra:JsonObject=buildJsonObject{}):JsonObject{val result=model.query(RovaiOperation.OFFICE_CLI,buildJsonObject{put("operation",operation);extra.forEach{(key,value)->put(key,value)}}).coreObject();require(generation==model.connectionGeneration){"工作区已切换"};return result}
    fun work(action:suspend ()->Unit){if(busy)return;busy=true;issue=null;coroutine.launch{try{action()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}
    LaunchedEffect(generation){try{info=call("detect");val saved=model.readBody(owner);if(saved.isNotBlank()){val value=Json.parseToJsonElement(saved).coreObject();task=call("status",buildJsonObject{put("commandId",value.coreText("commandId"))})}}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}}
    LaunchedEffect(task?.coreText("commandId"),task?.coreText("state")){val id=task?.coreText("commandId")?:return@LaunchedEffect;while(task?.coreText("state")=="running"){delay(1200);try{val value=call("status",buildJsonObject{put("commandId",id)});task=value;(value["info"]as?JsonObject)?.let{info=it}}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message;break}}}
    Column(verticalArrangement=Arrangement.spacedBy(8.dp)){
        Row{Text("OfficeCLI",Modifier.weight(1f),style=MaterialTheme.typography.titleSmall);Text(when{info==null->"正在检测…";info?.get("runnable")==JsonPrimitive(true)->"可以运行";info?.get("installed")==JsonPrimitive(true)->"已安装，运行异常";else->"未安装"},style=MaterialTheme.typography.bodySmall)}
        val running=task?.coreText("state")=="running"
        Row{TextButton(enabled=!busy&&!running,onClick={work{info=call("detect")}}){Text(stringResource(R.string.rovai_fusion_a13550662feade03))};TextButton(enabled=!busy&&!running,onClick={confirm="install"}){Text(if(info?.get("installed")==JsonPrimitive(true))"重新安装" else "安装")};TextButton(enabled=!busy&&!running&&info?.get("installed")==JsonPrimitive(true),onClick={confirm="uninstall"}){Text(stringResource(R.string.rovai_fusion_06bc14b60f3598a3))};if(running)TextButton(enabled=!busy,onClick={work{task=call("cancel",buildJsonObject{put("commandId",task!!.coreText("commandId"))})}}){Text(stringResource(R.string.rovai_fusion_191a09e32e312bc4))}}
        info?.coreText("version")?.takeIf{it.isNotBlank()}?.let{Text(it,style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)};info?.coreText("runtimeError")?.takeIf{it.isNotBlank()}?.let{Text(it,color=CodegTheme.colors.danger)};issue?.let{Text(it,color=CodegTheme.colors.danger)}
        task?.let{value->TextButton(onClick={expanded=!expanded}){Text(when(value.coreText("state")){"running"->"正在执行";"completed"->"操作已完成";"not_found"->"未找到原操作";"unknown"->"结果待核对";else->"操作未完成"})};if(value.coreText("state")=="unknown"||value.coreText("state")=="not_found")TextButton(enabled=!busy,onClick={work{task=call("status",buildJsonObject{put("commandId",value.coreText("commandId"))})}}){Text(stringResource(R.string.rovai_fusion_3107a8f862e45a45))};if(expanded){if(value.coreText("error").isNotBlank())Text(value.coreText("error"),color=CodegTheme.colors.danger);SelectionContainer{Text(value.coreText("log"),Modifier.heightIn(max=260.dp).verticalScroll(rememberScrollState()),style=MaterialTheme.typography.bodySmall.copy(fontFamily=androidx.compose.ui.text.font.FontFamily.Monospace))}}}
    }
    confirm?.let{operation->AlertDialog(onDismissRequest={if(!busy)confirm=null},title={Text(if(operation=="install")"在工作区主机安装OfficeCLI？" else "卸载工作区主机的OfficeCLI？")},text={Text(if(operation=="install")"将在当前工作区所在电脑运行OfficeCLI官方安装脚本，并写入用户程序目录。过程会下载程序，安装日志保留在这里。" else "仅移除主机用户程序目录中的OfficeCLI。技能、任务历史和文档保留。")},confirmButton={TextButton(enabled=!busy,onClick={work{val id=UUID.randomUUID().toString();val saved=buildJsonObject{put("commandId",id)}.toString();model.rememberBody(owner,saved);model.saveBody(owner,saved);task=buildJsonObject{put("commandId",id);put("state","unknown")};task=call(operation,buildJsonObject{put("commandId",id);put("confirmed",true);put("expectedDigest",info?.get("digest")?:JsonNull)});confirm=null}}){Text(stringResource(R.string.rovai_fusion_68a35e2242686dcc))}},dismissButton={TextButton(enabled=!busy,onClick={confirm=null}){Text(stringResource(R.string.rovai_fusion_2cd0f3be8738a86c))}})}
}
