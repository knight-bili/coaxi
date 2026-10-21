package app.codeg.android.feature.main

import android.content.Context
import android.net.Uri
import dagger.hilt.android.qualifiers.ApplicationContext
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.codeg.android.core.data.ServerRepository
import app.codeg.android.core.datastore.ServerProfile
import app.codeg.android.core.network.*
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.sync.Mutex
import kotlinx.serialization.json.*
import java.util.UUID
import javax.inject.Inject

fun JsonElement?.coreObject():JsonObject=this as? JsonObject?:buildJsonObject{}
fun JsonElement?.coreArray():List<JsonObject> =(this as? JsonArray)?.mapNotNull{it as? JsonObject}?:emptyList()
fun JsonObject.coreText(key:String):String=(get(key) as? JsonPrimitive)?.contentOrNull?:""
fun JsonObject.coreLong(key:String):Long=(get(key) as? JsonPrimitive)?.longOrNull?:0
data class RovaiWorkspaceState(val connected:Boolean=false,val busy:Boolean=false,val error:String?=null,val navigation:JsonObject=buildJsonObject{},val members:List<JsonObject> =emptyList(),val missions:List<JsonObject> =emptyList(),val boards:List<JsonObject> =emptyList(),val selectedThread:String?=null,val thread:JsonObject?=null,val pending:JsonObject?=null,val lastResult:JsonElement?=null,val lastCompletedOperation:String?=null,val publicSources:List<JsonObject> =emptyList(),val publicSourcesReady:Boolean=false,val lastInputOwner:String?=null,val lastInputBody:String?=null)

