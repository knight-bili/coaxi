package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.coaxi.android.core.designsystem.theme.CoaxiTheme
import app.coaxi.android.core.network.CoaxiOperation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import java.util.UUID

private val conversationFeatures=listOf("question" to "向你提问","sessionInfo" to "参考本次选中的旧讨论","automations" to "安排后续工作","workTasks" to "创建未启动使命")
private fun emptyConversationFlags()=buildJsonObject{conversationFeatures.forEach{put(it.first,false)}}

/** 复用原Coaxi移动端队员设置行；权限记录只保留在Core。 */
@Composable fun CoaxiConversationToolSettings(model:CoaxiWorkspaceViewModel,agentId:String){
    val coroutine=rememberCoroutineScope();val state by model.state.collectAsState();val generation=remember(agentId){model.connectionGeneration};val owner="conversation-tools-policy:$agentId"
    var policy by remember(agentId){mutableStateOf<JsonObject?>(null)};var pending by remember(agentId){mutableStateOf<JsonObject?>(null)};var busy by remember(agentId){mutableStateOf(false)};var issue by remember(agentId){mutableStateOf<String?>(null)};var expanded by remember(agentId){mutableStateOf(false)}
    suspend fun read(){policy=nativeUserCall(model,CoaxiOperation.CONVERSATIONTOOLS_POLICY_GET,buildJsonObject{})}
    fun work(action:suspend()->Unit){if(busy)return;busy=true;issue=null;coroutine.launch{try{action()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(generation==model.connectionGeneration)issue=error.message}finally{if(generation==model.connectionGeneration)busy=false}}}
    suspend fun accept(result:JsonObject,original:JsonObject){
        require(result.coreText("commandId")==original.coreText("commandId")&&result.coreText("commandType")=="conversationTools.policy.set"){"原设置回执归属不同"}
        require(model.clearBodyIfMatches(owner,original.toString(),generation)){"原请求记录已变化，请保留记录"};pending=null
        if(result.coreText("status")=="rejected"){read();error(result.coreText("code"))}
        policy=result["payload"].coreObject()
    }
    fun save(global:JsonObject?=null,memberFlags:JsonObject?=null,feedback:JsonElement?=null){
        val base=policy?:return;if(pending!=null)return
        val members=if(memberFlags==null)base["members"].coreArray()else base["members"].coreArray().filter{it.coreText("agentId")!=agentId}+buildJsonObject{put("agentId",agentId);put("flags",memberFlags)}
        val id=UUID.randomUUID().toString();val original=buildJsonObject{put("commandId",id);put("operation","conversationTools.policy.set")}
        val input=buildJsonObject{put("commandId",id);put("expectedRevision",base.coreLong("revision"));put("globalFlags",global?:base["globalFlags"]?:emptyConversationFlags());put("feedbackGate",feedback?:base["feedbackGate"]?:JsonNull);put("members",JsonArray(members))}
        work{model.saveBody(owner,original.toString());require(generation==model.connectionGeneration);pending=original;val value=nativeUserCall(model,CoaxiOperation.CONVERSATIONTOOLS_POLICY_SET,input);require(model.clearBodyIfMatches(owner,original.toString(),generation));pending=null;policy=value}
    }
    LaunchedEffect(agentId,generation){busy=true;try{val raw=model.readBody(owner);if(raw.isNotEmpty()){require(raw.length<=1024);val original=Json.parseToJsonElement(raw).coreObject();require(original.keys==setOf("commandId","operation")&&original.coreText("operation")=="conversationTools.policy.set");UUID.fromString(original.coreText("commandId"));pending=original};read()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    val global=policy?.get("globalFlags").coreObject();val flags=policy?.get("members").coreArray().firstOrNull{it.coreText("agentId")==agentId}?.get("flags").coreObject().ifEmpty{emptyConversationFlags()}
    val enabled=!busy&&policy!=null&&pending==null&&!state.busy&&state.pending==null&&generation==model.connectionGeneration
    Column(verticalArrangement=Arrangement.spacedBy(6.dp)){
        Text(stringResource(R.string.coaxi_fusion_cbe5cfa033e39df4),style=MaterialTheme.typography.titleSmall)
        TextButton(onClick={expanded=!expanded}){Text(if(expanded)"收起全局工具开关" else "全局工具开关")}
        if(expanded){conversationFeatures.forEach{(key,label)->Row{Text("允许分配：$label",Modifier.weight(1f));Switch(checked=global[key]==JsonPrimitive(true),enabled=enabled,onCheckedChange={checked->save(global=buildJsonObject{global.forEach{(name,value)->put(name,value)};put(key,checked)})})}};Text(stringResource(R.string.coaxi_fusion_df527163c81d0aa6),style=MaterialTheme.typography.labelMedium);listOf("沿用原反馈设置" to JsonNull,"新执行关闭反馈" to JsonPrimitive(false),"允许原已开启队员" to JsonPrimitive(true)).forEach{(label,value)->FilterChip(selected=(policy?.get("feedbackGate")?:JsonNull)==value,onClick={save(feedback=value)},enabled=enabled,label={Text(label)})}}
        conversationFeatures.forEach{(key,label)->Row{Column(Modifier.weight(1f)){Text(label);if(global[key]!=JsonPrimitive(true))Text(stringResource(R.string.coaxi_fusion_685678c3e11f69a3),style=MaterialTheme.typography.bodySmall,color=CoaxiTheme.colors.textSecondary)};Switch(checked=flags[key]==JsonPrimitive(true),enabled=enabled,onCheckedChange={checked->save(memberFlags=buildJsonObject{flags.forEach{(name,value)->put(name,value)};put(key,checked)})})}}
        Text(stringResource(R.string.coaxi_fusion_2f9bc0b7f206d70d),style=MaterialTheme.typography.bodySmall,color=CoaxiTheme.colors.textSecondary)
        pending?.let{original->TextButton(enabled=!busy&&generation==model.connectionGeneration,onClick={work{val value=nativeUserCall(model,CoaxiOperation.COMMANDS_RECONCILE,buildJsonObject{put("operation","conversationTools.policy.set");putJsonObject("params"){put("commandId",original.coreText("commandId"))}});require(value.coreText("state")=="recorded"){"原设置还没有确定回执，没有重复保存"};accept(value["result"].coreObject(),original)}}){Text(stringResource(R.string.coaxi_fusion_38ec197a5eb4bec8))}}
        issue?.let{Text(it,style=MaterialTheme.typography.bodySmall,color=CoaxiTheme.colors.danger)}
    }
}
