package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.component.SecretField
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import java.util.UUID
import java.util.concurrent.atomic.AtomicReference

private val migrationDomains=mapOf("modelProviders" to "模型服务商","agentSettings" to "智能体设置","customAgents" to "自定义程序","quickMessages" to "快捷语","taskTemplates" to "任务模板","preferences" to "可迁移偏好")

/** 原移动端设置列表承载原配置同步的选范围、预览、确认与撤销，不在导入时运行任务。 */
@Composable
fun RovaiPortableConfigurationMigration(model:RovaiWorkspaceViewModel,onClose:()->Unit){
    val context=LocalContext.current;val scope=rememberCoroutineScope();val state by model.state.collectAsStateWithLifecycle()
    val sourceRef=remember{AtomicReference<RovaiConfigurationSource?>(null)}
    var source by remember{mutableStateOf<RovaiConfigurationSource?>(null)};var passphrase by remember{mutableStateOf("")}
    var identity by remember{mutableStateOf("")};var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)}
    var archives by remember{mutableStateOf<List<JsonObject>>(emptyList())};var archiveNext by remember{mutableStateOf<String?>(null)};var archive by remember{mutableStateOf<JsonObject?>(null)}
    var rows by remember{mutableStateOf<List<JsonObject>>(emptyList())};var visible by remember{mutableStateOf(50)};var choices by remember{mutableStateOf<Map<String,JsonObject>>(emptyMap())}
    var targets by remember{mutableStateOf<Map<String,List<JsonObject>>>(emptyMap())};var targetNext by remember{mutableStateOf<Map<String,String?>>(emptyMap())}
    var mappings by remember{mutableStateOf<Map<String,JsonObject>>(emptyMap())};var preview by remember{mutableStateOf<JsonObject?>(null)};var undo by remember{mutableStateOf(false)}
    var pending by remember{mutableStateOf<JsonObject?>(null)};var last by remember{mutableStateOf<JsonObject?>(null)};var receipt by remember{mutableStateOf<JsonObject?>(null)}
    var confirm by remember{mutableStateOf(false)};var memberMenu by remember{mutableStateOf(false)}
    var exportArchive by remember{mutableStateOf<JsonObject?>(null)}
    if(exportArchive!=null){RovaiPortableConfigurationExport(model,sourceArchive=exportArchive){exportArchive=null};return}
    val disabled=busy||identity.isEmpty();val selected=choices.values.filter{it["decision"].coreObject().coreText("action")!="skip"}
    fun owner()="portable-migration/$identity"
    suspend fun current(){if(identity.isEmpty()||model.presentationIdentity()!=identity)throw CancellationException("工作区已切换")}
    suspend fun remember(value:JsonObject?){current();model.saveBody(owner(),value?.toString()?:"");current();pending=value}
    suspend fun release(){sourceRef.getAndSet(null)?.let{releaseRovaiConfiguration(context,it)};source=null}
    suspend fun loadArchives(more:Boolean=false){val page=model.query(RovaiOperation.CONFIGURATION_MIGRATION_ARCHIVES,buildJsonObject{put("after",if(more)archiveNext?.let(::JsonPrimitive)?:JsonNull else JsonNull);put("limit",100)}).coreObject();current();archives=if(more)archives+page["items"].coreArray() else page["items"].coreArray();archiveNext=(page["next"] as? JsonPrimitive)?.contentOrNull}
    suspend fun open(value:JsonObject){val result=model.query(RovaiOperation.CONFIGURATION_MIGRATION_PREVIEW,buildJsonObject{put("schemaVersion",1);put("archiveId",value["archiveId"]!!);put("sourceDigest",value["sourceDigest"]!!);put("selections",JsonArray(emptyList()));put("members",JsonArray(emptyList()))}).coreObject();current();archive=value;rows=result["rows"].coreArray();visible=50;choices=emptyMap();mappings=emptyMap();preview=null;undo=false;targets=emptyMap();targetNext=emptyMap()}
    suspend fun loadTargets(domain:String,more:Boolean=false){val page=model.query(RovaiOperation.CONFIGURATION_MIGRATION_TARGETS,buildJsonObject{put("domain",domain);put("after",if(more)targetNext[domain]?.let(::JsonPrimitive)?:JsonNull else JsonNull);put("limit",100)}).coreObject();current();targets=targets+(domain to if(more)targets[domain].orEmpty()+page["items"].coreArray() else page["items"].coreArray());targetNext=targetNext+(domain to (page["next"] as? JsonPrimitive)?.contentOrNull)}
    fun decision(row:JsonObject,action:String,target:JsonObject?=null){val id=row.coreText("entryId");choices=choices+(id to buildJsonObject{put("entryId",id);put("domain",row["domain"]!!);put("decision",buildJsonObject{put("entryId",id);put("action",action);put("targetId",if(action=="replace")target?.get("id")?:JsonNull else JsonNull);put("expectedRevision",if(action=="replace")target?.get("revision")?:JsonNull else JsonNull)})});preview=null;undo=false}
    fun work(action:suspend()->Unit){if(busy)return;busy=true;issue=null;scope.launch{try{current();action()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message?:"配置处理未完成"}finally{busy=false}}}
    suspend fun readReceipt(command:JsonObject){
        val value=model.query(RovaiOperation.CONFIGURATION_MIGRATION_RECEIPT,buildJsonObject{put("schemaVersion",1);put("commandId",command["commandId"]!!)}).coreObject();current()
        require(value["commandId"]==command["commandId"]&&value["previewId"]==command["previewId"]&&value["previewDigest"]==command["previewDigest"]){"原请求回执不匹配"}
        receipt=value
        if(value.coreText("state") in listOf("applied","rejected")){
            if(value.coreText("state")=="applied"){model.saveBody(owner()+"/last",command.toString());last=command}
            if(pending?.get("commandId")==command["commandId"])remember(null)
        }
    }
    suspend fun registered(value:JsonObject,original:JsonObject){require(value["archiveId"]==original["commandId"]&&value["sourceDigest"]==original["sourceDigest"]){"候选文件与原请求不同"};remember(null);passphrase="";release();loadArchives();open(value)}
    LaunchedEffect(state.connected){
        if(!state.connected){identity="";archive=null;rows=emptyList();preview=null;pending=null;return@LaunchedEffect}
        busy=true
        try{identity=model.presentationIdentity()
            val allowed=setOf("operation","commandId","sourceDigest","archiveId","previewId","previewDigest")
            fun decoded(raw:String):JsonObject?=raw.takeIf(String::isNotBlank)?.let{Json.parseToJsonElement(it).coreObject().also{value->require(value.keys.all{it in allowed}&&value.coreText("operation") in listOf("configuration.archive.register","configuration.migration.confirm","configuration.migration.undoConfirm"));UUID.fromString(value.coreText("commandId"));require(value.values.all{it is JsonPrimitive&&it.content.length<=256})}}
            pending=decoded(model.readBody(owner()));last=decoded(model.readBody(owner()+"/last"));loadArchives()
            val original=pending?:last;if(original!=null&&original.coreText("operation")!="configuration.archive.register")readReceipt(original)
        }catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue="迁移列表或原请求恢复失败"}finally{busy=false}
    }
    DisposableEffect(Unit){onDispose{val old=sourceRef.getAndSet(null);if(old!=null)kotlinx.coroutines.CoroutineScope(kotlinx.coroutines.Dispatchers.IO).launch{releaseRovaiConfiguration(context,old)}}}
    val picker=rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()){uri->if(uri!=null)work{val value=prepareRovaiConfiguration(context,uri);try{current();release();source=value;sourceRef.set(value)}catch(error:Exception){releaseRovaiConfiguration(context,value);throw error}}}
    BackHandler(enabled=!busy){onClose()}
    LazyColumn(Modifier.fillMaxSize(),contentPadding=PaddingValues(12.dp),verticalArrangement=Arrangement.spacedBy(8.dp)){
        item{Row{TextButton(onClick=onClose,enabled=!busy){Text(stringResource(R.string.rovai_fusion_572cf45ba43634b3))};TextButton(onClick={work{loadArchives()}},enabled=!disabled){Text(stringResource(R.string.rovai_fusion_3b105bd177b17ddf))}};Text(stringResource(R.string.rovai_fusion_a3fe615f2784baaf),style=MaterialTheme.typography.titleLarge);Text(stringResource(R.string.rovai_fusion_94b166989d99fd88),style=MaterialTheme.typography.bodySmall)}
        if(pending!=null)item{GlassRow{
            Text(stringResource(R.string.rovai_fusion_312e15c753920199))
            TextButton(enabled=!disabled,onClick={work{val original=pending?:return@work;if(original.coreText("operation")=="configuration.archive.register"){val result=model.privateConfiguration("receipt",buildJsonObject{put("operationId",original["commandId"]!!)});val saved=result["archive"].coreObject();if(saved.isEmpty())error("尚未查到原文件登记，请稍后核对");registered(saved,original)}else readReceipt(original)}}){Text(stringResource(R.string.rovai_fusion_d027c9aa232e6375))}
        }}
        item{GlassRow{
            TextButton(enabled=!disabled&&(pending==null||pending?.coreText("operation")=="configuration.archive.register"),onClick={picker.launch(arrayOf("application/json","application/octet-stream","*/*"))}){Text(source?.displayName?:"选择原来的配置文件")}
            SecretField(passphrase,{passphrase=it},"文件加密口令（未加密可留空）")
            TextButton(enabled=!disabled&&source!=null&&(pending==null||pending?.coreText("operation")=="configuration.archive.register"),onClick={work{
                val file=source?:error("请先选择文件");val original=pending?:buildJsonObject{put("operation","configuration.archive.register");put("commandId",UUID.randomUUID().toString());put("sourceDigest",file.sourceDigest)}
                require(original.coreText("sourceDigest")==file.sourceDigest){"继续原请求时须选择同一份文件"};if(pending==null)remember(original)
                val value=model.configurationArchive(buildJsonObject{put("operationId",original["commandId"]!!);put("sourceDigest",file.sourceDigest);put("byteSize",file.byteSize);put("passphrase",passphrase);put("manifest",JsonNull)},file.file);current();registered(value,original)
            }}){Text(if(pending?.coreText("operation")=="configuration.archive.register")"继续登记原文件" else "读取为迁移候选")}
        }}
        items(archives,key={it.coreText("archiveId")}){value->TextButton(enabled=!disabled&&pending==null,onClick={work{open(value)}}){Text("候选 · ${value.coreText("sourceKind")} · ${value.coreText("createdAt")}")}}
        if(archiveNext!=null)item{TextButton(enabled=!disabled,onClick={work{loadArchives(true)}}){Text(stringResource(R.string.rovai_fusion_136e66cc37ff7d47))}}
        if(archive!=null){
            item{TextButton(enabled=!disabled&&pending==null&&source==null,onClick={exportArchive=buildJsonObject{put("archiveId",archive!!["archiveId"]!!);put("sourceDigest",archive!!["sourceDigest"]!!)}}){Text(stringResource(R.string.rovai_fusion_7c4da3d6bab9b375))}}
            item{Text(stringResource(R.string.rovai_fusion_9c9c4eb88f694660),style=MaterialTheme.typography.titleMedium)}
            items(rows.take(visible),key={it.coreText("entryId")}){row->
                val choice=choices[row.coreText("entryId")];val action=if(undo)row.coreText("action") else choice?.get("decision").coreObject().coreText("action").ifEmpty{"skip"};val domain=row.coreText("domain")
                GlassRow{Text(row.coreText("name"),style=MaterialTheme.typography.titleSmall);Text(migrationDomains[domain]?:"原值仅保留，尚无应用入口",style=MaterialTheme.typography.bodySmall)
                    if(domain in migrationDomains&&!undo)Row{listOf("skip" to "跳过","create" to "新增","replace" to "替换").forEach{(kind,label)->FilterChip(selected=action==kind,enabled=!disabled&&pending==null,onClick={decision(row,kind);if(kind=="replace"&&targets[domain]==null)work{loadTargets(domain)}},label={Text(label)})}}
                    if(action=="replace"&&!undo){
                        PortableMigrationChoice("选择准确的本机目标",targets[domain].orEmpty(),choice?.get("decision").coreObject().coreText("targetId"),!disabled&&pending==null){target->decision(row,"replace",target)}
                        if(targetNext[domain]!=null)TextButton(enabled=!disabled,onClick={work{loadTargets(domain,true)}}){Text(stringResource(R.string.rovai_fusion_bef53ff8bd0c07b1))}
                    }
                    val checked=preview?.get("rows").coreArray().find{it["entryId"]==row["entryId"]}
                    if(checked!=null&&action!="skip"){Text(checked.coreText("status"));if(checked.coreText("reason").isNotEmpty())Text(checked.coreText("reason"),color=MaterialTheme.colorScheme.error);var details by remember(row.coreText("entryId")){mutableStateOf(false)};TextButton(onClick={details=!details}){Text(stringResource(R.string.rovai_fusion_bdbc894fb7b67568))};if(details){Text("原值\n${checked["before"]}",style=MaterialTheme.typography.bodySmall);Text("将保存\n${checked["after"]}",style=MaterialTheme.typography.bodySmall)}}
                }
            }
            if(visible<rows.size)item{TextButton(onClick={visible+=50}){Text(stringResource(R.string.rovai_fusion_c56bfa115708896e))}}
            if(!undo)item{Text(stringResource(R.string.rovai_fusion_6964264a2426dc58),style=MaterialTheme.typography.titleMedium);Box{TextButton(enabled=!disabled&&pending==null,onClick={memberMenu=true}){Text(stringResource(R.string.rovai_fusion_64247e8d19205dc7))};DropdownMenu(expanded=memberMenu,onDismissRequest={memberMenu=false}){state.members.filter{it.coreText("presence")=="present"&&it.coreText("agentId") !in mappings}.forEach{member->DropdownMenuItem(text={Text(member.coreText("displayName"))},onClick={memberMenu=false;work{
                val id=member.coreText("agentId");val actual=model.query(RovaiOperation.CONFIGURATION_MEMBERCONNECTION_GET,buildJsonObject{put("schemaVersion",1);put("agentId",id)}).coreObject();current();val runtime=actual["runtime"].coreObject();require(runtime.isNotEmpty()){"请先为这位队员选择可用的程序"};val selection=runtime["modelSelection"]?:buildJsonObject{put("mode","runtime_default")}
                mappings=mappings+(id to buildJsonObject{put("agentId",id);put("expectedVersion",actual["agentVersion"]!!);put("installationId",runtime["installationId"]!!);put("expectedInstallationGeneration",runtime["installationGeneration"]!!);put("providerEntryId","");put("environmentEntryId",JsonNull);put("modelSelection",selection);put("originalSelection",selection)})
            }})}}}}
            if(!undo)items(mappings.values.toList(),key={it.coreText("agentId")}){mapping->GlassRow{
                val id=mapping.coreText("agentId");fun changed(name:String,value:JsonElement){mappings=mappings+(id to JsonObject(mapping.toMutableMap().apply{put(name,value)}));preview=null}
                Text(state.members.find{it.coreText("agentId")==id}?.coreText("displayName")?:id)
                val candidates=rows.filter{it.coreText("domain")=="modelProviders"&&choices[it.coreText("entryId")]?.get("decision").coreObject().coreText("action") in listOf("create","replace")}.map{buildJsonObject{put("id",it["entryId"]!!);put("name",it["name"]!!)}}
                PortableMigrationChoice("选择已勾选的服务商来源",candidates,mapping.coreText("providerEntryId"),!disabled&&pending==null){changed("providerEntryId",it["id"]!!)}
                PortableMigrationChoice("环境设置：留空或选准确来源",listOf(buildJsonObject{put("id","");put("name","不迁入环境值")})+rows.filter{it.coreText("domain")=="agentSettings"}.map{buildJsonObject{put("id",it["entryId"]!!);put("name",it["name"]!!)}},mapping.coreText("environmentEntryId"),!disabled&&pending==null){changed("environmentEntryId",it.coreText("id").takeIf(String::isNotEmpty)?.let(::JsonPrimitive)?:JsonNull)}
                Row{FilterChip(selected=mapping["modelSelection"]==mapping["originalSelection"],enabled=!disabled,onClick={changed("modelSelection",mapping["originalSelection"]!!)},label={Text(stringResource(R.string.rovai_fusion_fd860924a3da2d26))});FilterChip(selected=mapping["modelSelection"].coreObject().coreText("mode")=="runtime_default",enabled=!disabled,onClick={changed("modelSelection",buildJsonObject{put("mode","runtime_default")})},label={Text(stringResource(R.string.rovai_fusion_cd4677d7431b4397))})}
                TextButton(enabled=!disabled,onClick={mappings=mappings-id;preview=null}){Text(stringResource(R.string.rovai_fusion_b4796755677ce470))}
            }}
            item{Row{
                if(!undo)TextButton(enabled=!disabled&&pending==null&&selected.isNotEmpty()&&selected.none{it["decision"].coreObject().coreText("action")=="replace"&&it["decision"].coreObject()["targetId"] is JsonNull}&&mappings.values.none{it.coreText("providerEntryId").isEmpty()},onClick={work{
                    val value=model.query(RovaiOperation.CONFIGURATION_MIGRATION_PREVIEW,buildJsonObject{put("schemaVersion",1);put("archiveId",archive!!["archiveId"]!!);put("sourceDigest",archive!!["sourceDigest"]!!);put("selections",JsonArray(selected));put("members",JsonArray(mappings.values.map{JsonObject(it.filterKeys{key->key!="originalSelection"})}))}).coreObject();current();preview=value;undo=false
                }}){Text(stringResource(R.string.rovai_fusion_d66e8f934e64ac8f))}
                TextButton(enabled=!disabled&&pending==null&&preview?.get("canApply")==JsonPrimitive(true),onClick={confirm=true}){Text(if(undo)"确认撤销" else "确认搬入")}
            }}
        }
        if(receipt!=null)item{GlassRow{
            Text("原结果 · ${receipt!!.coreText("state")}",style=MaterialTheme.typography.titleMedium);Text(stringResource(R.string.rovai_fusion_475ce8a6b14f7f94))
            receipt!!["files"].coreArray().forEach{file->Text("${file.coreText("targetId")} · ${file.coreText("state")}")}
            TextButton(enabled=!disabled&&last!=null,onClick={work{readReceipt(last!!)}}){Text(stringResource(R.string.rovai_fusion_66f98bd9b1149ef5))}
            if(last!=null&&receipt!!.coreText("state")=="applied"&&receipt!!["files"].coreArray().any{it.coreText("state")=="pending"})TextButton(enabled=!disabled&&pending==null,onClick={work{model.privateConfiguration("files-finish",buildJsonObject{put("commandId",last!!["commandId"]!!)});current();readReceipt(last!!)}}){Text(stringResource(R.string.rovai_fusion_e26fac1811e18695))}
            TextButton(
                enabled = !disabled && pending == null && receipt!!.coreText("state") == "applied"
                    && last?.coreText("operation") == "configuration.migration.confirm"
                    && receipt!!["files"].coreArray().all { it.coreText("state") == "applied" },
                onClick = {
                    work {
                        val value = model.query(
                            RovaiOperation.CONFIGURATION_MIGRATION_UNDOPREVIEW,
                            buildJsonObject {
                                put("schemaVersion", 1)
                                put("originalCommandId", last!!["commandId"]!!)
                            }
                        ).coreObject()
                        current()
                        preview = value
                        rows = value["rows"].coreArray()
                        undo = true
                        visible = 50
                        if (archive == null) {
                            archive = buildJsonObject {
                                put("archiveId", value["archiveId"] ?: JsonNull)
                                put("sourceDigest", value["sourceDigest"] ?: JsonNull)
                            }
                        }
                    }
                }
            ) { Text(stringResource(R.string.rovai_fusion_c7258de16fde624e)) }
        }}
        if(issue!=null)item{Text(issue!!,color=MaterialTheme.colorScheme.error)}
        if(busy)item{LinearProgressIndicator(Modifier.fillMaxWidth())}
    }
    if(confirm)AlertDialog(onDismissRequest={if(!busy)confirm=false},title={Text(if(undo)"撤销这次配置导入？" else "搬入已预览的配置？")},text={Text(stringResource(R.string.rovai_fusion_8d099a6c6e9675a5))},dismissButton={TextButton(enabled=!busy,onClick={confirm=false}){Text(stringResource(R.string.rovai_fusion_c568c3012a5e0f77))}},confirmButton={TextButton(enabled=!disabled&&pending==null&&preview?.get("canApply")==JsonPrimitive(true),onClick={confirm=false;work{
        val frozen=preview?:error("请重新预览");val operation=if(undo)"configuration.migration.undoConfirm" else "configuration.migration.confirm";val command=buildJsonObject{put("operation",operation);put("commandId",UUID.randomUUID().toString());put("previewId",frozen["previewId"]!!);put("previewDigest",frozen["previewDigest"]!!)};remember(command)
        val value=model.query(if(undo)RovaiOperation.CONFIGURATION_MIGRATION_UNDOCONFIRM else RovaiOperation.CONFIGURATION_MIGRATION_CONFIRM,buildJsonObject{put("schemaVersion",1);put("commandId",command["commandId"]!!);put("previewId",frozen["previewId"]!!);put("previewDigest",frozen["previewDigest"]!!)}).coreObject();current();require(value["commandId"]==command["commandId"]){"原回执编号不同"};readReceipt(command)
    }}){Text(stringResource(R.string.rovai_fusion_36f33adaf0942634))}})
}

@Composable
private fun PortableMigrationChoice(label:String,values:List<JsonObject>,selected:String,enabled:Boolean,onSelect:(JsonObject)->Unit){
    var expanded by remember{mutableStateOf(false)}
    Box{OutlinedButton(enabled=enabled,onClick={expanded=true}){Text(values.find{it.coreText("id")==selected}?.coreText("name")?:label)};DropdownMenu(expanded=expanded,onDismissRequest={expanded=false}){values.forEach{value->DropdownMenuItem(text={Text(value.coreText("name"))},onClick={expanded=false;onSelect(value)})}}}
}