@HiltViewModel
class RovaiWorkspaceViewModel @Inject constructor(private val servers:ServerRepository,@ApplicationContext private val appContext:Context):ViewModel(){
    private val mutable=MutableStateFlow(RovaiWorkspaceState());val state=mutable.asStateFlow()
    private var profile:ServerProfile?=null;private var client:RovaiHostClient?=null;private var lifetime=0L;private var events:Job?=null;private var active=true
    private val refreshGate=Mutex();private val writeGate=Mutex();private val bodyGate=Mutex();private val bodyCache=mutableMapOf<String,String>()
    fun bind(next:ServerProfile?){if(profile==next)return;events?.cancel();lifetime++;bodyCache.clear();profile=next;client=null;mutable.value=RovaiWorkspaceState();if(next!=null)viewModelScope.launch{try{val generation=lifetime;val current=servers.rovaiClient(next)?:throw RovaiHostFailure("缺少服务器凭据");if(generation!=lifetime)return@launch;client=current;val pending=servers.loadRovaiPending(next);mutable.update{it.copy(pending=pending)};current.connect();if(generation==lifetime){mutable.update{it.copy(connected=true)};refresh();listen()}}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){mutable.update{it.copy(error=error.message)}}}}
    fun authenticate(){viewModelScope.launch{try{val generation=lifetime;val current=client?:profile?.let{servers.rovaiClient(it)}?:return@launch;mutable.update{it.copy(busy=true,error=null)};current.connect(authenticate=true);if(generation!=lifetime)return@launch;client=current;mutable.update{it.copy(connected=true)};refresh();listen()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){mutable.update{it.copy(error=error.message,connected=false)}}finally{mutable.update{it.copy(busy=false)}}}}
    fun foreground(value:Boolean){active=value;if(!value){events?.cancel();return};if(state.value.connected){viewModelScope.launch{refresh();listen()}}}
    private fun listen(){events?.cancel();if(!active)return;val current=client?:return;val generation=lifetime;events=viewModelScope.launch{try{current.invalidations().conflate().collect{if(generation!=lifetime)return@collect;delay(250);refresh()}}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(generation==lifetime)mutable.update{it.copy(error=error.message)}}}}
    suspend fun refresh(){val current=client?:return;if(!refreshGate.tryLock())return;val generation=lifetime;try{
        coroutineScope{
            val navigation=async{current.request(RovaiOperation.NAVIGATION_SNAPSHOT).coreObject()};val members=async{current.request(RovaiOperation.MEMBERS_LIST).coreArray()};val missions=async{current.request(RovaiOperation.MISSIONS_LIST).coreArray()};val boards=async{current.request(RovaiOperation.CANVAS_BOARDS_LIST).coreObject()["boards"].coreArray()};val selected=state.value.selectedThread;val thread=async{selected?.let{current.request(RovaiOperation.THREADS_OPEN,buildJsonObject{put("threadId",it);put("traceId",UUID.randomUUID().toString())}).coreObject()}}
            val navValue=navigation.await();val memberValue=members.await();val missionValue=missions.await();val boardValue=boards.await();val threadValue=thread.await();if(generation==lifetime)mutable.update{it.copy(navigation=navValue,members=memberValue,missions=missionValue,boards=boardValue,thread=if(it.selectedThread==selected)threadValue else it.thread,error=null)}
        }
    }catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(generation==lifetime)mutable.update{it.copy(error=error.message)}}finally{refreshGate.unlock()}}
    fun refreshNow(){viewModelScope.launch{refresh()}}
    fun openThread(id:String?){mutable.update{it.copy(selectedThread=id,thread=null,publicSources=emptyList(),publicSourcesReady=false)};if(id!=null){viewModelScope.launch{try{val sources=readPublicSources(id);if(state.value.selectedThread==id)mutable.update{it.copy(publicSources=sources,publicSourcesReady=true)};refresh()}catch(error:Exception){mutable.update{it.copy(error=error.message)}}}}}
    private suspend fun readPublicSources(threadId:String,selected:ServerProfile?=profile):List<JsonObject>{val current=selected?:throw RovaiHostFailure("尚未连接");val raw=servers.readRovaiBody(current,"public-attachments:$threadId");return if(raw.isEmpty())emptyList()else Json.parseToJsonElement(raw).coreArray()}
    private suspend fun savePublicSources(threadId:String,sources:List<JsonObject>,selected:ServerProfile,generation:Long){require(sources.size<=32){"待发送附件超过32个，请先发送或移除"};servers.saveRovaiBody(selected,"public-attachments:$threadId",if(sources.isEmpty())""else JsonArray(sources).toString());if(generation==lifetime&&state.value.selectedThread==threadId)mutable.update{it.copy(publicSources=sources,publicSourcesReady=true)}}
    fun removePublicSource(id:String){if(state.value.pending!=null)return;val thread=state.value.selectedThread?:return;val selected=profile?:return;val generation=lifetime;viewModelScope.launch{try{bodyGate.withLock{savePublicSources(thread,readPublicSources(thread,selected).filter{it.coreText("id")!=id},selected,generation)}}catch(error:Exception){if(generation==lifetime)mutable.update{it.copy(error=error.message)}}}}
    private suspend fun settlePublicSend(pending:JsonObject,result:JsonElement,selected:ServerProfile,generation:Long){if(pending.coreText("operation")!=RovaiOperation.THREAD_MESSAGES_SEND.wire)return;val receipt=result.coreObject()["commandResult"].coreObject().ifEmpty{result.coreObject()};if(receipt.coreText("status")=="rejected")return;require(receipt.coreText("status") in listOf("applied","accepted")){"发送结果尚未确定"};val params=pending["params"].coreObject();val sent=params["sourceAttachments"].coreArray().map{it.coreText("id")}.toSet();val thread=params.coreText("threadId");bodyGate.withLock{savePublicSources(thread,readPublicSources(thread,selected).filter{it.coreText("id") !in sent},selected,generation)}}
    suspend fun query(operation:RovaiOperation,params:JsonElement=buildJsonObject{}):JsonElement=(client?:throw RovaiHostFailure("尚未连接")).request(operation,params)
    suspend fun files(action:String,params:JsonObject):JsonObject{val generation=lifetime;val result=(client?:throw RovaiHostFailure("尚未连接")).files(action,params);if(generation!=lifetime)throw CancellationException("工作区已切换");return result}
    suspend fun fileBytes(action:String,params:JsonObject):RovaiFileBytes{val generation=lifetime;val result=(client?:throw RovaiHostFailure("尚未连接")).fileBytes(action,params);if(generation!=lifetime)throw CancellationException("工作区已切换");return result}
    suspend fun readBody(owner:String):String {bodyCache[owner]?.let{return it};val selected=profile?:throw RovaiHostFailure("尚未连接");val generation=lifetime;val body=servers.readRovaiBody(selected,owner);if(generation!=lifetime)throw CancellationException("工作区已切换");return body}
    suspend fun saveBody(owner:String,body:String)=bodyGate.withLock{val selected=profile?:throw RovaiHostFailure("尚未连接");val generation=lifetime;servers.saveRovaiBody(selected,owner,body);if(generation==lifetime&&bodyCache[owner]==null)bodyCache[owner]=body;if(generation!=lifetime)throw CancellationException("工作区已切换")}
    fun rememberBody(owner:String,body:String){val selected=profile?:return;val generation=lifetime;bodyCache[owner]=body;viewModelScope.launch{try{bodyGate.withLock{servers.saveRovaiBody(selected,owner,body)}}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(generation==lifetime)mutable.update{it.copy(error=error.message)}}}}
    val connectionGeneration:Long get()=lifetime
    fun upload(uri:Uri,threadId:String,target:JsonObject,revision:Long,expectedHostGeneration:Long=lifetime){
        if(expectedHostGeneration!=lifetime){mutable.update{it.copy(error="工作区已切换，请重新选择附件")};return}
        if(state.value.pending!=null||state.value.busy)return
        viewModelScope.launch{val generation=lifetime;mutable.update{it.copy(busy=true,error=null)};try{val prepared=RovaiUploadSource.prepare(appContext,uri,threadId,target,revision);if(generation!=lifetime){RovaiUploadSource.remove(appContext,prepared);return@launch};submit(buildJsonObject{put("operation","__host.upload");put("params",prepared)})}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(generation==lifetime)mutable.update{it.copy(error=error.message)}}finally{if(generation==lifetime)mutable.update{it.copy(busy=false)}}}
    }
    private suspend fun completeUpload(pending:JsonObject,result:JsonObject,selected:ServerProfile,generation:Long){
        require(result["draft"]!=null&&result["draft"]!=JsonNull||result["receipt"]!=null){"上传结果尚未确定，请核对原上传"}
        val intent=pending["params"].coreObject()["intent"].coreObject();if(intent["target"].coreObject().coreText("kind")=="camp"&&result["draft"] is JsonObject){val thread=intent.coreText("threadId");val source=result["draft"]!!.jsonObject;require(source.coreText("id").isNotBlank()){ "上传来源回执不完整" };bodyGate.withLock{savePublicSources(thread,(readPublicSources(thread,selected)+source).distinctBy{it.coreText("id")},selected,generation)}}
        servers.saveRovaiPending(selected,null);runCatching{RovaiUploadSource.remove(appContext,pending["params"]!!.jsonObject)}
        if(generation!=lifetime)return
        mutable.update{it.copy(pending=null,lastResult=result,lastCompletedOperation="__host.upload",error=if(result["receipt"].coreObject().coreText("status")=="rejected")result["receipt"].coreObject().coreText("code")else null)};refresh()
    }
    private suspend fun dispatchUpload(pending:JsonObject,current:RovaiHostClient,selected:ServerProfile,generation:Long){
        val params=pending["params"]!!.jsonObject;val intent=params["intent"]!!.jsonObject
        if(params["attempted"]==JsonPrimitive(true)){
            val action=if(intent["target"].coreObject().coreText("kind")=="camp")"reconcile" else "retry"
            val status=current.uploadStatus(intent,action)
            if(status["draft"]!=null||status.coreText("state")=="recorded"){completeUpload(pending,status,selected,generation);return}
            require(status.coreText("state") in listOf("unknown","needs_source")){"原上传仍待核对"}
        }
        val marked=buildJsonObject{pending.forEach{(key,value)->put(key,value)};put("params",buildJsonObject{params.forEach{(key,value)->put(key,value)};put("attempted",true)})}
        servers.saveRovaiPending(selected,marked);if(generation==lifetime)mutable.update{it.copy(pending=marked)}
        val bytes=RovaiUploadSource.bytes(appContext,params);completeUpload(marked,current.upload(intent,bytes),selected,generation)
    }
    fun executePrivate(operation:RovaiOperation,command:JsonObject,owner:String,body:String,replace:Boolean=false){
        if(state.value.pending!=null)return
        submit(buildJsonObject{put("operation",operation.wire);putJsonObject("params"){put("commandId",UUID.randomUUID().toString());put("command",command)};putJsonObject("nativeBody"){put("owner",owner);put("expectedBody",body);put("replace",replace)}})
    }
    private suspend fun settlePrivate(pending:JsonObject,result:JsonElement,selected:ServerProfile,current:RovaiHostClient){
        val local=pending["nativeBody"] as? JsonObject?:return;val receipt=result.coreObject()["commandResult"].coreObject().ifEmpty{result.coreObject()};if(receipt.coreText("status")=="rejected")return
        require(receipt.coreText("status") in listOf("applied","accepted")){"私聊操作结果尚未确定"}
        val payload=receipt["payload"].coreObject();val replace=(local["replace"] as? JsonPrimitive)?.booleanOrNull==true
        if(replace&&payload.coreText("placement")=="retained")return
        var replacement=""
        if(replace){val draft=payload["draft"].coreObject();require(draft.coreLong("scopeVersion")==1L&&draft.coreText("scopeKey").isEmpty()&&draft["composerScope"]==JsonNull){"私聊材料归属发生变化"};replacement=(payload["body"] as? JsonPrimitive)?.contentOrNull?:throw IllegalStateException("私聊恢复正文缺失")
            val command=pending["params"].coreObject()["command"].coreObject();val operation=if(pending.coreText("operation").startsWith("delegations."))RovaiOperation.DELEGATIONS_CONVERSATION_GET else RovaiOperation.SINGLECHAT_GET
            val snapshot=current.request(operation,buildJsonObject{put("conversationId",command.coreText("conversationId"));put("composerScope",JsonNull)}).coreObject();require(snapshot["draft"].coreObject().coreLong("revision")==draft.coreLong("revision")){"材料版本已变化，请先核对原操作"}
        }
        bodyGate.withLock{val saved=servers.readRovaiBody(selected,local.coreText("owner"));if(!replace&&saved!=local.coreText("expectedBody"))return@withLock;require(saved==local.coreText("expectedBody")||saved==replacement){"输入内容已变化，已保留当前草稿和原请求"};servers.saveRovaiBody(selected,local.coreText("owner"),replacement);if(selected==profile&&current===client)bodyCache[local.coreText("owner")]=replacement}
    }
    fun execute(operation:RovaiOperation,params:JsonObject){if(state.value.pending!=null)return;val pending=buildJsonObject{put("operation",operation.wire);put("params",params)};submit(pending)}
    private fun submit(pending:JsonObject){viewModelScope.launch{if(!writeGate.tryLock())return@launch;val current=client;val selected=profile;val generation=lifetime;try{if(current==null||selected==null)return@launch;mutable.update{it.copy(pending=pending,busy=true,error=null)};servers.saveRovaiPending(selected,pending);if(pending.coreText("operation")=="__host.upload"){dispatchUpload(pending,current,selected,generation);return@launch};val operation=RovaiOperation.entries.first{it.wire==pending.coreText("operation")};val result=current.request(operation,pending["params"]?:JsonNull);settlePrivate(pending,result,selected,current);settlePublicSend(pending,result,selected,generation);servers.saveRovaiPending(selected,null);if(generation!=lifetime)return@launch;mutable.update{it.copy(pending=null,lastResult=result,lastCompletedOperation=operation.wire,lastInputOwner=pending["nativeBody"].coreObject().coreText("owner").takeIf{it.isNotEmpty()},lastInputBody=pending["nativeBody"].coreObject().coreText("expectedBody"))};val receipt=result.coreObject()["commandResult"].coreObject().ifEmpty{result.coreObject()};if(receipt.coreText("status")=="rejected")mutable.update{it.copy(error=receipt.coreText("code"))};refresh()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(generation==lifetime)mutable.update{it.copy(error=error.message)}}finally{if(generation==lifetime)mutable.update{it.copy(busy=false)};writeGate.unlock()}}}
    fun retry(){state.value.pending?.let(::submit)}
    fun reconcile(){val pending=state.value.pending?:return;viewModelScope.launch{val current=client?:return@launch;val selected=profile?:return@launch;val generation=lifetime;try{if(pending.coreText("operation")=="__host.upload"){val value=current.uploadStatus(pending["params"]!!.jsonObject["intent"]!!.jsonObject);completeUpload(pending,value,selected,generation);return@launch};val operation=RovaiOperation.entries.first{it.wire==pending.coreText("operation")};val value=current.reconcile(operation,pending["params"]?:JsonNull).coreObject();if(generation!=lifetime)return@launch;if(value.coreText("state")=="recorded"){settlePrivate(pending,value["result"]?:JsonNull,selected,current);settlePublicSend(pending,value["result"]?:JsonNull,selected,generation);servers.saveRovaiPending(selected,null);mutable.update{it.copy(pending=null,lastResult=value["result"],lastCompletedOperation=operation.wire,lastInputOwner=pending["nativeBody"].coreObject().coreText("owner").takeIf{it.isNotEmpty()},lastInputBody=pending["nativeBody"].coreObject().coreText("expectedBody"),error=null)};refresh()}else mutable.update{it.copy(error="原操作结果尚不明确，请稍后核对或重试原操作。")}}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(generation==lifetime)mutable.update{it.copy(error=error.message)}}}}
    fun publicBody(text:String,recipients:Set<String>,revision:Long):String=buildJsonObject{put("version",1);put("text",text);put("recipients",JsonArray(recipients.map(::JsonPrimitive)));put("revision",revision)}.toString()
    fun send(text:String,recipients:Set<String>,document:String,expectedThreadId:String){
        val threadId=state.value.selectedThread?:return;if(threadId!=expectedThreadId||text.isBlank()||state.value.pending!=null||state.value.busy)return
        val body=Json.parseToJsonElement(document).coreObject();require(body.coreText("text")==text&&(body["recipients"] as? JsonArray)?.mapNotNull{(it as? JsonPrimitive)?.contentOrNull}?.toSet()==recipients){"输入草稿发生变化"}
        val params=buildJsonObject{put("commandId",UUID.randomUUID().toString());put("threadId",threadId);putJsonObject("content"){put("version",2);putJsonArray("segments"){recipients.forEach{id->add(buildJsonObject{put("kind","atom");putJsonObject("atom"){put("type","member");put("agentId",id)}})};add(buildJsonObject{put("kind","text");put("text",text)})}};put("sourceAttachments",JsonArray(state.value.publicSources));put("quotes",JsonArray(emptyList()));put("replyToThreadMessageId",JsonNull);putJsonObject("execution"){put("taskId",JsonNull);put("purpose",text.trim());put("completionRole","required")}}
        submit(buildJsonObject{put("operation",RovaiOperation.THREAD_MESSAGES_SEND.wire);put("params",params);putJsonObject("nativeBody"){put("owner","public:$threadId");put("expectedBody",document);put("replace",false)}})
    }

    fun command(operation:RovaiOperation,command:JsonObject){execute(operation,buildJsonObject{put("commandId",UUID.randomUUID().toString());put("command",command)})}
    fun logout(){viewModelScope.launch{events?.cancel();try{client?.logout();mutable.update{it.copy(connected=false)}}catch(error:Exception){mutable.update{it.copy(error=error.message)}}}}
    override fun onCleared(){events?.cancel();super.onCleared()}
}
