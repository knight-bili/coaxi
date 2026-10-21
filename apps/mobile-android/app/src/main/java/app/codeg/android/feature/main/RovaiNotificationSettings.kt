package app.codeg.android.feature.main

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*

private val reminderGroups=listOf("会话" to listOf("approvalHeadsUpEnabled" to "待审批","userMentionHeadsUpEnabled" to "提到你","turnCompletedHeadsUpEnabled" to "本轮完成","singleChatHeadsUpEnabled" to "单聊回复","turnIncompleteHeadsUpEnabled" to "执行未完成"),"使命" to listOf("missionNeedsYouHeadsUpEnabled" to "使命需要你","missionStatusHeadsUpEnabled" to "使命状态变更"),"任务" to listOf("taskStatusHeadsUpEnabled" to "任务状态变更"))
private val reminderStatuses=mapOf("missionStatuses" to listOf("completed" to "已完成","in_progress" to "进行中","not_started" to "未开始"),"taskStatuses" to listOf("completed" to "已完成","blocked" to "受阻","cancelled" to "已取消","in_progress" to "进行中","pending" to "待开始"))
@Composable
fun RovaiNotificationSettings(model:RovaiWorkspaceViewModel){
    val state by model.state.collectAsStateWithLifecycle();val scope=rememberCoroutineScope();var preference by remember{mutableStateOf<JsonObject?>(null)};var issue by remember{mutableStateOf<String?>(null)};var loading by remember{mutableStateOf(false)}
    suspend fun load(){loading=true;try{val value=model.query(RovaiOperation.NOTIFICATIONS_PREFERENCE_GET).coreObject();require(value["version"] is JsonPrimitive&&value["headsUpEnabled"] is JsonPrimitive){"提醒设置格式不兼容"};preference=value;issue=null}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{loading=false}}
    LaunchedEffect(Unit){load()};LaunchedEffect(state.lastResult){if(state.lastCompletedOperation==RovaiOperation.NOTIFICATIONS_PREFERENCE_UPDATE.wire)load()}
    val disabled=loading||state.busy||state.pending!=null||preference==null;val enabled=preference?.get("headsUpEnabled")==JsonPrimitive(true)
    fun update(key:String,value:JsonElement){val current=preference?:return;model.command(RovaiOperation.NOTIFICATIONS_PREFERENCE_UPDATE,buildJsonObject{put("expectedVersion",current["version"]!!);(listOf("headsUpEnabled")+reminderGroups.flatMap{it.second.map{it.first}}+listOf("missionStatuses","taskStatuses")).forEach{name->put(name,if(name==key)value else current[name]?:JsonNull)}})}
    LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
        item{Row{Text("提醒",Modifier.weight(1f),style=MaterialTheme.typography.titleLarge);TextButton(enabled=!disabled,onClick={scope.launch{load()}}){Text("刷新")}}}
        issue?.let{item{Text(it,color=CodegTheme.colors.danger)}}
        item{GlassRow(Modifier.fillMaxWidth()){Row{Text("应用内提醒",Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);Switch(checked=enabled,onCheckedChange={update("headsUpEnabled",JsonPrimitive(it))},enabled=!disabled)}}}
        reminderGroups.forEach{(title,categories)->item{GlassRow(Modifier.fillMaxWidth()){Text(title,style=MaterialTheme.typography.titleMedium);categories.forEach{(key,label)->Row{Text(label,Modifier.weight(1f));Switch(checked=preference?.get(key)==JsonPrimitive(true),onCheckedChange={update(key,JsonPrimitive(it))},enabled=!disabled&&enabled)}};val filter=when(title){"使命"->"missionStatuses";"任务"->"taskStatuses";else->null};if(filter!=null){val selected=(preference?.get(filter) as? JsonArray)?.mapNotNull{(it as? JsonPrimitive)?.contentOrNull}?.toSet()?:emptySet();reminderStatuses[filter].orEmpty().forEach{(id,label)->Row{Checkbox(checked=id in selected,onCheckedChange={checked->update(filter,JsonArray(reminderStatuses[filter].orEmpty().filter{if(it.first==id)checked else it.first in selected}.map{JsonPrimitive(it.first)}))},enabled=!disabled&&enabled&&preference?.get(if(filter=="missionStatuses")"missionStatusHeadsUpEnabled" else "taskStatusHeadsUpEnabled")==JsonPrimitive(true));Text(label)}}}}}}
    }
}
