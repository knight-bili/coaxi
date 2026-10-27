package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import app.coaxi.android.core.designsystem.component.GlassRow
import app.coaxi.android.core.designsystem.markdown.MarkdownContent
import app.coaxi.android.core.designsystem.theme.CoaxiTheme
import app.coaxi.android.core.network.CoaxiOperation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*

/** 沿用原版原生 Skills 的选择、列表、文件导航和正文阅读，不安装或执行技能。 */
@Composable fun CoaxiNativeSkills(model:CoaxiWorkspaceViewModel,onClose:()->Unit){
    val state=model.state.collectAsState().value;val coroutine=rememberCoroutineScope();val generation=remember{model.connectionGeneration}
    var runtimes by remember{mutableStateOf(listOf("codex-cli"))};var target by remember{mutableStateOf("codex-cli")};var picker by remember{mutableStateOf(false)}
    var scan by remember{mutableStateOf(buildJsonObject{})};var current by remember{mutableStateOf<JsonObject?>(null)};var content by remember{mutableStateOf(buildJsonObject{})};var path by remember{mutableStateOf("SKILL.md")};var query by remember{mutableStateOf("")};var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)};var raw by remember{mutableStateOf(false)}
    val custom=state.members.filter{it.coreText("presence")!="removed"&&it["runtimeConfiguration"].coreObject().coreText("adapterKind")=="custom-acp"}
    val label=if(target.startsWith("member:"))custom.find{it.coreText("agentId")==target.removePrefix("member:")}?.coreText("displayName")?:"自定义队员" else target
    val disabled=busy||generation!=model.connectionGeneration
    suspend fun call(operation:CoaxiOperation,input:JsonObject=buildJsonObject{}):JsonElement{require(generation==model.connectionGeneration);val value=model.query(operation,input);require(generation==model.connectionGeneration){"工作区已切换"};return value}
    suspend fun load(refresh:Boolean){scan=call(CoaxiOperation.NATIVESKILLS_LIST,buildJsonObject{put("adapterKind",if(target.startsWith("member:"))"custom-acp" else target);put("refresh",refresh);if(target.startsWith("member:"))put("agentId",target.removePrefix("member:"))}).coreObject()}
    fun work(block:suspend()->Unit){if(disabled)return;busy=true;coroutine.launch{try{block();issue=null}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(generation==model.connectionGeneration)issue=error.message}finally{busy=false}}}
    LaunchedEffect(Unit){busy=true;try{val values=call(CoaxiOperation.RUNTIME_INSTALLATIONS_LIST).coreArray().map{it.coreText("adapterKind")}.filter{it.isNotEmpty()&&it!="custom-acp"}.distinct();if(values.isNotEmpty())runtimes=values;load(false)}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    LaunchedEffect(current?.coreText("id"),path){val selected=current?:return@LaunchedEffect;busy=true;content=buildJsonObject{};try{content=call(CoaxiOperation.NATIVESKILLS_READ,buildJsonObject{put("skillId",selected.coreText("id"));put("path",path)}).coreObject();issue=null}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    BackHandler{if(current!=null){current=null;content=buildJsonObject{}}else onClose()}
    Dialog(onDismissRequest=onClose,properties=DialogProperties(usePlatformDefaultWidth=false)){Surface(Modifier.fillMaxSize(),color=CoaxiTheme.colors.bg){LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
        item{Row{TextButton(onClick={if(current!=null){current=null;content=buildJsonObject{}}else onClose()}){Text(stringResource(R.string.coaxi_fusion_572cf45ba43634b3))};if(current==null)TextButton(enabled=!disabled,onClick={picker=true}){Text(label)};TextButton(enabled=!disabled,onClick={work{if(current==null)load(true)else content=call(CoaxiOperation.NATIVESKILLS_READ,buildJsonObject{put("skillId",current!!.coreText("id"));put("path",path)}).coreObject()}}){Text(stringResource(R.string.coaxi_fusion_aee88743413144a2))}};Text(current?.coreText("name")?:"原生 Skills",style=MaterialTheme.typography.titleLarge)}
        issue?.let{item{Text(it,color=CoaxiTheme.colors.danger)}}
        if(current==null){
            item{OutlinedTextField(query,{query=it},label={Text(stringResource(R.string.coaxi_fusion_adb3dcb1d5e3221e))},modifier=Modifier.fillMaxWidth());Text(stringResource(R.string.coaxi_fusion_8490e3ebbb62faf0),style=MaterialTheme.typography.bodySmall,color=CoaxiTheme.colors.textSecondary)}
            items((scan["errors"] as? JsonArray).orEmpty()){item->Text((item as? JsonPrimitive)?.content.orEmpty(),color=CoaxiTheme.colors.danger)}
            items(scan["skills"].coreArray().filter{(it.coreText("name")+" "+it.coreText("description")).contains(query,true)},key={it.coreText("id")}){skill->GlassRow(Modifier.fillMaxWidth()){Text(skill.coreText("name"),style=MaterialTheme.typography.titleMedium);Text(skill.coreText("description"),color=CoaxiTheme.colors.textSecondary);TextButton(enabled=!disabled,onClick={path="SKILL.md";raw=false;current=skill}){Text(stringResource(R.string.coaxi_fusion_393dd8a7f29e2844))}}}
            if(!busy&&scan["skills"].coreArray().isEmpty())item{Text(stringResource(R.string.coaxi_fusion_6babebf5562e0b3e))}
        }else{
            item{Text(current!!.coreText("entryPath"),style=MaterialTheme.typography.bodySmall);Row{Text(stringResource(R.string.coaxi_fusion_354b28c853334568),Modifier.weight(1f));Switch(raw,{raw=it})}}
            items(content["files"].coreArray(),key={it.coreText("path")}){file->TextButton(enabled=!disabled,onClick={path=file.coreText("path")}){Text(file.coreText("path"))}}
            item{if(content.coreText("status")=="text"){if(raw)Text(content.coreText("content"))else MarkdownContent(raw=content.coreText("content"))}else if(!busy)Text(if(content.coreText("status")=="binary")"该文件不是文本文件。" else "正文暂不可读或超过预览上限。")}
        }
    }}}
    if(picker)AlertDialog(onDismissRequest={picker=false},title={Text(stringResource(R.string.coaxi_fusion_b81826c3ba1f03b2))},text={LazyColumn{items(runtimes){kind->TextButton(onClick={target=kind;picker=false;work{load(false)}}){Text(kind)}};items(custom,key={it.coreText("agentId")}){member->TextButton(onClick={target="member:"+member.coreText("agentId");picker=false;work{load(false)}}){Text(member.coreText("displayName")+" · 自定义智能体")}}}},confirmButton={TextButton(onClick={picker=false}){Text(stringResource(R.string.coaxi_fusion_3fd47edce45b3603))}})
}
