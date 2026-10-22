package app.codeg.android.feature.main

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.component.SecretField
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import java.util.UUID

/** 操作同一主机现有配置同步任务，不在手机上另起自动上传器。 */
@Composable
fun RovaiConfigurationSyncSettings(model:RovaiWorkspaceViewModel,onClose:()->Unit){
    val scope=rememberCoroutineScope();val generation=model.connectionGeneration
    var identity by remember{mutableStateOf("")};var saved by remember{mutableStateOf(buildJsonObject{})};var draft by remember{mutableStateOf(buildJsonObject{})}
    var passphrase by remember{mutableStateOf("")};var clear by remember{mutableStateOf(false)};var dirty by remember{mutableStateOf(false)};var busy by remember{mutableStateOf(false)}
    var pending by remember{mutableStateOf<JsonObject?>(null)};var issue by remember{mutableStateOf<String?>(null)};var message by remember{mutableStateOf("")};var leaving by remember{mutableStateOf(false)};var archive by remember{mutableStateOf(false)}
    fun owner()="portable-sync/$identity"
    suspend fun current(){if(generation!=model.connectionGeneration||identity.isEmpty()||model.presentationIdentity()!=identity)throw CancellationException("工作区已切换")}
    suspend fun call(operation:String,extra:JsonObject=buildJsonObject{}):JsonObject{val result=model.privateConfiguration("sync",JsonObject(extra.toMutableMap().apply{put("operation",JsonPrimitive(operation))}));current();return result}
    fun accept(value:JsonObject){saved=value;draft=value;passphrase="";clear=false;dirty=false}
    suspend fun refresh(){val value=call("get");saved=value;if(!dirty)draft=value}
    suspend fun remember(value:JsonObject?){model.saveBody(owner(),value?.toString()?:"");current();pending=value}
    suspend fun reconcile(){val original=pending?:return;val result=call("receipt",buildJsonObject{put("commandId",original["commandId"]!!)});require(result["commandId"]==original["commandId"]){"原上传编号不同"};if(result.coreText("state")=="completed"){remember(null);message="原配置上传已完成"}else message=if(result.coreText("state")=="preparing")"主机正在准备原副本" else "原上传尚待核对，未新建任务";refresh()}
    fun work(action:suspend()->Unit){if(busy)return;busy=true;issue=null;scope.launch{try{current();action()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message?:"配置同步未完成"}finally{busy=false}}}
    fun field(name:String,value:JsonElement){draft=JsonObject(draft.toMutableMap().apply{put(name,value);if(name=="fullConfiguration"&&value==JsonPrimitive(false))put("includeSecrets",JsonPrimitive(false))});dirty=true}
    LaunchedEffect(generation){busy=true;try{identity=model.presentationIdentity();val raw=model.readBody(owner());if(raw.isNotBlank()){val value=Json.parseToJsonElement(raw).coreObject();require(value.keys==setOf("commandId","expectedRevision"));UUID.fromString(value.coreText("commandId"));pending=value};accept(call("get"));if(pending!=null)reconcile()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue="同步设置或原请求恢复失败"}finally{busy=false}}
    fun close(){if(dirty)leaving=true else onClose()}
    BackHandler{if(!busy)close()}
    val active=saved["state"].coreObject().coreText("activeCommandId");val phase=saved["state"].coreObject().coreText("phase")
    val blocked=busy||identity.isEmpty();val interval=(draft["intervalMinutes"] as? JsonPrimitive)?.intOrNull?:0
    LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(12.dp),verticalArrangement=Arrangement.spacedBy(10.dp)){
        item{Row{TextButton(enabled=!busy,onClick={close()}){Text("返回")};TextButton(enabled=!blocked,onClick={work{refresh()}}){Text("刷新主机状态")}};Text("主机配置自动上传",style=MaterialTheme.typography.titleLarge);Text("默认仅上传界面偏好。完整配置和密钥分别选择；另一台主机下载后仍须逐项选择，不自动工作。")}
        item{GlassRow{
            listOf("enabled" to "启用这台主机的配置上传","autoSync" to "自动上传有变化的配置","fullConfiguration" to "同时上传完整的模型、智能体和模板配置","includeSecrets" to "完整加密副本中包含密钥").forEach{(name,label)->Row{Text(label,modifier=Modifier.weight(1f));Switch(checked=draft[name]==JsonPrimitive(true),onCheckedChange={field(name,JsonPrimitive(it))},enabled=!blocked&&active.isEmpty()&&(name!="includeSecrets"||draft["fullConfiguration"]==JsonPrimitive(true)))}}
            OutlinedTextField(value=draft.coreText("intervalMinutes"),onValueChange={field("intervalMinutes",it.toIntOrNull()?.let(::JsonPrimitive)?:JsonPrimitive(it))},label={Text("检查间隔（1至1440分钟）")},enabled=!blocked&&active.isEmpty())
            SecretField(passphrase,{passphrase=it;dirty=true},if(saved["passphraseSet"]==JsonPrimitive(true))"新加密口令（留空保留原口令）" else "配置加密口令（至少8个字符）")
            Row{Checkbox(checked=clear,onCheckedChange={clear=it;dirty=true},enabled=!blocked&&active.isEmpty());Text("清除已存的加密口令")}
            Row{
                TextButton(enabled=!blocked&&active.isEmpty()&&dirty&&interval in 1..1440,onClick={work{val value=call("set",buildJsonObject{put("expectedRevision",draft["revision"]?:JsonNull);for(name in listOf("enabled","autoSync","fullConfiguration","includeSecrets","intervalMinutes"))put(name,draft[name]?:if(name=="intervalMinutes")JsonPrimitive(5) else JsonPrimitive(false));put("passphrase",passphrase);put("clearPassphrase",clear)});accept(value);message="配置同步设置已保存"}}){Text("保存设置")}
                TextButton(enabled=!blocked&&dirty,onClick={accept(saved)}){Text("放弃未保存修改")}
            }
        }}
        item{GlassRow{
            Text(if(active.isEmpty())"主机当前没有活动上传" else "原上传 · $phase")
            saved["state"].coreObject().coreText("lastSyncAt").takeIf(String::isNotEmpty)?.let{Text("上次实际上传 · $it",style=MaterialTheme.typography.bodySmall)}
            if(pending!=null){Text("本次原请求已保留，不自动另建上传。");TextButton(enabled=!blocked,onClick={work{reconcile()}}){Text("核对原上传")}}
            TextButton(enabled=!blocked&&!dirty&&active.isEmpty()&&saved["enabled"]==JsonPrimitive(true)&&saved["passphraseSet"]==JsonPrimitive(true),onClick={work{val original=pending?:buildJsonObject{put("commandId",UUID.randomUUID().toString());put("expectedRevision",saved["revision"]?:JsonNull)};if(pending==null)remember(original);call("upload",original);refresh()}}){Text(if(pending==null)"立即上传当前配置" else "继续原编号上传")}
            if(active.isNotEmpty())Row{
                TextButton(enabled=!blocked,onClick={work{call("cancel",buildJsonObject{put("expectedRevision",saved["revision"]?:JsonNull);put("commandId",active)});refresh()}}){Text("停止原任务")}
                if(phase=="unknown")TextButton(enabled=!blocked,onClick={work{call("resume",buildJsonObject{put("expectedRevision",saved["revision"]?:JsonNull);put("commandId",active)});refresh()}}){Text("恢复原上传")}
            }
            if((phase=="unknown"||pending!=null)&&!dirty)TextButton(enabled=!blocked,onClick={archive=true}){Text("保留未决记录后另建上传…")}
            saved["state"].coreObject().coreText("lastError").takeIf(String::isNotEmpty)?.let{Text(it,color=MaterialTheme.colorScheme.error)}
        }}
        if(message.isNotEmpty())item{Text(message)};if(issue!=null)item{Text(issue!!,color=MaterialTheme.colorScheme.error)};if(busy)item{LinearProgressIndicator(Modifier.fillMaxWidth())}
    }
    if(leaving)AlertDialog(onDismissRequest={leaving=false},title={Text("放弃未保存修改？")},dismissButton={TextButton(onClick={leaving=false}){Text("继续编辑")}},confirmButton={TextButton(onClick=onClose){Text("放弃并返回")}})
    if(archive)AlertDialog(onDismissRequest={if(!busy)archive=false},title={Text("保留原记录并继续新上传")},text={Text("会保留原任务的未决记录，之后的上传使用新编号。远端可能已有原副本；正在运行的任务不能直接跳过。")},dismissButton={TextButton(enabled=!busy,onClick={archive=false}){Text("返回")}},confirmButton={TextButton(enabled=!blocked,onClick={archive=false;work{
        if(active.isNotEmpty())call("archive",buildJsonObject{put("expectedRevision",saved["revision"]?:JsonNull);put("commandId",active);put("confirmed",true)})
        pending?.let{model.saveBody(owner()+"/archived/"+it.coreText("commandId"),it.toString())};remember(null);accept(call("get"));message="未决记录已保留，可明确开始下一次上传"
    }}){Text("确认保留记录并继续")}})
}
