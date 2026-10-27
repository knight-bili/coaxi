package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.coaxi.android.core.designsystem.component.GlassRow
import app.coaxi.android.core.designsystem.theme.CoaxiTheme
import app.coaxi.android.core.network.CoaxiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*
import java.util.UUID

@Composable fun CoaxiWebdavRetention(model:CoaxiWorkspaceViewModel,revision:String,disabled:Boolean,onChanged:()->Unit){
    val scope=rememberCoroutineScope();val generation=model.connectionGeneration;val owner="webdav-retention-plan"
    var backups by remember{mutableStateOf("10")};var configurations by remember{mutableStateOf("20")};var busy by remember{mutableStateOf(false)};var ready by remember{mutableStateOf(false)}
    var saved by remember{mutableStateOf<JsonObject?>(null)};var issue by remember{mutableStateOf<String?>(null)};var message by remember{mutableStateOf<String?>(null)};var confirm by remember{mutableStateOf<JsonObject?>(null)}
    var cleanup by remember{mutableStateOf<List<JsonObject>>(emptyList())};var cursor by remember{mutableStateOf<String?>(null)}
    suspend fun request(input:JsonObject):JsonObject{val result=model.query(CoaxiOperation.BACKUP_WEBDAV_REMOTE,input).coreObject();require(generation==model.connectionGeneration){"工作区已切换"};return result}
    fun input(action:String,extra:JsonObject=buildJsonObject{})=JsonObject(buildJsonObject{put("operation",action);put("expectedRevision",revision)}+extra)
    suspend fun store(value:JsonObject?){model.saveBody(owner,value?.toString().orEmpty());require(generation==model.connectionGeneration);saved=value}
    fun work(action:suspend ()->Unit){if(busy||disabled||!ready)return;busy=true;issue=null;message=null;scope.launch{try{action()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}
    suspend fun preview(original:JsonObject){val result=request(original);store(buildJsonObject{put("request",original);put("result",result);put("phase","previewed")})}
    suspend fun scan(after:String?=null){val result=request(input("cleanup_preview",buildJsonObject{if(after!=null)put("snapshotId",after)}));cleanup=if(after==null)result["items"].coreArray()else(cleanup+result["items"].coreArray()).distinctBy{it["snapshot"].coreObject().coreText("id")};cursor=(result["nextCursor"]as?JsonPrimitive)?.contentOrNull}
    LaunchedEffect(generation){try{val raw=model.readBody(owner);saved=raw.takeIf{it.isNotBlank()}?.let{Json.parseToJsonElement(it).coreObject()};ready=true}catch(error:Exception){issue=error.message}}
    GlassRow(Modifier.fillMaxWidth()){
        Text(stringResource(R.string.coaxi_fusion_c50516f2bfddabd2),style=MaterialTheme.typography.titleSmall);Text(stringResource(R.string.coaxi_fusion_86c3199f67478c3f),style=MaterialTheme.typography.bodySmall,color=CoaxiTheme.colors.textSecondary)
        OutlinedTextField(backups,{backups=it.filter(Char::isDigit)},Modifier.fillMaxWidth(),label={Text(stringResource(R.string.coaxi_fusion_ac5bbcbd72e39578))},singleLine=true,enabled=!disabled&&!busy&&saved==null)
        OutlinedTextField(configurations,{configurations=it.filter(Char::isDigit)},Modifier.fillMaxWidth(),label={Text(stringResource(R.string.coaxi_fusion_9e69b82ccc8a85ba))},singleLine=true,enabled=!disabled&&!busy&&saved==null)
        Row{TextButton(enabled=ready&&!disabled&&!busy&&saved==null&&backups.toIntOrNull()in 1..1000&&configurations.toIntOrNull()in 1..1000,onClick={work{val original=input("retention_preview",buildJsonObject{put("commandId",UUID.randomUUID().toString());put("keepBackups",backups.toInt());put("keepConfigurations",configurations.toInt())});store(buildJsonObject{put("request",original);put("phase","preparing")});preview(original)}}){Text(stringResource(R.string.coaxi_fusion_90f0614de177d8c3))};TextButton(enabled=ready&&!disabled&&!busy,onClick={work{scan()}}){Text(stringResource(R.string.coaxi_fusion_945a5061fe5e1b5a))}}
        issue?.let{Text(it,color=CoaxiTheme.colors.danger)};message?.let{Text(it,color=CoaxiTheme.colors.textSecondary)}
        saved?.let{current->val original=current["request"].coreObject();val plan=current["result"].coreObject()["plan"].coreObject();Text("${plan["delete"].coreArray().size}份待移除；之后还有${plan.coreLong("remaining")}份可重新预览",style=MaterialTheme.typography.bodySmall);plan["delete"].coreArray().forEach{Text((if(it.coreText("kind")=="configuration")"配置快照 · " else "完整备份 · ")+it.coreText("createdAt"),style=MaterialTheme.typography.bodySmall)}
            Row{TextButton(enabled=!disabled&&!busy,onClick={work{val status=request(JsonObject(original.toMutableMap().apply{put("operation",JsonPrimitive("retention_status"));remove("keepBackups");remove("keepConfigurations")}));if(status["removed"]==JsonPrimitive(true)){store(null);message="本批移除已完成";onChanged()}else message="原批次尚未完成；保留原请求"}}){Text(stringResource(R.string.coaxi_fusion_10565842efb87467))}
                if(current.coreText("phase")=="preparing")TextButton(enabled=!disabled&&!busy,onClick={work{preview(original)}}){Text(stringResource(R.string.coaxi_fusion_33a6c49ef4ee5791))}
                else TextButton(enabled=!disabled&&!busy&&plan["delete"].coreArray().isNotEmpty(),onClick={confirm=JsonObject(original.toMutableMap().apply{put("operation",JsonPrimitive("retention_apply"));put("confirmed",JsonPrimitive(true));remove("keepBackups");remove("keepConfigurations")})}){Text(stringResource(R.string.coaxi_fusion_9fbc35575aea3bc5))}
            }
            if(current.coreText("phase")=="previewed")TextButton(enabled=!disabled&&!busy,onClick={work{store(null)}}){Text(stringResource(R.string.coaxi_fusion_6538b75e96d82b28))}
        }
        cleanup.forEach{entry->val snapshot=entry["snapshot"].coreObject();Row{Text(snapshot.coreText("createdAt"),Modifier.weight(1f),style=MaterialTheme.typography.bodySmall);TextButton(enabled=!disabled&&!busy,onClick={confirm=input("cleanup_removed",buildJsonObject{put("snapshotId",snapshot.coreText("id"));put("expectedDigest",snapshot.coreText("sha256"));put("confirmed",true)})}){Text(stringResource(R.string.coaxi_fusion_64f20b092de55c46))}}}
        cursor?.let{after->TextButton(enabled=!disabled&&!busy,onClick={work{scan(after)}}){Text(stringResource(R.string.coaxi_fusion_7d0079c0b220d9f9))}}
    }
    confirm?.let{frozen->AlertDialog(onDismissRequest={if(!busy)confirm=null},title={Text(if(frozen.coreText("operation")=="retention_apply")"移除预览中的这一批？" else "清理已移除副本的文件？")},text={Text(stringResource(R.string.coaxi_fusion_7ad242dabac67304))},confirmButton={TextButton(enabled=!busy&&!disabled,onClick={work{if(frozen.coreText("operation")=="retention_apply"){store(JsonObject(saved!!.toMutableMap().apply{put("phase",JsonPrimitive("applying"))}));request(frozen);store(null);onChanged()}else{request(frozen);scan()};confirm=null}}){Text(stringResource(R.string.coaxi_fusion_36f33adaf0942634))}},dismissButton={TextButton(enabled=!busy,onClick={confirm=null}){Text(stringResource(R.string.coaxi_fusion_2cd0f3be8738a86c))}})}
}
