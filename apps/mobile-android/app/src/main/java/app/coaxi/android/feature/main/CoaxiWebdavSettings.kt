package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

import androidx.compose.foundation.layout.*
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import app.coaxi.android.core.designsystem.component.GlassRow
import app.coaxi.android.core.designsystem.theme.CoaxiTheme
import app.coaxi.android.core.network.CoaxiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*

/** Coaxi account/snapshot rows, controlling the connected Host's saved WebDAV connection. */
@Composable fun CoaxiWebdavSettings(model:CoaxiWorkspaceViewModel,onClose:()->Unit){
    val scope=rememberCoroutineScope();val generation=model.connectionGeneration
    var configurationSync by remember{mutableStateOf(false)}
    if(configurationSync){CoaxiConfigurationSyncSettings(model){configurationSync=false};return}
    var saved by remember{mutableStateOf(buildJsonObject{})};var endpoint by remember{mutableStateOf("")};var username by remember{mutableStateOf("")};var password by remember{mutableStateOf("")}
    var clearPassword by remember{mutableStateOf(false)};var insecure by remember{mutableStateOf(false)};var dirty by remember{mutableStateOf(false)}
    var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)};var message by remember{mutableStateOf<String?>(null)}
    var snapshots by remember{mutableStateOf<List<JsonObject>>(emptyList())};var transfers by remember{mutableStateOf<List<JsonObject>>(emptyList())}
    var confirm by remember{mutableStateOf<JsonObject?>(null)};var removals by remember{mutableStateOf<List<JsonObject>>(emptyList())};var leaving by remember{mutableStateOf(false)}
    val owner="webdav-remote-removals"
    suspend fun request(operation:CoaxiOperation,input:JsonObject):JsonObject{val value=model.query(operation,input).coreObject();require(generation==model.connectionGeneration){"工作区已切换"};return value}
    suspend fun remote(action:String,extra:JsonObject=buildJsonObject{}):JsonObject=request(CoaxiOperation.BACKUP_WEBDAV_REMOTE,JsonObject(buildJsonObject{put("operation",action);put("expectedRevision",saved["revision"]?:JsonNull)}+extra))
    fun accept(value:JsonObject){saved=value;endpoint=value.coreText("endpoint");username=value.coreText("username");insecure=value["allowInsecure"]==JsonPrimitive(true);password="";clearPassword=false;dirty=false}
    fun work(action:suspend ()->Unit){if(busy)return;busy=true;issue=null;message=null;scope.launch{try{action()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}
    suspend fun list(){snapshots=remote("list")["entries"].coreArray();transfers=remote("transfers")["items"].coreArray()}
    suspend fun keepRemovals(next:List<JsonObject>){model.saveBody(owner,JsonArray(next).toString());require(generation==model.connectionGeneration);removals=next}
    suspend fun remove(frozen:JsonObject){keepRemovals((removals.filter{it.coreText("snapshotId")!=frozen.coreText("snapshotId")}+frozen));request(CoaxiOperation.BACKUP_WEBDAV_REMOTE,frozen);keepRemovals(removals.filter{it.coreText("snapshotId")!=frozen.coreText("snapshotId")});list();message="已移除远端索引中的副本；本地备份保持原样。"}
    LaunchedEffect(generation){busy=true;try{accept(request(CoaxiOperation.BACKUP_WEBDAV_SETTINGS,buildJsonObject{put("operation","get")}));val raw=model.readBody(owner);removals=if(raw.isBlank())emptyList()else Json.parseToJsonElement(raw).coreArray()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    BackHandler(enabled=dirty||busy){if(!busy)leaving=true}
    Column(Modifier.fillMaxSize()){
        Row(Modifier.fillMaxWidth().padding(12.dp)){Text(stringResource(R.string.coaxi_fusion_acde8c3e5ad0a8e3),Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(enabled=!busy,onClick={if(dirty)leaving=true else onClose()}){Text(stringResource(R.string.coaxi_fusion_572cf45ba43634b3))}}
        LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
            item{TextButton(enabled=!busy&&!dirty,onClick={configurationSync=true}){Text(stringResource(R.string.coaxi_fusion_01afdad9f555e416))}}
            item{issue?.let{Text(it,color=CoaxiTheme.colors.danger)};message?.let{Text(it,color=CoaxiTheme.colors.textSecondary)}}
            item{GlassRow{
                Text(stringResource(R.string.coaxi_fusion_c3be26441a356ba0),style=MaterialTheme.typography.bodySmall,color=CoaxiTheme.colors.textSecondary)
                OutlinedTextField(endpoint,{endpoint=it;dirty=true},Modifier.fillMaxWidth(),label={Text(stringResource(R.string.coaxi_fusion_8d3184e8c7cd544c))},singleLine=true,enabled=!busy)
                OutlinedTextField(username,{username=it;dirty=true},Modifier.fillMaxWidth(),label={Text(stringResource(R.string.coaxi_fusion_1a3f0617d6de8e52))},singleLine=true,enabled=!busy)
                OutlinedTextField(password,{password=it;dirty=true},Modifier.fillMaxWidth(),label={Text(if(saved["passwordSet"]==JsonPrimitive(true))"密码已保存；留空保留" else "密码")},visualTransformation=PasswordVisualTransformation(),singleLine=true,enabled=!busy)
                Row{Checkbox(clearPassword,{clearPassword=it;dirty=true},enabled=!busy);Text(stringResource(R.string.coaxi_fusion_e844c50e14056de2))}
                Row{Switch(insecure,{insecure=it;dirty=true},enabled=!busy);Text(stringResource(R.string.coaxi_fusion_747a8f3d4a9c1975))}
                Button(enabled=!busy&&dirty&&endpoint.isNotBlank(),onClick={confirm=buildJsonObject{put("operation","set");put("expectedRevision",saved["revision"]?:JsonNull);put("endpoint",endpoint);put("username",username);put("password",password);put("clearPassword",clearPassword);put("allowInsecure",insecure)}}){Text(stringResource(R.string.coaxi_fusion_d9ebefb4a63d7871))}
            }}
            item{Row(horizontalArrangement=Arrangement.spacedBy(8.dp)){TextButton(enabled=!busy&&!dirty&&saved["revision"]!=null&&saved["revision"]!=JsonNull,onClick={work{remote("probe");message="连接成功"}}){Text(stringResource(R.string.coaxi_fusion_0a98a48eaaac4175))};TextButton(enabled=!busy&&!dirty&&saved["revision"]!=null&&saved["revision"]!=JsonNull,onClick={work{list()}}){Text(stringResource(R.string.coaxi_fusion_91bc0eed2da4206d))}}}
            if(saved.coreText("revision").isNotEmpty())item{CoaxiWebdavRetention(model,saved.coreText("revision"),busy||dirty){work{list()}}}
            items(removals,key={"pending:"+it.coreText("snapshotId")}){frozen->GlassRow{Text(stringResource(R.string.coaxi_fusion_83e2e900534aadcc),style=MaterialTheme.typography.titleSmall);Text(frozen.coreText("snapshotId"),style=MaterialTheme.typography.labelSmall);Row{
                TextButton(enabled=!busy,onClick={work{val value=request(CoaxiOperation.BACKUP_WEBDAV_REMOTE,JsonObject(frozen.toMutableMap().apply{put("operation",JsonPrimitive("removal_status"));remove("confirmed");remove("expectedIndexEtag")}));if(value["removed"]==JsonPrimitive(true)){keepRemovals(removals.filter{it.coreText("snapshotId")!=frozen.coreText("snapshotId")});message="原移除已完成"}else message="原副本仍在或结果未知；保留原请求，可稍后核对"}}){Text(stringResource(R.string.coaxi_fusion_d2a9cfb49bed73f5))}
                TextButton(enabled=!busy,onClick={confirm=frozen}){Text(stringResource(R.string.coaxi_fusion_5a66cdc8a4bc8ccb))}
            }}}
            items(snapshots,key={it.coreText("id")}){snapshot->GlassRow{Text(if(snapshot.coreText("kind")=="configuration")"配置快照" else "完整备份",style=MaterialTheme.typography.titleSmall);Text(snapshot.coreText("createdAt"));Text("${snapshot.coreLong("bytes")} 字节",style=MaterialTheme.typography.bodySmall,color=CoaxiTheme.colors.textSecondary);TextButton(enabled=!busy&&!dirty,onClick={work{val preview=remote("removal_preview",buildJsonObject{put("snapshotId",snapshot.coreText("id"));put("expectedDigest",snapshot.coreText("sha256"))});confirm=buildJsonObject{put("operation","remove");put("expectedRevision",saved["revision"]?:JsonNull);put("snapshotId",snapshot.coreText("id"));put("expectedDigest",snapshot.coreText("sha256"));put("expectedIndexEtag",preview.coreText("indexEtag"));put("confirmed",true)}}}){Text(stringResource(R.string.coaxi_fusion_e5338e1e51152703))}}}
            items(transfers, key = { it.coreText("commandId") }) { transfer ->
                GlassRow {
                    Text((if (transfer.coreText("operation") == "push") "上传" else "下载") + " · " + transfer.coreText("state"))
                    if (transfer.coreText("message").isNotEmpty()) {
                        Text(transfer.coreText("message"), style = MaterialTheme.typography.bodySmall)
                    }
                    Row {
                        TextButton(enabled = !busy, onClick = {
                            work {
                                val value = remote("status", buildJsonObject {
                                    put("commandId", transfer.coreText("commandId"))
                                })
                                transfers = transfers.map { old ->
                                    if (old.coreText("commandId") == transfer.coreText("commandId")) value else old
                                }
                            }
                        }) { Text(stringResource(R.string.coaxi_fusion_aca2df8feb90ba8a)) }
                        if (transfer.coreText("state") in listOf("started", "stopping")) {
                            TextButton(enabled = !busy, onClick = {
                                confirm = buildJsonObject {
                                    put("operation", "cancel")
                                    put("expectedRevision", saved["revision"] ?: JsonNull)
                                    put("commandId", transfer.coreText("commandId"))
                                }
                            }) { Text(stringResource(R.string.coaxi_fusion_97319c6b91e3ac45)) }
                        }
                    }
                }
            }
        }
    }
    confirm?.let{frozen->AlertDialog(onDismissRequest={if(!busy)confirm=null},title={Text(when(frozen.coreText("operation")){"set"->"保存到连接的主机？";"remove"->"移除此远端副本？";else->"停止原传输？"})},text={Text(when(frozen.coreText("operation")){"set"->"以打开此页时的账号版本保存。其他端修改过时拒绝覆盖。";"remove"->"按刚才预览的远端索引版本移除；保留本地数据与已下载文件。结果未知时继续核对同一请求。";else->"停止不代表远端操作已经撤销，随后核对原任务。"})},confirmButton={TextButton(enabled=!busy,onClick={work{when(frozen.coreText("operation")){"set"->{accept(request(CoaxiOperation.BACKUP_WEBDAV_SETTINGS,frozen));message="主机连接已保存"};"remove"->remove(frozen);else->{request(CoaxiOperation.BACKUP_WEBDAV_REMOTE,frozen);transfers=remote("transfers")["items"].coreArray()}};confirm=null}}){Text(stringResource(R.string.coaxi_fusion_36f33adaf0942634))}},dismissButton={TextButton(enabled=!busy,onClick={confirm=null}){Text(stringResource(R.string.coaxi_fusion_2cd0f3be8738a86c))}})}
    if(leaving)AlertDialog(onDismissRequest={leaving=false},title={Text(stringResource(R.string.coaxi_fusion_d702e151061efe75))},text={Text(stringResource(R.string.coaxi_fusion_4c0897452c4428f6))},confirmButton={TextButton(onClick={password="";onClose()}){Text(stringResource(R.string.coaxi_fusion_9b7824cefa1e8b16))}},dismissButton={TextButton(onClick={leaving=false}){Text(stringResource(R.string.coaxi_fusion_fd4b9e3b6c685bae))}})
}
