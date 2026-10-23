package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*
import java.util.UUID

@Composable fun RovaiForgeWriteActions(model:RovaiWorkspaceViewModel,threadId:String,accountId:String,kind:String,repository:JsonObject?,selected:JsonObject?,onChanged:()->Unit){
    var action by remember(threadId,accountId,kind,selected?.forgeNumber()){mutableStateOf<String?>(null)}
    val blocked=model.state.collectAsState().value.pending!=null
    Row(Modifier.fillMaxWidth().padding(horizontal=12.dp),horizontalArrangement=Arrangement.spacedBy(8.dp)){
        if(selected==null)TextButton(enabled=repository!=null&&!blocked,onClick={action=if(kind=="pulls")"create_pr" else "create_issue"}){Text(if(kind=="pulls")"新建PR / MR" else "新建议题")}
        else{TextButton(enabled=repository!=null&&!blocked,onClick={action=if(kind=="pulls")"comment_pr" else "comment_issue"}){Text(stringResource(R.string.rovai_fusion_0bfca7bdd2207da4))};TextButton(enabled=repository!=null&&!blocked,onClick={action=if(kind=="pulls")"update_pr" else "update_issue"}){Text(stringResource(R.string.rovai_fusion_051836569928a9f9))}}
    }
    action?.let{operation->if(repository!=null)key(threadId,accountId,kind,selected?.forgeNumber(),operation){RovaiForgeWriteEditor(model,threadId,accountId,operation,repository,selected,onDismiss={action=null},onSaved={action=null;onChanged()})}}
}

