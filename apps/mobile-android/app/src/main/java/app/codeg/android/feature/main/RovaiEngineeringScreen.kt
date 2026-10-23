package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import java.util.UUID

// The original native folder drill-in and Git preview layout, backed by Core
// scope identities rather than the donor's folder/conversation database.
@Composable
fun RovaiEngineeringScreen(model:RovaiWorkspaceViewModel,threadId:String,blocked:Boolean,onClose:()->Unit){
    val ownerState by model.state.collectAsStateWithLifecycle();var submitted by remember{mutableStateOf<Pair<String,String>?>(null)};val coroutine=rememberCoroutineScope();val scope=remember(threadId){buildJsonObject{put("threadId",threadId)}}
    var directory by remember(threadId){mutableStateOf("")};var entries by remember(threadId){mutableStateOf<List<JsonObject>>(emptyList())};var next by remember{mutableStateOf<String?>(null)}
    var office by remember(threadId){mutableStateOf<String?>(null)}
    var file by remember(threadId){mutableStateOf<JsonObject?>(null)};var draft by remember(threadId){mutableStateOf("")};var baseline by remember(threadId){mutableStateOf("")}
    var section by remember{mutableIntStateOf(0)};var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)}
    var gitView by remember{mutableStateOf("status")};var gitOutput by remember{mutableStateOf("")};var action by remember{mutableStateOf("stage")};var paths by remember{mutableStateOf("")};var target by remember{mutableStateOf("")};var message by remember{mutableStateOf("")};var mode by remember{mutableStateOf("mixed")}
    var plan by remember(threadId){mutableStateOf<JsonObject?>(null)};var planArgs by remember{mutableStateOf<JsonObject?>(null)};var discard by remember{mutableStateOf(false)};var exitRequested by remember{mutableStateOf(false)}
    office?.let{selected->RovaiOfficePreview(model, threadId, selected, onClose = { office = null })}
    val dirty=file!=null&&draft!=baseline
    fun work(block:suspend ()->Unit){if(busy)return;busy=true;issue=null;coroutine.launch{try{block()}catch(error:Exception){issue=error.message}finally{busy=false}}}
    suspend fun readDirectory(after:String?=null){val result=model.query(RovaiOperation.WORKBENCH_FILES_LIST,buildJsonObject{put("scope",scope);put("path",directory);if(after!=null)put("after",after)}).coreObject();entries=if(after==null)result["entries"].coreArray() else entries+result["entries"].coreArray();next=(result["next"] as? JsonPrimitive)?.contentOrNull}
    suspend fun readGit(){val result=model.query(RovaiOperation.WORKBENCH_GIT_READ,buildJsonObject{put("scope",scope);put("action",gitView);if(target.isNotBlank())put("target",target);put("paths",JsonArray(paths.lines().filter{it.isNotBlank()}.map(::JsonPrimitive)))}).coreObject();gitOutput=result.coreText("output");if(result["success"]==JsonPrimitive(false))throw Exception(result.coreText("error"))}
    fun close(){if(dirty){exitRequested=true;discard=true}else onClose()}
    BackHandler{if(file!=null){if(dirty){exitRequested=false;discard=true}else file=null}else close()}
    LaunchedEffect(threadId,directory){if(file==null)work{readDirectory()}}
    LaunchedEffect(ownerState.lastResult){val sent=submitted;val result=ownerState.lastResult.coreObject();if(sent!=null&&ownerState.lastCompletedOperation==RovaiOperation.WORKBENCH_FILES_MUTATE.wire&&result["applied"]==JsonPrimitive(true)&&file?.coreText("path")==sent.first){baseline=sent.second;file=buildJsonObject{file!!.forEach{(key,value)->put(key,value)};put("digest",result.coreText("digest"))};submitted=null}}
    Column(Modifier.fillMaxSize()){
        Row(Modifier.fillMaxWidth().padding(8.dp),horizontalArrangement=Arrangement.spacedBy(8.dp)){
            TextButton(onClick=::close){Text(stringResource(R.string.rovai_fusion_39755d00a27cc1d3))}
            FilterChip(selected=section==0,onClick={section=0},label={Text(stringResource(R.string.rovai_fusion_39932f24fe11a6ba))});FilterChip(selected=section==1,onClick={if(!dirty){section=1;work{readGit()}}},label={Text("Git")});FilterChip(selected=section==2,enabled=!dirty,onClick={section=2},label={Text(stringResource(R.string.rovai_fusion_ccc6ad4872c09db7))})
            Spacer(Modifier.weight(1f));IconButton(enabled=!busy&&!dirty,onClick={work{if(section==0)readDirectory()else readGit()}}){Icon(Icons.Outlined.Refresh,"刷新")}
        }
        issue?.let{Text(it,Modifier.padding(12.dp),color=CodegTheme.colors.danger)}
        if(section==2){RovaiForgeWorkspace(model,threadId)}else if(section==0){
            if(file==null){
                Row(Modifier.fillMaxWidth().padding(horizontal=12.dp)){Text(directory.ifEmpty{"工程根目录"},Modifier.weight(1f),fontFamily=FontFamily.Monospace);if(directory.isNotEmpty())TextButton(enabled=!busy,onClick={directory=directory.substringBeforeLast('/',"")}){Text(stringResource(R.string.rovai_fusion_8e7847be62b68c2b))}}
                LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(horizontal=12.dp)){
                    items(entries,key={it.coreText("path")}){entry->ListItem(headlineContent={Text(entry.coreText("name"))},leadingContent={Icon(if(entry["directory"]==JsonPrimitive(true))Icons.Outlined.Folder else Icons.Outlined.Description,null)},trailingContent={TextButton(enabled=!busy&&entry["link"]!=JsonPrimitive(true),onClick={if(entry["directory"]==JsonPrimitive(true))directory=entry.coreText("path")else if(rovaiOfficeFile(entry.coreText("path")))office=entry.coreText("path")else work{val result=model.query(RovaiOperation.WORKBENCH_FILES_READ,buildJsonObject{put("scope",scope);put("path",entry.coreText("path"))}).coreObject();file=buildJsonObject{result.forEach{(key,value)->put(key,value)};put("path",entry.coreText("path"))};draft=result.coreText("text");baseline=draft}}){Text(stringResource(R.string.rovai_fusion_c771248e511fbf93))}},colors=ListItemDefaults.colors(containerColor=androidx.compose.ui.graphics.Color.Transparent))}
                    next?.let{after->item{TextButton(enabled=!busy,onClick={work{readDirectory(after)}}){Text(stringResource(R.string.rovai_fusion_af90a08fec8ee28d))}}}
                }
            }else{
                Text(file!!.coreText("path"),Modifier.padding(horizontal=12.dp),fontFamily=FontFamily.Monospace,style=MaterialTheme.typography.labelMedium)
                OutlinedTextField(value=draft,onValueChange={draft=it},modifier=Modifier.fillMaxWidth().weight(1f).padding(12.dp),enabled=!blocked,textStyle=MaterialTheme.typography.bodySmall.copy(fontFamily=FontFamily.Monospace))
                Row(Modifier.padding(12.dp),horizontalArrangement=Arrangement.spacedBy(8.dp)){
                    Button(enabled=dirty&&!blocked&&!busy,onClick={val selected=file!!;submitted=selected.coreText("path") to draft;model.execute(RovaiOperation.WORKBENCH_FILES_MUTATE,buildJsonObject{put("scope",scope);put("commandId",UUID.randomUUID().toString());put("action","save");put("path",selected.coreText("path"));put("text",draft);put("expectedDigest",selected.coreText("digest"))})}){Text(stringResource(R.string.rovai_fusion_a3030bf8f16dc63c))}
                    TextButton(enabled=!busy,onClick={if(dirty){exitRequested=false;discard=true}else file=null}){Text(stringResource(R.string.rovai_fusion_9a46fcf5ce5f9618))}
                    TextButton(enabled=!busy,onClick={work{val current=model.query(RovaiOperation.WORKBENCH_FILES_READ,buildJsonObject{put("scope",scope);put("path",file!!.coreText("path"))}).coreObject();if(current.coreText("digest")!=file!!.coreText("digest")){issue="宿主文件已变化；你的编辑仍保留。请核对后另存或重新读取。"}else issue="宿主文件版本未变化。"}}){Text(stringResource(R.string.rovai_fusion_247b60f620687179))}
                }
            }
        }else{
            Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState())){listOf("status","diff","diff_staged","log","branches","stash","remotes","worktrees","reflog","tags").forEach{view->FilterChip(selected=gitView==view,onClick={gitView=view;work{readGit()}},label={Text(view)})}}
            LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(12.dp)){items(gitOutput.replace('\u0000','\n').split('\u001e','\n')){line->Text(line,Modifier.fillMaxWidth().padding(vertical=3.dp),fontFamily=FontFamily.Monospace,style=MaterialTheme.typography.bodySmall)}}
            Column(Modifier.fillMaxWidth().padding(12.dp),verticalArrangement=Arrangement.spacedBy(6.dp)){
                Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState())){listOf("stage","unstage","commit","fetch","pull","push","checkout","branch_create","merge","rebase","merge_continue","merge_abort","rebase_continue","rebase_abort","stash_save","stash_pop","stash_apply","reset","cherry_pick","revert").forEach{value->FilterChip(selected=action==value,onClick={action=value},label={Text(value)})}}
                OutlinedTextField(paths,{paths=it},label={Text(stringResource(R.string.rovai_fusion_e85afedeb4368a03))},modifier=Modifier.fillMaxWidth(),maxLines=2)
                Row(horizontalArrangement=Arrangement.spacedBy(6.dp)){OutlinedTextField(target,{target=it},label={Text(stringResource(R.string.rovai_fusion_6273ed3124b0664f))},modifier=Modifier.weight(1f));OutlinedTextField(mode,{mode=it},label={Text(stringResource(R.string.rovai_fusion_47a270081ab2892f))},modifier=Modifier.weight(1f))}
                OutlinedTextField(message,{message=it},label={Text(stringResource(R.string.rovai_fusion_969ba563d0e79749))},modifier=Modifier.fillMaxWidth(),maxLines=2)
                Button(enabled=!busy&&!blocked,onClick={work{val args=buildJsonObject{put("scope",scope);put("action",action);put("paths",JsonArray(paths.lines().filter{it.isNotBlank()}.map(::JsonPrimitive)));if(target.isNotBlank())put("target",target);if(message.isNotBlank())put("message",message);if(mode.isNotBlank())put("mode",mode)};val preview=model.query(RovaiOperation.WORKBENCH_GIT_PREVIEW,args).coreObject();planArgs=args;plan=preview}}){Text(stringResource(R.string.rovai_fusion_91e4bc0aafb2a907))}
            }
        }
    }
    if(discard)AlertDialog(onDismissRequest={discard=false},title={Text(stringResource(R.string.rovai_fusion_a75cac035a6d8a15))},confirmButton={TextButton(onClick={discard=false;file=null;if(exitRequested)onClose()}){Text(stringResource(R.string.rovai_fusion_9b7824cefa1e8b16))}},dismissButton={TextButton(onClick={discard=false}){Text(stringResource(R.string.rovai_fusion_fd4b9e3b6c685bae))}})
    plan?.let{preview->AlertDialog(onDismissRequest={plan=null},title={Text(stringResource(R.string.rovai_fusion_1351aa5bc2afebba))},text={Column{Text(preview["arguments"].toString(),fontFamily=FontFamily.Monospace);Text(preview.coreText("cleanPreview"))}},confirmButton={TextButton(enabled=!blocked,onClick={val args=planArgs?:return@TextButton;model.execute(RovaiOperation.WORKBENCH_GIT_EXECUTE,buildJsonObject{args.forEach{(key,value)->put(key,value)};put("commandId",UUID.randomUUID().toString());put("expectedRevision",preview.coreText("revision"));put("confirmed",true)});plan=null}){Text(stringResource(R.string.rovai_fusion_68a35e2242686dcc))}},dismissButton={TextButton(onClick={plan=null}){Text(stringResource(R.string.rovai_fusion_2cd0f3be8738a86c))}})}
}
