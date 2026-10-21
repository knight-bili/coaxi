package app.codeg.android.feature.main

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.markdown.MarkdownContent
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*

// Original Codeg native list/detail layout; installation and member choices belong to Core.
@Composable
fun RovaiSkillSettings(model:RovaiWorkspaceViewModel){
    val state by model.state.collectAsStateWithLifecycle();val scope=rememberCoroutineScope()
    var catalog by remember{mutableStateOf(buildJsonObject{})};var installations by remember{mutableStateOf<List<JsonObject>>(emptyList())}
    var assignments by remember{mutableStateOf<List<JsonObject>>(emptyList())};var selected by remember{mutableStateOf<String?>(null)}
    var query by remember{mutableStateOf("")};var family by remember{mutableStateOf("all")};var content by remember{mutableStateOf(buildJsonObject{})}
    var path by remember(selected){mutableStateOf("SKILL.md")};var issue by remember{mutableStateOf<String?>(null)};var loading by remember{mutableStateOf(false)}
    var remove by remember{mutableStateOf<JsonObject?>(null)}
    suspend fun load(){loading=true;try{catalog=model.query(RovaiOperation.SKILLPACKS_LIST).coreObject();installations=model.query(RovaiOperation.SKILLPACKS_INSTALLATIONS_LIST).coreArray();assignments=model.query(RovaiOperation.SKILLPACKS_MEMBERS_LIST).coreArray();issue=null}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{loading=false}}
    LaunchedEffect(Unit){load()};LaunchedEffect(state.lastResult){if(state.lastCompletedOperation?.startsWith("skillPacks.")==true)load()}
    LaunchedEffect(selected,path){content=buildJsonObject{};val id=selected?:return@LaunchedEffect;try{content=model.query(RovaiOperation.SKILLPACKS_READ,buildJsonObject{put("skillId",id);put("path",path)}).coreObject()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}}
    BackHandler(enabled=selected!=null){selected=null}
    val entry=catalog["entries"].coreArray().find{it.coreText("id")==selected};val installed=installations.find{it.coreText("skillId")==selected};val members=assignments.find{it.coreText("skillId")==selected}
    val disabled=state.pending!=null||state.busy||loading
    fun localized(value:JsonElement?):String{val objectValue=value.coreObject();return objectValue.coreText("zh-CN").ifEmpty{objectValue.coreText("zh").ifEmpty{objectValue.coreText("en")}}}
    LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
        item{Row(Modifier.fillMaxWidth()){if(selected!=null)TextButton(onClick={selected=null}){Text("返回技能库")};TextButton(onClick={scope.launch{load()}},enabled=!disabled){Text("刷新")}}}
        issue?.let{item{Text(it,color=CodegTheme.colors.danger)}}
        if(entry==null){
            item{OutlinedTextField(value=query,onValueChange={query=it},label={Text("搜索技能")},modifier=Modifier.fillMaxWidth())}
            item{Row(horizontalArrangement=Arrangement.spacedBy(4.dp)){listOf("all" to "全部","experts" to "专家","science" to "科研").forEach{(id,title)->FilterChip(selected=family==id,onClick={family=id},label={Text(title)})}}}
            item{Row(horizontalArrangement=Arrangement.spacedBy(4.dp)){listOf("office" to "Office","ui" to "交互卡片","custom" to "自定义").forEach{(id,title)->FilterChip(selected=family==id,onClick={family=id},label={Text(title)})}}}
            items(catalog["entries"].coreArray().filter{(family=="all"||it.coreText("family")==family)&&("${it.coreText("id")} ${localized(it["displayName"])} ${localized(it["description"])}").contains(query,true)},key={it.coreText("id")}){skill->
                val status=installations.find{it.coreText("skillId")==skill.coreText("id")}
                GlassRow(Modifier.fillMaxWidth().clickable{selected=skill.coreText("id")}){Text(localized(skill["displayName"]).ifEmpty{skill.coreText("id")},style=MaterialTheme.typography.titleMedium);Text(localized(skill["description"]),color=CodegTheme.colors.textSecondary,maxLines=3);Text(if(status?.coreText("status")=="installed")"已安装 · ${status.coreText("availability")}" else "未安装",style=MaterialTheme.typography.labelSmall)}
            }
        }else{
            item{GlassRow(Modifier.fillMaxWidth()){
                Text(localized(entry["displayName"]).ifEmpty{entry.coreText("id")},style=MaterialTheme.typography.titleLarge);Text(localized(entry["description"]),color=CodegTheme.colors.textSecondary)
                Text("来源：${entry.coreText("sourceUrl")}\n版本：${catalog.coreText("commit").take(12)}",style=MaterialTheme.typography.bodySmall)
                if((entry["needsKey"] as? JsonPrimitive)?.booleanOrNull==true)Text("需要外部服务凭据。")
                if((entry["needsEnv"] as? JsonPrimitive)?.booleanOrNull==true)Text("可能需要额外环境；安装技能不会自动安装依赖。")
                Row(horizontalArrangement=Arrangement.spacedBy(10.dp)){
                    if(entry.coreText("contentSource") in listOf("embedded","custom"))Button(enabled=!disabled,onClick={val custom=entry.coreText("contentSource")=="custom";model.command(if(custom)RovaiOperation.SKILLPACKS_CUSTOM_INSTALL else RovaiOperation.SKILLPACKS_INSTALL,buildJsonObject{put("skillId",entry.coreText("id"));put("expectedVersion",installed?.coreText("version")?:"absent");put("sourceRevision",if(custom)entry.coreText("loadId") else catalog.coreText("commit"))})}){Text(if(installed?.coreText("status")=="installed")"更新安装" else "安装")}
                    if(installed?.coreText("status")=="installed")TextButton(enabled=!disabled,onClick={remove=installed}){Text("卸载")}
                }
            }}
            if((members?.get("installed") as? JsonPrimitive)?.booleanOrNull==true){
                item{Text("谁可以使用",style=MaterialTheme.typography.titleMedium);Text("用于队员的新任务；取消选择不改写旧任务。",color=CodegTheme.colors.textSecondary)}
                items(state.members.filter{it.coreText("presence")!="removed"},key={it.coreText("agentId")}){member->val picked=(members?.get("memberIds") as? JsonArray)?.mapNotNull{(it as? JsonPrimitive)?.contentOrNull}?.toSet()?:emptySet();val id=member.coreText("agentId")
                    GlassRow(Modifier.fillMaxWidth()){Row{Text(member.coreText("displayName"),Modifier.weight(1f));Checkbox(checked=id in picked,enabled=!disabled,onCheckedChange={checked->model.command(RovaiOperation.SKILLPACKS_MEMBERS_SET,buildJsonObject{put("skillId",selected);put("expectedVersion",members!!.coreText("version"));put("memberIds",JsonArray((if(checked)picked+id else picked-id).sorted().map(::JsonPrimitive)))})})}}
                }
            }
            item{Text("附带文件",style=MaterialTheme.typography.titleMedium)}
            items(content["files"].coreArray(),key={it.coreText("path")}){file->TextButton(onClick={path=file.coreText("path")}){Text(file.coreText("path"))}}
            item{if(content.coreText("status")=="text")MarkdownContent(content.coreText("content"))else if(content.coreText("status").isNotEmpty())Text(if(content.coreText("status")=="binary")"该文件不是文本文件。" else "文件较大，暂不支持正文预览。")}
        }
    }
    remove?.let{target->AlertDialog(onDismissRequest={if(!disabled)remove=null},title={Text("卸载技能包")},text={Text("保留文件和历史版本；重新安装后不会自动恢复队员选择。")},confirmButton={TextButton(enabled=!disabled,onClick={model.command(RovaiOperation.SKILLPACKS_UNINSTALL,buildJsonObject{put("skillId",target.coreText("skillId"));put("expectedVersion",target.coreText("version"))});remove=null}){Text("确认卸载")}},dismissButton={TextButton(enabled=!disabled,onClick={remove=null}){Text("取消")}})}
}
