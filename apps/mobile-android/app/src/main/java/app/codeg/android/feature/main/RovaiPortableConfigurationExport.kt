package app.codeg.android.feature.main

import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.component.SecretField
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.*
import java.io.File
import java.util.UUID

@Composable
fun RovaiPortableConfigurationExport(model:RovaiWorkspaceViewModel,sourceArchive:JsonObject?=null,onClose:()->Unit){
    val context=LocalContext.current;val scope=rememberCoroutineScope();val state by model.state.collectAsStateWithLifecycle()
    var identity by remember{mutableStateOf("")};var pending by remember{mutableStateOf<JsonObject?>(null)};var receipt by remember{mutableStateOf<JsonObject?>(null)}
    var includeSecrets by remember{mutableStateOf(false)};var passphrase by remember{mutableStateOf("")};var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)};var message by remember{mutableStateOf("")}
    fun owner()="portable-export/$identity"+(sourceArchive?.let{"/original/${it.coreText("archiveId")}"}?:"")
    suspend fun current(){if(identity.isEmpty()||model.presentationIdentity()!=identity)throw CancellationException("工作区已切换")}
    suspend fun refresh(){val original=pending?:return;val value=model.privateConfiguration("export-receipt",buildJsonObject{put("operationId",original["operationId"]!!)});current();require(value["operationId"]==original["operationId"]){"原导出编号不同"};receipt=value;if(value.coreText("state")=="ready")passphrase=""}
    fun work(action:suspend()->Unit){if(busy)return;busy=true;issue=null;scope.launch{try{current();action()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message?:"导出未完成"}finally{busy=false}}}
    LaunchedEffect(state.connected){
        if(!state.connected){identity="";pending=null;receipt=null;passphrase="";includeSecrets=false;return@LaunchedEffect}
        busy=true
        try{identity=model.presentationIdentity();val raw=model.readBody(owner());if(raw.isNotBlank()){val value=Json.parseToJsonElement(raw).coreObject();require(value.keys.all{it in setOf("operationId","includeSecrets","sourceArchive")}&&value["sourceArchive"]==sourceArchive&&value["includeSecrets"]?.jsonPrimitive?.booleanOrNull!=null);UUID.fromString(value.coreText("operationId"));pending=value;includeSecrets=value["includeSecrets"]==JsonPrimitive(true);refresh()}}
        catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue="原导出恢复失败，记录已保留"}finally{busy=false}
    }
    val save=rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/octet-stream")){uri->if(uri!=null)work{
        val frozen=receipt?:error("请先准备副本");require(frozen.coreText("state")=="ready"){"请先准备副本"}
        val directory=File(context.cacheDir,"portable-configuration-export").apply{mkdirs()}.canonicalFile;require(directory.parentFile==context.cacheDir.canonicalFile){"配置缓存目录无效"}
        val file=File(directory,"export-${UUID.randomUUID()}.rovai-config")
        try{model.downloadConfigurationExport(frozen,file);current();withContext(Dispatchers.IO){context.contentResolver.openOutputStream(uri,"w")?.use{output->file.inputStream().use{it.copyTo(output,64*1024)}}?:error("无法保存到所选位置")};current();message="加密配置副本已保存"}finally{if(file.canonicalFile.parentFile==directory&&file.name.startsWith("export-"))withContext(Dispatchers.IO){file.delete()}}
    }}
    BackHandler{if(!busy)onClose()}
    LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(12.dp),verticalArrangement=Arrangement.spacedBy(10.dp)){
        item{TextButton(enabled=!busy,onClick=onClose){Text("返回")};Text(if(sourceArchive==null)"导出完整配置" else "保留原配置的完整副本",style=MaterialTheme.typography.titleLarge);Text(if(sourceArchive==null)"保存一份加密副本，到另一台主机再选择要搬入的内容和本机队员。" else "保存这份候选的完整原内容，包括尚不支持的字段；不会启用其中配置。")}
        if(receipt?.coreText("state")!="ready")item{GlassRow{
            Row{Text(if(sourceArchive==null)"同时包含密钥" else "确认原内容和可能包含的密钥一起加密保存",modifier=Modifier.weight(1f));Switch(checked=includeSecrets,onCheckedChange={includeSecrets=it},enabled=!busy&&pending==null)}
            SecretField(passphrase,{passphrase=it},"副本加密口令（至少8个字符）")
            Text(if(sourceArchive==null)"默认不含密钥。原生登录、服务令牌和本机安装锁不随配置迁移。" else "原副本可能包含原应用保存的敏感设置；明确勾选后才能导出，始终加密。",style=MaterialTheme.typography.bodySmall)
            TextButton(enabled=!busy&&identity.isNotEmpty()&&passphrase.codePointCount(0,passphrase.length)>=8&&(sourceArchive==null||includeSecrets),onClick={work{
                val original=pending?:buildJsonObject{put("operationId",UUID.randomUUID().toString());put("includeSecrets",includeSecrets);sourceArchive?.let{put("sourceArchive",it)}}
                if(pending==null){model.saveBody(owner(),original.toString());current();pending=original}
                val value=model.privateConfiguration("export",buildJsonObject{put("operationId",original["operationId"]!!);put("includeSecrets",original["includeSecrets"]!!);put("passphrase",passphrase);original["sourceArchive"]?.let{put("sourceArchive",it)}});current();require(value["operationId"]==original["operationId"]){"原导出编号不同"};receipt=value;if(value.coreText("state")=="ready")passphrase=""
            }}){Text(if(pending==null)"准备加密副本" else "继续准备原副本")}
        }}
        if(pending!=null)item{TextButton(enabled=!busy,onClick={work{refresh()}}){Text("核对原导出")}}
        if(receipt!=null)item{Text(when(receipt!!.coreText("state")){"ready"->"副本已准备好";"prepared"->"原副本仍待完成，请使用原口令继续";else->"尚未查到原导出结果，未自动重发"})}
        if(receipt?.coreText("state")=="ready")item{Row{
            TextButton(enabled=!busy,onClick={save.launch("configuration.rovai-config")}){Text("保存加密副本")}
            TextButton(enabled=!busy,onClick={work{model.saveBody(owner(),"");current();pending=null;receipt=null;includeSecrets=false;passphrase="";message=""}}){Text("另建一份副本")}
        }}
        if(message.isNotEmpty())item{Text(message)}
        if(issue!=null)item{Text(issue!!,color=MaterialTheme.colorScheme.error)}
        if(busy)item{LinearProgressIndicator(Modifier.fillMaxWidth())}
    }
}
