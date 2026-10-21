package app.codeg.android.feature.main

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*
import java.util.UUID

@Composable fun RovaiWebdavRetention(model:RovaiWorkspaceViewModel,revision:String,disabled:Boolean,onChanged:()->Unit){
    val scope=rememberCoroutineScope();val generation=model.connectionGeneration;val owner="webdav-retention-plan"
    var backups by remember{mutableStateOf("10")};var configurations by remember{mutableStateOf("20")};var busy by remember{mutableStateOf(false)};var ready by remember{mutableStateOf(false)}
    var saved by remember{mutableStateOf<JsonObject?>(null)};var issue by remember{mutableStateOf<String?>(null)};var message by remember{mutableStateOf<String?>(null)};var confirm by remember{mutableStateOf<JsonObject?>(null)}
    var cleanup by remember{mutableStateOf<List<JsonObject>>(emptyList())};var cursor by remember{mutableStateOf<String?>(null)}
    suspend fun request(input:JsonObject):JsonObject{val result=model.query(RovaiOperation.BACKUP_WEBDAV_REMOTE,input).coreObject();require(generation==model.connectionGeneration){"工作区已切换"};return result}
    fun input(action:String,extra:JsonObject=buildJsonObject{})=JsonObject(buildJsonObject{put("operation",action);put("expectedRevision",revision)}+extra)
    suspend fun store(value:JsonObject?){model.saveBody(owner,value?.toString().orEmpty());require(generation==model.connectionGeneration);saved=value}
    fun work(action:suspend ()->Unit){if(busy||disabled||!ready)return;busy=true;issue=null;message=null;scope.launch{try{action()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}
    suspend fun preview(original:JsonObject){val result=request(original);store(buildJsonObject{put("request",original);put("result",result);put("phase","previewed")})}
    suspend fun scan(after:String?=null){val result=request(input("cleanup_preview",buildJsonObject{if(after!=null)put("snapshotId",after)}));cleanup=if(after==null)result["items"].coreArray()else(cleanup+result["items"].coreArray()).distinctBy{it["snapshot"].coreObject().coreText("id")};cursor=(result["nextCursor"]as?JsonPrimitive)?.contentOrNull}
    LaunchedEffect(generation){try{val raw=model.readBody(owner);saved=raw.takeIf{it.isNotBlank()}?.let{Json.parseToJsonElement(it).coreObject()};ready=true}catch(error:Exception){issue=error.message}}
    GlassRow(Modifier.fillMaxWidth()){
        Text("远端副本保留",style=MaterialTheme.typography.titleSmall);Text("先预览；每批最多移除20份。已移除副本的文件可另行清理。",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)
        OutlinedTextField(backups,{backups=it.filter(Char::isDigit)},Modifier.fillMaxWidth(),label={Text("保留完整备份数量")},singleLine=true,enabled=!disabled&&!busy&&saved==null)
        OutlinedTextField(configurations,{configurations=it.filter(Char::isDigit)},Modifier.fillMaxWidth(),label={Text("保留配置快照数量")},singleLine=true,enabled=!disabled&&!busy&&saved==null)
        Row{TextButton(enabled=ready&&!disabled&&!busy&&saved==null&&backups.toIntOrNull()in 1..1000&&configurations.toIntOrNull()in 1..1000,onClick={work{val original=input("retention_preview",buildJsonObject{put("commandId",UUID.randomUUID().toString());put("keepBackups",backups.toInt());put("keepConfigurations",configurations.toInt())});store(buildJsonObject{put("request",original);put("phase","preparing")});preview(original)}}){Text("预览保留计划")};TextButton(enabled=ready&&!disabled&&!busy,onClick={work{scan()}}){Text("查找待清理文件")}}
        issue?.let{Text(it,color=CodegTheme.colors.danger)};message?.let{Text(it,color=CodegTheme.colors.textSecondary)}
        saved?.let{current->val original=current["request"].coreObject();val plan=current["result"].coreObject()["plan"].coreObject();Text("${plan["delete"].coreArray().size}份待移除；之后还有${plan.coreLong("remaining")}份可重新预览",style=MaterialTheme.typography.bodySmall);plan["delete"].coreArray().forEach{Text((if(it.coreText("kind")=="configuration")"配置快照 · " else "完整备份 · ")+it.coreText("createdAt"),style=MaterialTheme.typography.bodySmall)}
            Row{TextButton(enabled=!disabled&&!busy,onClick={work{val status=request(JsonObject(original.toMutableMap().apply{put("operation",JsonPrimitive("retention_status"));remove("keepBackups");remove("keepConfigurations")}));if(status["removed"]==JsonPrimitive(true)){store(null);message="本批移除已完成";onChanged()}else message="原批次尚未完成；保留原请求"}}){Text("核对原批次")}
                if(current.coreText("phase")=="preparing")TextButton(enabled=!disabled&&!busy,onClick={work{preview(original)}}){Text("重读原预览")}
                else TextButton(enabled=!disabled&&!busy&&plan["delete"].coreArray().isNotEmpty(),onClick={confirm=JsonObject(original.toMutableMap().apply{put("operation",JsonPrimitive("retention_apply"));put("confirmed",JsonPrimitive(true));remove("keepBackups");remove("keepConfigurations")})}){Text("确认移除本批")}
            }
            if(current.coreText("phase")=="previewed")TextButton(enabled=!disabled&&!busy,onClick={work{store(null)}}){Text("放弃此预览")}
        }
        cleanup.forEach{entry->val snapshot=entry["snapshot"].coreObject();Row{Text(snapshot.coreText("createdAt"),Modifier.weight(1f),style=MaterialTheme.typography.bodySmall);TextButton(enabled=!disabled&&!busy,onClick={confirm=input("cleanup_removed",buildJsonObject{put("snapshotId",snapshot.coreText("id"));put("expectedDigest",snapshot.coreText("sha256"));put("confirmed",true)})}){Text("清理残留文件")}}}
        cursor?.let{after->TextButton(enabled=!disabled&&!busy,onClick={work{scan(after)}}){Text("继续查找更早记录")}}
    }
    confirm?.let{frozen->AlertDialog(onDismissRequest={if(!busy)confirm=null},title={Text(if(frozen.coreText("operation")=="retention_apply")"移除预览中的这一批？" else "清理已移除副本的文件？")},text={Text("只处理已预览的明确对象；保留本地数据、其他副本和远端墓碑。中断后先核对原结果。")},confirmButton={TextButton(enabled=!busy&&!disabled,onClick={work{if(frozen.coreText("operation")=="retention_apply"){store(JsonObject(saved!!.toMutableMap().apply{put("phase",JsonPrimitive("applying"))}));request(frozen);store(null);onChanged()}else{request(frozen);scan()};confirm=null}}){Text("确认")}},dismissButton={TextButton(enabled=!busy,onClick={confirm=null}){Text("取消")}})}
}
