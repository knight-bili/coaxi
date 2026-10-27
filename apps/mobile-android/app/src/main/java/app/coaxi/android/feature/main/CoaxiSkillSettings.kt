package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

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
import app.coaxi.android.core.designsystem.component.GlassRow
import app.coaxi.android.core.designsystem.markdown.MarkdownContent
import app.coaxi.android.core.designsystem.theme.CoaxiTheme
import app.coaxi.android.core.network.CoaxiOperation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*

// Original Coaxi native list/detail layout; installation and member choices belong to Core.
@Composable
fun CoaxiSkillSettings(model:CoaxiWorkspaceViewModel){
    val state by model.state.collectAsStateWithLifecycle();val scope=rememberCoroutineScope()
    var native by remember{mutableStateOf(false)}
    if(native)CoaxiNativeSkills(model,onClose={native=false})
    var catalog by remember{mutableStateOf(buildJsonObject{})};var installations by remember{mutableStateOf<List<JsonObject>>(emptyList())}
    var assignments by remember{mutableStateOf<List<JsonObject>>(emptyList())};var selected by remember{mutableStateOf<String?>(null)}
    var query by remember{mutableStateOf("")};var family by remember{mutableStateOf("all")};var content by remember{mutableStateOf(buildJsonObject{})}
    var path by remember(selected){mutableStateOf("SKILL.md")};var issue by remember{mutableStateOf<String?>(null)};var loading by remember{mutableStateOf(false)}
    var remove by remember{mutableStateOf<JsonObject?>(null)}
    suspend fun load(){loading=true;try{catalog=model.query(CoaxiOperation.SKILLPACKS_LIST).coreObject();installations=model.query(CoaxiOperation.SKILLPACKS_INSTALLATIONS_LIST).coreArray();assignments=model.query(CoaxiOperation.SKILLPACKS_MEMBERS_LIST).coreArray();issue=null}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{loading=false}}
    LaunchedEffect(Unit){load()};LaunchedEffect(state.lastResult){if(state.lastCompletedOperation?.startsWith("skillPacks.")==true)load()}
    LaunchedEffect(selected,path){content=buildJsonObject{};val id=selected?:return@LaunchedEffect;try{content=model.query(CoaxiOperation.SKILLPACKS_READ,buildJsonObject{put("skillId",id);put("path",path)}).coreObject()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}}
    BackHandler(enabled=selected!=null){selected=null}
    val entry=catalog["entries"].coreArray().find{it.coreText("id")==selected};val installed=installations.find{it.coreText("skillId")==selected};val members=assignments.find{it.coreText("skillId")==selected}
    val disabled=state.pending!=null||state.busy||loading
    fun localized(value:JsonElement?):String{val objectValue=value.coreObject();return objectValue.coreText("zh-CN").ifEmpty{objectValue.coreText("zh").ifEmpty{objectValue.coreText("en")}}}
    LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
        item{TextButton(onClick={native=true}){Text(stringResource(R.string.coaxi_fusion_a7e6636d42fc542c))}}
        item{Row(Modifier.fillMaxWidth()){if(selected!=null)TextButton(onClick={selected=null}){Text(stringResource(R.string.coaxi_fusion_8bf997158c7a5b04))};TextButton(onClick={scope.launch{load()}},enabled=!disabled){Text(stringResource(R.string.coaxi_fusion_aee88743413144a2))}}}
        issue?.let{item{Text(it,color=CoaxiTheme.colors.danger)}}
        if(entry==null){
            item{OutlinedTextField(value=query,onValueChange={query=it},label={Text(stringResource(R.string.coaxi_fusion_43b9d66b88a87f61))},modifier=Modifier.fillMaxWidth())}
            item{Row(horizontalArrangement=Arrangement.spacedBy(4.dp)){listOf("all" to "全部","experts" to "专家","science" to "科研").forEach{(id,title)->FilterChip(selected=family==id,onClick={family=id},label={Text(title)})}}}
            item{Row(horizontalArrangement=Arrangement.spacedBy(4.dp)){listOf("office" to "Office","ui" to "交互卡片","custom" to "自定义").forEach{(id,title)->FilterChip(selected=family==id,onClick={family=id},label={Text(title)})}}}
            items(catalog["entries"].coreArray().filter{(family=="all"||it.coreText("family")==family)&&("${it.coreText("id")} ${localized(it["displayName"])} ${localized(it["description"])}").contains(query,true)},key={it.coreText("id")}){skill->
                val status=installations.find{it.coreText("skillId")==skill.coreText("id")}
                GlassRow(Modifier.fillMaxWidth().clickable{selected=skill.coreText("id")}){Text(localized(skill["displayName"]).ifEmpty{skill.coreText("id")},style=MaterialTheme.typography.titleMedium);Text(localized(skill["description"]),color=CoaxiTheme.colors.textSecondary,maxLines=3);Text(if(status?.coreText("status")=="installed")"已安装 · ${status.coreText("availability")}" else "未安装",style=MaterialTheme.typography.labelSmall)}
            }
        }else{
            item{GlassRow(Modifier.fillMaxWidth()){
                Text(localized(entry["displayName"]).ifEmpty{entry.coreText("id")},style=MaterialTheme.typography.titleLarge);Text(localized(entry["description"]),color=CoaxiTheme.colors.textSecondary)
                Text("来源：${entry.coreText("sourceUrl")}\n版本：${catalog.coreText("commit").take(12)}",style=MaterialTheme.typography.bodySmall)
                if((entry["needsKey"] as? JsonPrimitive)?.booleanOrNull==true)Text(stringResource(R.string.coaxi_fusion_b5f91f44318cc189))
                if((entry["needsEnv"] as? JsonPrimitive)?.booleanOrNull==true)Text(stringResource(R.string.coaxi_fusion_b50f3ec00854d77d))
                Row(horizontalArrangement=Arrangement.spacedBy(10.dp)){
                    if(entry.coreText("contentSource") in listOf("embedded","custom"))Button(enabled=!disabled,onClick={val custom=entry.coreText("contentSource")=="custom";model.command(if(custom)CoaxiOperation.SKILLPACKS_CUSTOM_INSTALL else CoaxiOperation.SKILLPACKS_INSTALL,buildJsonObject{put("skillId",entry.coreText("id"));put("expectedVersion",installed?.coreText("version")?:"absent");put("sourceRevision",if(custom)entry.coreText("loadId") else catalog.coreText("commit"))})}){Text(if(installed?.coreText("status")=="installed")"更新安装" else "安装")}
                    if(installed?.coreText("status")=="installed")TextButton(enabled=!disabled,onClick={remove=installed}){Text(stringResource(R.string.coaxi_fusion_06bc14b60f3598a3))}
                }
            }}
            if((members?.get("installed") as? JsonPrimitive)?.booleanOrNull==true){
                item{Text(stringResource(R.string.coaxi_fusion_bd37181b53c2715d),style=MaterialTheme.typography.titleMedium);Text(stringResource(R.string.coaxi_fusion_3122a69b1fdfd0eb),color=CoaxiTheme.colors.textSecondary)}
                items(state.members.filter{it.coreText("presence")!="removed"},key={it.coreText("agentId")}){member->val picked=(members?.get("memberIds") as? JsonArray)?.mapNotNull{(it as? JsonPrimitive)?.contentOrNull}?.toSet()?:emptySet();val id=member.coreText("agentId")
                    GlassRow(Modifier.fillMaxWidth()){Row{Text(member.coreText("displayName"),Modifier.weight(1f));Checkbox(checked=id in picked,enabled=!disabled,onCheckedChange={checked->model.command(CoaxiOperation.SKILLPACKS_MEMBERS_SET,buildJsonObject{put("skillId",selected);put("expectedVersion",members!!.coreText("version"));put("memberIds",JsonArray((if(checked)picked+id else picked-id).sorted().map(::JsonPrimitive)))})})}}
                }
            }
            item{Text(stringResource(R.string.coaxi_fusion_9d48c251f55a113b),style=MaterialTheme.typography.titleMedium)}
            items(content["files"].coreArray(),key={it.coreText("path")}){file->TextButton(onClick={path=file.coreText("path")}){Text(file.coreText("path"))}}
            item{if(content.coreText("status")=="text")MarkdownContent(content.coreText("content"))else if(content.coreText("status").isNotEmpty())Text(if(content.coreText("status")=="binary")"该文件不是文本文件。" else "文件较大，暂不支持正文预览。")}
        }
    }
    remove?.let{target->AlertDialog(onDismissRequest={if(!disabled)remove=null},title={Text(stringResource(R.string.coaxi_fusion_f34442ed97ee0e38))},text={Text(stringResource(R.string.coaxi_fusion_54caaf8cf78cccee))},confirmButton={TextButton(enabled=!disabled,onClick={model.command(CoaxiOperation.SKILLPACKS_UNINSTALL,buildJsonObject{put("skillId",target.coreText("skillId"));put("expectedVersion",target.coreText("version"))});remove=null}){Text(stringResource(R.string.coaxi_fusion_4fec200ac3f7fc85))}},dismissButton={TextButton(enabled=!disabled,onClick={remove=null}){Text(stringResource(R.string.coaxi_fusion_2cd0f3be8738a86c))}})}
}
