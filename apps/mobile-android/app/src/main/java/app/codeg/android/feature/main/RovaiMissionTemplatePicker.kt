package app.codeg.android.feature.main

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*

/** 在原使命表单内选择 Codeg 模板；填入只改本次草稿，不启动或修改队员配置。 */
@Composable
fun RovaiMissionTemplatePicker(model:RovaiWorkspaceViewModel,disabled:Boolean,memberCount:Int,materials:List<Pair<String,String>>,onChooseMaterials:()->Unit,onApply:(String,String)->Unit){
    val scope=rememberCoroutineScope();val generation=model.connectionGeneration
    var open by remember{mutableStateOf(false)};var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)}
    var rows by remember{mutableStateOf<List<JsonObject>>(emptyList())};var next by remember{mutableStateOf<String?>(null)}
    var selected by remember{mutableStateOf<JsonObject?>(null)};var configuration by remember{mutableStateOf<JsonObject?>(null)}
    var mappings by remember{mutableStateOf<Map<Int,String>>(emptyMap())};var confirmed by remember{mutableStateOf(false)}
    LaunchedEffect(generation){open=false;selected=null;configuration=null;rows=emptyList();mappings=emptyMap();confirmed=false;busy=false;issue=null}
    suspend fun current(){if(model.connectionGeneration!=generation)throw CancellationException("工作区已切换")}
    fun work(action:suspend()->Unit){if(busy)return;busy=true;issue=null;scope.launch{try{current();action()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(model.connectionGeneration==generation)issue=error.message?:"无法读取任务模板"}finally{if(model.connectionGeneration==generation)busy=false}}}
    suspend fun list(more:Boolean=false){val value=model.privateConfiguration("templates",buildJsonObject{put("operation","list");put("limit",100);if(more)next?.let{put("after",it)}});current();val loaded=value["items"].coreArray().map{it.coreObject()};rows=if(more)(rows+loaded).distinctBy{it.coreText("id")}else loaded;next=value["next"]?.jsonPrimitive?.contentOrNull}
    val blocks=configuration?.get("prompt_blocks").coreArray()
    val required=blocks.withIndex().filter{it.value.coreObject().coreText("type")!="text"}
    val settings=configuration?.let{it["agent_type"]!=null&&it["agent_type"]!=JsonNull||it["mode_id"]!=null&&it["mode_id"]!=JsonNull||it["base_branch"]!=null&&it["base_branch"]!=JsonNull||it["config_values"].coreObject().isNotEmpty()}?:false
    val canApply=selected!=null&&configuration!=null&&!disabled&&!busy&&required.all{row->materials.any{it.first==mappings[row.index]}}&&(!settings||(confirmed&&memberCount>0))
    Column{
        TextButton(enabled=!disabled&&!busy,onClick={open=!open;if(open)work{selected=null;configuration=null;list()}}){Text(if(open)"收起任务模板" else "使用任务模板")}
        if(open){
            if(selected==null){
                rows.forEach{row->TextButton(enabled=!disabled&&!busy,onClick={work{
                    val value=model.privateConfiguration("templates",buildJsonObject{put("operation","get");put("id",row["id"]!!)});current();require(value.isNotEmpty()){ "这份模板已被删除" }
                    val config=Json.parseToJsonElement(value.coreText("config")).coreObject();require(config.isNotEmpty()&&config["prompt_blocks"].let{it==null||it is JsonArray}){"原模板需要先补齐配置，完整内容仍保留"}
                    require(config["prompt_blocks"].coreArray().all{it is JsonObject}&&config["prompt_blocks"].coreArray().size<=8192&&config["prompt_blocks"].coreArray().count{it.coreObject().coreText("type")!="text"}<=128){"原模板材料无法读取或数量超过限制"}
                    selected=value;configuration=config;mappings=emptyMap();confirmed=false
                }}){Text(row.coreText("name"))}}
                if(next!=null)TextButton(enabled=!disabled&&!busy,onClick={work{list(true)}}){Text("更多模板")}
                if(!busy&&rows.isEmpty())Text("还没有模板，可在主机配置迁移与模板设置中创建或搬入。")
            }else{
                Text(selected!!.coreText("title").ifBlank{selected!!.coreText("name")},style=MaterialTheme.typography.titleSmall)
                if(settings){Row{Checkbox(confirmed,{confirmed=it},enabled=!disabled&&!busy);Text("改用当前工程、分支和已选择队员的运行设置，原模板继续保留。",Modifier.weight(1f))};if(memberCount==0)Text("请先选择使命执行队员。")}
                if(required.isNotEmpty()){
                    Text("旧材料路径不会直接读取。先添加本次材料，再逐项对应。")
                    TextButton(enabled=!disabled&&!busy,onClick=onChooseMaterials){Text("选择本次材料")}
                    required.forEach{row->var expanded by remember(row.index){mutableStateOf(false)};val name=row.value.coreObject().coreText("name").ifEmpty{"原材料 ${row.index+1}"};Box{
                        TextButton(enabled=!disabled&&!busy,onClick={expanded=true}){Text(name+" · "+(materials.find{it.first==mappings[row.index]}?.second?:"选择已加入的材料"))}
                        DropdownMenu(expanded,onDismissRequest={expanded=false}){materials.forEach{(id,label)->DropdownMenuItem(text={Text(label)},onClick={expanded=false;mappings=mappings+(row.index to id)})}}
                    }}
                }
                Text("填入会替换当前标题和说明；确认创建使命前不会启动。",style=MaterialTheme.typography.bodySmall)
                Row{TextButton(enabled=canApply,onClick={val config=configuration!!;val body=config["display_text"]?.jsonPrimitive?.contentOrNull?:blocks.filter{it.coreObject().coreText("type")=="text"}.joinToString("\n"){it.coreObject().coreText("text")};onApply(selected!!.coreText("title"),body);open=false;selected=null;configuration=null}){Text("填入标题和说明")};TextButton(enabled=!busy,onClick={selected=null;configuration=null}){Text("返回列表")}}
            }
            if(issue!=null)Text(issue!!,color=MaterialTheme.colorScheme.error)
            if(busy)LinearProgressIndicator(Modifier.fillMaxWidth())
        }
    }
}
