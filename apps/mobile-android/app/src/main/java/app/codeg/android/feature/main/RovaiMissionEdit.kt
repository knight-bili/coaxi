package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
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

/** Reuses MissionEdit's definition/version boundary; membership and lead remain separate actions. */
@Composable fun RovaiMissionEdit(model:RovaiWorkspaceViewModel,mission:JsonObject,onClose:()->Unit){
    val state=model.state.collectAsState().value
    val scope=rememberCoroutineScope()
    val id=mission.coreText("missionId")
    val generation=remember(id){model.connectionGeneration}
    val owner="mission-edit:$id"
    fun contentOf(value:JsonObject)=(value["descriptionContent"]as?JsonArray)?:JsonArray(listOf(buildJsonObject{put("kind","text");put("text",value.coreText("description"))}))
    var baseline by remember(id){mutableStateOf(mission)}
    var title by remember(id){mutableStateOf(mission.coreText("title"))}
    var content by remember(id){mutableStateOf(contentOf(mission))}
    var tags by remember(id){mutableStateOf((mission["tags"]as?JsonArray)?.joinToString(", "){(it as?JsonPrimitive)?.contentOrNull.orEmpty()}.orEmpty())}
    var kept by remember(id){mutableStateOf(mission["attachments"].coreArray().map{it.coreText("id")})}
    var sources by remember(id){mutableStateOf<List<JsonObject>>(emptyList())}
    var commandId by remember(id){mutableStateOf(UUID.randomUUID().toString())}
    var legacySent by remember(id){mutableStateOf<JsonObject?>(null)}
    var ready by remember(id){mutableStateOf(false)}
    var busy by remember(id){mutableStateOf(false)}
    var issue by remember(id){mutableStateOf<String?>(null)}
    var latest by remember(id){mutableStateOf<JsonObject?>(null)}
    var sourcePreview by remember(id){mutableStateOf<JsonObject?>(null)}
    val blocked=!ready||busy||state.busy||state.pending!=null||generation!=model.connectionGeneration
    val locked=blocked||legacySent!=null
    val parsedTags=tags.split(',','，','\n').map(String::trim).filter(String::isNotEmpty).distinct()
    val rendered=missionDescriptionDisplay(content,state.members)
    val changed=title.trim()!=baseline.coreText("title")||content!=contentOf(baseline)||JsonArray(parsedTags.map(::JsonPrimitive))!=baseline["tags"]||sources.isNotEmpty()||kept!=baseline["attachments"].coreArray().map{it.coreText("id")}
    val descriptionLimit=if(baseline.coreLong("descriptionLimit")==20000L)20000 else 12000
    val valid=title.isNotBlank()&&title.trim().codePointCount(0,title.trim().length)<=200&&rendered.codePointCount(0,rendered.length)<=descriptionLimit&&parsedTags.size<=30&&parsedTags.all{it.codePointCount(0,it.length)<=24}&&kept.size+sources.size<=10
    fun document()=buildJsonObject{
        put("version",2);put("title",title);put("description",rendered);put("structuredDescription",content);put("tags",tags);put("baseline",baseline)
        put("kept",JsonArray(kept.map(::JsonPrimitive)));put("sources",JsonArray(sources));put("commandId",commandId);put("sent",legacySent?:JsonNull)
    }.toString()
    fun persist(){if(ready&&generation==model.connectionGeneration)model.rememberBody(owner,document())}
    fun finishReceipt(receipt:JsonObject){
        if(receipt.coreText("commandId")!=commandId)return
        when(receipt.coreText("status")){
            "applied","accepted"->{ready=false;model.rememberBody(owner,"");onClose()}
            "rejected"->{legacySent=null;commandId=UUID.randomUUID().toString();issue=receipt.coreText("code");persist()}
        }
    }
    fun close(){if(busy)return;if(!ready||generation!=model.connectionGeneration){onClose();return};val saved=document();scope.launch{try{model.rememberBody(owner,saved);model.saveBody(owner,saved);onClose()}catch(error:Exception){issue=error.message}}}
    val picker=rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()){uris->
        if(uris.isNotEmpty()&&!locked){
            if(kept.size+sources.size+uris.size>10)issue="使命最多保留10份附件"
            else{busy=true;val previous=sources;scope.launch{
                val added=mutableListOf<JsonObject>()
                try{for(uri in uris){added+=model.prepareMissionFile(uri);require(generation==model.connectionGeneration){"工作区已切换"}};sources=previous+added;val saved=document();model.saveBody(owner,saved);model.rememberBody(owner,saved)}
                catch(cancelled:CancellationException){added.forEach{model.removeMissionFile(it)};throw cancelled}
                catch(error:Exception){sources=previous;added.forEach{model.removeMissionFile(it)};issue=error.message}
                finally{busy=false}
            }}
        }
    }
    LaunchedEffect(id,generation){try{
        val raw=model.readBody(owner)
        if(raw.isNotEmpty()){
            val value=Json.parseToJsonElement(raw).coreObject();val version=value.coreLong("version");require(version==0L||version==2L){"原使命草稿格式无法读取，原记录保留"}
            val previous=value["baseline"].coreObject();require(previous.coreText("missionId")==id){"原草稿的使命标识不一致，原记录保留"}
            baseline=previous;title=value.coreText("title");tags=value.coreText("tags");commandId=value.coreText("commandId").ifEmpty{UUID.randomUUID().toString()};legacySent=value["sent"]as?JsonObject
            content=(value["structuredDescription"]as?JsonArray)?:if(value.coreText("description")==previous.coreText("description"))contentOf(previous)else JsonArray(listOf(buildJsonObject{put("kind","text");put("text",value.coreText("description"))}))
            kept=if(version==2L)(value["kept"]as?JsonArray)?.mapNotNull{(it as?JsonPrimitive)?.contentOrNull}?:throw IllegalArgumentException("原附件保留清单无法读取")else previous["attachments"].coreArray().map{it.coreText("id")}
            sources=value["sources"].coreArray()
        }
        ready=true
    }catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}}
    LaunchedEffect(state.lastResult){
        val receipt=state.lastResult.coreObject()["commandResult"].coreObject().ifEmpty{state.lastResult.coreObject()}
        if(state.lastCompletedOperation==RovaiOperation.MISSIONS_UPDATE.wire&&(state.lastInputOwner==owner||legacySent!=null))finishReceipt(receipt)
    }
    sourcePreview?.let{source->RovaiFilePreview(model,source){sourcePreview=null}}
    Dialog(onDismissRequest={close()},properties=DialogProperties(usePlatformDefaultWidth=false,dismissOnBackPress=!busy,dismissOnClickOutside=false)){
        Surface(Modifier.fillMaxSize(),color=CodegTheme.colors.bgElevated){Column(Modifier.fillMaxSize()){
            Row(Modifier.fillMaxWidth().padding(12.dp)){Text(stringResource(R.string.rovai_fusion_71311e4c8ed3af89),Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(enabled=!busy,onClick={close()}){Text(stringResource(R.string.rovai_fusion_6deb10821f4a69df))}}
            LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
                item{issue?.let{Text(it,color=CodegTheme.colors.danger)};if(state.pending!=null)Text(stringResource(R.string.rovai_fusion_31f4771c12e94b4c),color=CodegTheme.colors.textSecondary)}
                item{OutlinedTextField(title,{title=it;persist()},Modifier.fillMaxWidth(),label={Text(stringResource(R.string.rovai_fusion_d44e9b3d3b31d37b))},enabled=!locked);RovaiMissionDescriptionInput(content,state.members,locked){content=it;persist()};OutlinedTextField(tags,{tags=it;persist()},Modifier.fillMaxWidth(),label={Text(stringResource(R.string.rovai_fusion_d4ed5f32246247d7))},enabled=!locked)}
                item{Text(stringResource(R.string.rovai_fusion_f53ffe386595d985)+baseline.coreText("projectPath").ifEmpty{"快速对话"},color=CodegTheme.colors.textSecondary);val ids=(baseline["memberAgentIds"]as?JsonArray)?.mapNotNull{(it as?JsonPrimitive)?.contentOrNull}.orEmpty();Text(stringResource(R.string.rovai_fusion_fd98bb4534b9621b)+ids.joinToString("、"){member->state.members.firstOrNull{it.coreText("agentId")==member}?.coreText("displayName")?:"已不可用队员"},color=CodegTheme.colors.textSecondary);Text(stringResource(R.string.rovai_fusion_aaa1a47d9c634bd8),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}
                item{Row{Text("附件 · ${kept.size+sources.size}/10",Modifier.weight(1f));TextButton(enabled=!locked&&kept.size+sources.size<10,onClick={picker.launch(arrayOf("*/*"))}){Text(stringResource(R.string.rovai_fusion_c8a47d0018480abe))}}}
                items(baseline["attachments"].coreArray(),key={it.coreText("id")}){attachment->val attachmentId=attachment.coreText("id");Row{
                    TextButton(modifier=Modifier.weight(1f),onClick={sourcePreview=buildJsonObject{put("kind","attachment");put("threadId",baseline.coreText("threadId"));putJsonObject("locator"){put("owner","mission");put("threadId",baseline.coreText("threadId"));put("missionId",id);put("attachmentRefId",attachmentId)}}}){Text(attachment.coreText("displayName"))}
                    TextButton(enabled=!locked&&(attachmentId in kept||kept.size+sources.size<10),onClick={kept=if(attachmentId in kept)kept-attachmentId else baseline["attachments"].coreArray().map{it.coreText("id")}.filter{it in kept||it==attachmentId};persist()}){Text(if(attachmentId in kept)"移除引用" else "撤销移除")}
                }}
                items(sources,key={it["intent"].coreObject().coreText("commandId")}){source->Row{Text(source["intent"].coreObject().coreText("displayName"),Modifier.weight(1f));TextButton(enabled=!locked,onClick={busy=true;scope.launch{val previous=sources;try{sources=sources-source;val saved=document();model.saveBody(owner,saved);model.rememberBody(owner,saved);model.removeMissionFile(source)}catch(error:Exception){sources=previous;persist();issue=error.message}finally{busy=false}}}){Text(stringResource(R.string.rovai_fusion_c9f85ca58072dd36))}}}
                if(legacySent!=null)item{Text(stringResource(R.string.rovai_fusion_daa2504dbfa8ba08),color=CodegTheme.colors.textSecondary);Row{
                    TextButton(enabled=!blocked,onClick={busy=true;scope.launch{try{val result=model.query(RovaiOperation.COMMANDS_RECONCILE,buildJsonObject{put("operation",RovaiOperation.MISSIONS_UPDATE.wire);put("params",legacySent!!)}).coreObject();require(generation==model.connectionGeneration);if(result.coreText("state")=="recorded")finishReceipt(result["result"].coreObject())else issue="原结果尚未记录；可以稍后核对或重试原保存。"}catch(error:Exception){issue=error.message}finally{busy=false}}}){Text(stringResource(R.string.rovai_fusion_7eece029ee75bcc8))}
                    TextButton(enabled=!blocked,onClick={legacySent?.let{model.execute(RovaiOperation.MISSIONS_UPDATE,it)}}){Text(stringResource(R.string.rovai_fusion_656b7fa85744d5a5))}
                }}
                item{TextButton(enabled=!locked,onClick={busy=true;scope.launch{try{val value=model.query(RovaiOperation.MISSIONS_GET,buildJsonObject{put("missionId",id)}).coreObject();require(generation==model.connectionGeneration);latest=value}catch(error:Exception){issue=error.message}finally{busy=false}}}){Text(stringResource(R.string.rovai_fusion_5a55e40e4ab99f7e))}}
            }
            Button(enabled=!locked&&valid&&changed,modifier=Modifier.fillMaxWidth().padding(12.dp),onClick={
                val command=buildJsonObject{put("missionId",id);put("expectedDetailsVersion",baseline.coreLong("detailsVersion"));put("title",title.trim());put("descriptionContent",content);put("tags",JsonArray(parsedTags.map(::JsonPrimitive)))}
                val saved=document();val frozenSources=sources;val frozenKept=kept;val frozenId=commandId;busy=true
                scope.launch{try{model.saveBody(owner,saved);model.rememberBody(owner,saved);require(generation==model.connectionGeneration);model.executeMissionBatch(RovaiOperation.MISSIONS_UPDATE,frozenId,command,frozenKept,frozenSources,owner,saved)}catch(error:Exception){issue=error.message}finally{busy=false}}
            }){Text(stringResource(R.string.rovai_fusion_ac31d901d68d29e7))}
        }}
    }
    latest?.let{value->AlertDialog(onDismissRequest={latest=null},title={Text(stringResource(R.string.rovai_fusion_1e6c01014a598eca))},text={Column{Text(value.coreText("title"));Text(missionDescriptionDisplay(contentOf(value),state.members).take(1600));Text(stringResource(R.string.rovai_fusion_e780d10eb13b66f2))}},confirmButton={TextButton(enabled=!locked,onClick={busy=true;scope.launch{val abandoned=sources;try{val oldBaseline=baseline;val oldTitle=title;val oldContent=content;val oldTags=tags;val oldKept=kept;val oldId=commandId
                    baseline=value;title=value.coreText("title");content=contentOf(value);tags=(value["tags"]as?JsonArray)?.joinToString(", "){(it as?JsonPrimitive)?.contentOrNull.orEmpty()}.orEmpty();kept=value["attachments"].coreArray().map{it.coreText("id")};sources=emptyList();commandId=UUID.randomUUID().toString()
                    val saved=document();try{model.saveBody(owner,saved);model.rememberBody(owner,saved)}catch(error:Exception){baseline=oldBaseline;title=oldTitle;content=oldContent;tags=oldTags;kept=oldKept;sources=abandoned;commandId=oldId;throw error};abandoned.forEach{model.removeMissionFile(it)};latest=null;issue=null
                }catch(error:Exception){issue=error.message}finally{busy=false}}}){Text(stringResource(R.string.rovai_fusion_42633162706d3171))}},dismissButton={TextButton(onClick={latest=null}){Text(stringResource(R.string.rovai_fusion_5408c29c1aa03b31))}})}
}