/** Frozen command IDs reuse Core's remote-write receipt; unknown never becomes a new post. */
@Composable private fun RovaiForgeWriteEditor(model:RovaiWorkspaceViewModel,threadId:String,accountId:String,action:String,repository:JsonObject,selected:JsonObject?,onDismiss:()->Unit,onSaved:()->Unit){
    val owner="forge-write:$threadId:$accountId:$action:${selected?.forgeNumber()?:0}";val generation=model.connectionGeneration;val coroutine=rememberCoroutineScope()
    var title by remember{mutableStateOf(if(action.startsWith("update_"))selected?.coreText("title").orEmpty()else "")};var body by remember{mutableStateOf(if(action.startsWith("update_"))selected?.coreText("body")?.ifEmpty{selected.coreText("description")}.orEmpty()else "")};var head by remember{mutableStateOf("")};var base by remember{mutableStateOf("")};var draft by remember{mutableStateOf(false)};var status by remember{mutableStateOf(if(selected?.coreText("state")=="closed")"closed" else "open")}
    var ready by remember{mutableStateOf(false)};var busy by remember{mutableStateOf(false)};var confirming by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)};var commandId by remember{mutableStateOf(UUID.randomUUID().toString())};var frozen by remember{mutableStateOf<JsonObject?>(null)}
    fun record()=buildJsonObject{put("title",title);put("body",body);put("head",head);put("base",base);put("draft",draft);put("state",status);put("commandId",commandId);put("sent",frozen?:JsonNull);put("repositoryKey",repository.coreText("repositoryKey"))}.toString()
    LaunchedEffect(owner){try{val saved=model.readBody(owner);if(saved.isNotEmpty()){val value=Json.parseToJsonElement(saved).coreObject();require(value.coreText("repositoryKey")==repository.coreText("repositoryKey")){"工程远程仓库已变化，保留旧草稿；请在原仓库核对原操作"};title=value.coreText("title");body=value.coreText("body");head=value.coreText("head");base=value.coreText("base");draft=value["draft"]==JsonPrimitive(true);status=value.coreText("state");commandId=value.coreText("commandId").ifEmpty{commandId};frozen=value["sent"] as?JsonObject};ready=true}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}}
    val document=record()
    LaunchedEffect(document,ready){if(ready&&!busy){delay(350);try{model.rememberBody(owner,document)}catch(error:Exception){issue=error.message}}}
    fun submit(){if(!ready||busy||generation!=model.connectionGeneration)return;busy=true;issue=null;coroutine.launch{try{
        val sent=frozen?:buildJsonObject{putJsonObject("scope"){put("threadId",threadId)};put("action",action);if(accountId.isNotEmpty())put("accountId",accountId);if(selected!=null)put("number",selected.forgeNumber());put("title",title);put("body",body);if(action.startsWith("update_"))put("state",status);if(action=="create_pr"){put("head",head);put("base",base);put("draft",draft)};put("commandId",commandId);put("confirmed",true);put("expectedRepositoryKey",repository.coreText("repositoryKey"))}
        frozen=sent;val saved=record();model.rememberBody(owner,saved);model.saveBody(owner,saved)
        val result=model.query(RovaiOperation.FORGE_OPERATION,sent).coreObject();require(generation==model.connectionGeneration){"工作区已切换，原请求已保留"};require(result["success"]==JsonPrimitive(true)){"远端返回状态 ${result.coreLong("status")}，原请求已保留，请核对远端结果"}
        model.rememberBody(owner,"");onSaved()
    }catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false;confirming=false}}}
    Dialog(onDismissRequest={if(!busy)onDismiss()},properties=DialogProperties(usePlatformDefaultWidth=false)){Surface(Modifier.fillMaxSize(),color=CodegTheme.colors.bgElevated){Column(Modifier.fillMaxSize().padding(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
        Row{Text(when{action.startsWith("comment_")->"发表评论";action.startsWith("update_")->"编辑远端记录";else->"新建远端记录"},Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(enabled=!busy,onClick=onDismiss){Text(stringResource(R.string.rovai_fusion_5af4243f90419cd6))}}
        Text(repository.coreText("webUrl"),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary);issue?.let{Text(it,color=CodegTheme.colors.danger)};if(!ready)Text(stringResource(R.string.rovai_fusion_9cfa039e4e19c1fd))
        LazyColumn(Modifier.weight(1f),verticalArrangement=Arrangement.spacedBy(10.dp)){
            if(!action.startsWith("comment_"))item{OutlinedTextField(title,{title=it},enabled=ready&&!busy&&frozen==null,label={Text(stringResource(R.string.rovai_fusion_c3405f8c7d9d392a))},modifier=Modifier.fillMaxWidth())}
            item{OutlinedTextField(body,{body=it},enabled=ready&&!busy&&frozen==null,label={Text(stringResource(R.string.rovai_fusion_d661c3d96d53ebc0))},modifier=Modifier.fillMaxWidth(),minLines=6,maxLines=18)}
            if(action=="create_pr"){item{OutlinedTextField(head,{head=it},enabled=ready&&!busy&&frozen==null,label={Text(stringResource(R.string.rovai_fusion_5c50cb4aa40480b4))},modifier=Modifier.fillMaxWidth())};item{OutlinedTextField(base,{base=it},enabled=ready&&!busy&&frozen==null,label={Text(stringResource(R.string.rovai_fusion_f3d0ff87958d7d44))},modifier=Modifier.fillMaxWidth())};item{Row{Checkbox(draft,{draft=it},enabled=ready&&!busy&&frozen==null);Text(stringResource(R.string.rovai_fusion_2a2fd29bd27a6eb9))}}}
            if(action.startsWith("update_"))item{Row{FilterChip(selected=status=="open",enabled=ready&&!busy&&frozen==null,onClick={status="open"},label={Text(stringResource(R.string.rovai_fusion_c14c915da5fbec64))});FilterChip(selected=status=="closed",enabled=ready&&!busy&&frozen==null,onClick={status="closed"},label={Text(stringResource(R.string.rovai_fusion_3fd47edce45b3603))})}}
            if(frozen!=null)item{Text(stringResource(R.string.rovai_fusion_24e9f08baab26ea6),color=CodegTheme.colors.textSecondary)}
        }
        Button(enabled=ready&&!busy&&(frozen!=null||if(action.startsWith("comment_"))body.isNotBlank()else title.isNotBlank()&&(action!="create_pr"||head.isNotBlank()&&base.isNotBlank())),onClick={confirming=true},modifier=Modifier.fillMaxWidth()){Text(if(frozen!=null)"核对原操作" else "确认提交")}
    }}}
    if(confirming)AlertDialog(onDismissRequest={if(!busy)confirming=false},title={Text(if(frozen!=null)"核对这次原操作？" else "提交到远程仓库？")},text={Column{Text(repository.coreText("ownerRepo"));if(selected!=null)Text("#${selected.forgeNumber()}");if(title.isNotBlank())Text(title);Text(stringResource(R.string.rovai_fusion_39e64f3f503ac353))}},confirmButton={TextButton(enabled=!busy,onClick=::submit){Text(stringResource(R.string.rovai_fusion_36f33adaf0942634))}},dismissButton={TextButton(enabled=!busy,onClick={confirming=false}){Text(stringResource(R.string.rovai_fusion_2cd0f3be8738a86c))}})
}
