package app.codeg.android.feature.main

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.component.CodegTextField
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*

/** 原设置页的级别选择和目标覆盖，保存到所连接主机，不改变手机日志。 */
@Composable
fun RovaiHostLoggingSettings(model:RovaiWorkspaceViewModel,onClose:()->Unit){
    val scope=rememberCoroutineScope();val generation=model.connectionGeneration
    var identity by remember{mutableStateOf("")};var saved by remember{mutableStateOf<JsonObject?>(null)}
    var draft by remember{mutableStateOf(buildJsonObject{put("level","info");put("targets",JsonArray(emptyList()))})}
    var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)};var leaving by remember{mutableStateOf(false)}
    val dirty=saved!=null&&draft!=saved!!["settings"]
    val writable=saved?.get("writable")!=JsonPrimitive(false)
    suspend fun current(){if(generation!=model.connectionGeneration||identity.isEmpty()||model.presentationIdentity()!=identity)throw CancellationException("工作区已切换")}
    suspend fun read(){val value=model.privateConfiguration("logging",buildJsonObject{put("operation","get")});current();saved=value;draft=value["settings"].coreObject()}
    fun work(action:suspend()->Unit){if(busy)return;busy=true;issue=null;scope.launch{try{current();action()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message?:"日志设置未能保存，请重新读取原设置"}finally{busy=false}}}
    fun setField(key:String,value:JsonElement){draft=JsonObject(draft.toMutableMap().apply{put(key,value)})}
    fun setTarget(index:Int,key:String,value:String){val rows=draft["targets"].coreArray().toMutableList();rows[index]=JsonObject(rows[index].coreObject().toMutableMap().apply{put(key,JsonPrimitive(value))});setField("targets",JsonArray(rows))}
    LaunchedEffect(generation){busy=true;try{identity=model.presentationIdentity();read()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message?:"无法读取主机日志设置"}finally{busy=false}}
    BackHandler{if(!busy){if(dirty)leaving=true else onClose()}}
    LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(12.dp),verticalArrangement=Arrangement.spacedBy(10.dp)){
        item{TextButton(enabled=!busy,onClick={if(dirty)leaving=true else onClose()}){Text("返回")};Text("主机日志输出",style=MaterialTheme.typography.titleLarge)}
        if(!writable)item{Text("这台主机的日志设置由桌面程序管理，请在主机桌面设置中修改。")}
        else{
            item{Text("调整独立主机结构化诊断日志的详细程度。原始故障记录、运行状态、审批和操作回执照常保留。")}
            item{GlassRow{Text("默认级别");HostLogLevelPicker(draft.coreText("level"),!busy&&saved!=null){setField("level",JsonPrimitive(it))}}}
            val targets=draft["targets"].coreArray()
            items(targets.size){index->val row=targets[index].coreObject();GlassRow{
                CodegTextField(value=row.coreText("target"),onValueChange={if(!busy)setTarget(index,"target",it)},label="日志目标")
                HostLogLevelPicker(row.coreText("level"),!busy){setTarget(index,"level",it)}
                TextButton(enabled=!busy,onClick={setField("targets",JsonArray(targets.filterIndexed{at,_->at!=index}))}){Text("移除目标")}
            }}
            item{TextButton(enabled=!busy&&saved!=null&&targets.size<1024,onClick={setField("targets",JsonArray(targets+buildJsonObject{put("target","");put("level","info")}))}){Text("添加日志目标")}}
            item{Row{
                TextButton(enabled=!busy&&saved!=null&&dirty,onClick={work{
                    val names=draft["targets"].coreArray().map{it.coreObject().coreText("target")};require(names.all{Regex("^[a-zA-Z0-9_.:-]{1,256}$").matches(it)}&&names.distinct().size==names.size){"请填写有效且不重复的日志目标名称"}
                    val value=model.privateConfiguration("logging",buildJsonObject{put("operation","set");put("expectedDigest",saved!!["digest"]!!);put("settings",draft)});current();saved=value;draft=value["settings"].coreObject()
                }}){Text("保存日志设置")}
                TextButton(enabled=!busy&&dirty,onClick={draft=saved!!["settings"].coreObject()}){Text("放弃修改")}
            }}
        }
        item{TextButton(enabled=!busy&&!dirty,onClick={work{read()}}){Text("重新读取已保存设置")}}
        if(issue!=null)item{Text(issue!!,color=MaterialTheme.colorScheme.error)}
        if(busy)item{LinearProgressIndicator(Modifier.fillMaxWidth())}
    }
    if(leaving)AlertDialog(onDismissRequest={leaving=false},title={Text("放弃未保存的日志修改？")},confirmButton={TextButton(onClick={leaving=false;onClose()}){Text("放弃并返回")}},dismissButton={TextButton(onClick={leaving=false}){Text("继续编辑")}})
}
@Composable
private fun HostLogLevelPicker(value:String,enabled:Boolean,onChange:(String)->Unit){
    var expanded by remember{mutableStateOf(false)}
    val levels=listOf("off" to "关闭","error" to "错误","warn" to "警告","info" to "信息","debug" to "调试","trace" to "追踪")
    Box{TextButton(enabled=enabled,onClick={expanded=true}){Text(levels.find{it.first==value}?.second?:value)};DropdownMenu(expanded=expanded,onDismissRequest={expanded=false}){levels.forEach{(id,label)->DropdownMenuItem(text={Text(label)},onClick={expanded=false;onChange(id)})}}}
}
