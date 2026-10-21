package app.codeg.android.feature.main

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.codeg.android.core.data.ServerRepository
import app.codeg.android.core.datastore.ServerProfile
import app.codeg.android.core.network.*
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import kotlinx.coroutines.sync.Mutex
import kotlinx.serialization.json.*
import java.util.UUID
import javax.inject.Inject

fun JsonElement?.coreObject():JsonObject=this as? JsonObject?:buildJsonObject{}
fun JsonElement?.coreArray():List<JsonObject> =(this as? JsonArray)?.mapNotNull{it as? JsonObject}?:emptyList()
fun JsonObject.coreText(key:String):String=(get(key) as? JsonPrimitive)?.contentOrNull?:""
fun JsonObject.coreLong(key:String):Long=(get(key) as? JsonPrimitive)?.longOrNull?:0
data class RovaiWorkspaceState(val connected:Boolean=false,val busy:Boolean=false,val error:String?=null,val navigation:JsonObject=buildJsonObject{},val members:List<JsonObject> =emptyList(),val missions:List<JsonObject> =emptyList(),val boards:List<JsonObject> =emptyList(),val selectedThread:String?=null,val thread:JsonObject?=null,val pending:JsonObject?=null,val lastResult:JsonElement?=null,val lastCompletedOperation:String?=null)

@HiltViewModel
class RovaiWorkspaceViewModel @Inject constructor(private val servers:ServerRepository):ViewModel(){
    private val mutable=MutableStateFlow(RovaiWorkspaceState());val state=mutable.asStateFlow()
    private var profile:ServerProfile?=null;private var client:RovaiHostClient?=null;private var lifetime=0L;private var events:Job?=null;private var active=true
    private val refreshGate=Mutex();private val writeGate=Mutex()
    fun bind(next:ServerProfile?){if(profile==next)return;events?.cancel();lifetime++;profile=next;client=null;mutable.value=RovaiWorkspaceState();if(next!=null)viewModelScope.launch{try{val generation=lifetime;val current=servers.rovaiClient(next)?:throw RovaiHostFailure("缺少服务器凭据");if(generation!=lifetime)return@launch;client=current;val pending=servers.loadRovaiPending(next);mutable.update{it.copy(pending=pending)};current.connect();if(generation==lifetime){mutable.update{it.copy(connected=true)};refresh();listen()}}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){mutable.update{it.copy(error=error.message)}}}}
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
    fun openThread(id:String?){mutable.update{it.copy(selectedThread=id,thread=null)};if(id!=null)refreshNow()}
    suspend fun query(operation:RovaiOperation,params:JsonElement=buildJsonObject{}):JsonElement=(client?:throw RovaiHostFailure("尚未连接")).request(operation,params)
    suspend fun readBody(owner:String):String {val selected=profile?:throw RovaiHostFailure("尚未连接");val generation=lifetime;val body=servers.readRovaiBody(selected,owner);if(generation!=lifetime)throw CancellationException("工作区已切换");return body}
    suspend fun saveBody(owner:String,body:String){val selected=profile?:throw RovaiHostFailure("尚未连接");val generation=lifetime;servers.saveRovaiBody(selected,owner,body);if(generation!=lifetime)throw CancellationException("工作区已切换")}
    fun execute(operation:RovaiOperation,params:JsonObject){if(state.value.pending!=null)return;val pending=buildJsonObject{put("operation",operation.wire);put("params",params)};submit(pending)}
    private fun submit(pending:JsonObject){viewModelScope.launch{if(!writeGate.tryLock())return@launch;val current=client;val selected=profile;val generation=lifetime;try{if(current==null||selected==null)return@launch;servers.saveRovaiPending(selected,pending);mutable.update{it.copy(pending=pending,busy=true,error=null)};val operation=RovaiOperation.entries.first{it.wire==pending.coreText("operation")};val result=current.request(operation,pending["params"]?:JsonNull);servers.saveRovaiPending(selected,null);if(generation!=lifetime)return@launch;mutable.update{it.copy(pending=null,lastResult=result,lastCompletedOperation=operation.wire)};val receipt=result.coreObject()["commandResult"].coreObject().ifEmpty{result.coreObject()};if(receipt.coreText("status")=="rejected")mutable.update{it.copy(error=receipt.coreText("code"))};refresh()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(generation==lifetime)mutable.update{it.copy(error=error.message)}}finally{if(generation==lifetime)mutable.update{it.copy(busy=false)};writeGate.unlock()}}}
    fun retry(){state.value.pending?.let(::submit)}
    fun reconcile(){val pending=state.value.pending?:return;viewModelScope.launch{val current=client?:return@launch;val selected=profile?:return@launch;val generation=lifetime;try{val operation=RovaiOperation.entries.first{it.wire==pending.coreText("operation")};val value=current.reconcile(operation,pending["params"]?:JsonNull).coreObject();if(generation!=lifetime)return@launch;if(value.coreText("state")=="recorded"){servers.saveRovaiPending(selected,null);mutable.update{it.copy(pending=null,lastResult=value["result"],lastCompletedOperation=operation.wire,error=null)};refresh()}else mutable.update{it.copy(error="原操作结果尚不明确，请稍后核对或重试原操作。")}}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(generation==lifetime)mutable.update{it.copy(error=error.message)}}}}
    fun send(text:String,recipients:Set<String>){val threadId=state.value.selectedThread?:return;if(text.isBlank())return;val params=buildJsonObject{put("commandId",UUID.randomUUID().toString());put("threadId",threadId);putJsonObject("content"){put("version",2);putJsonArray("segments"){recipients.forEach{id->add(buildJsonObject{put("kind","atom");putJsonObject("atom"){put("type","member");put("agentId",id)}})};add(buildJsonObject{put("kind","text");put("text",text)})}};put("sourceAttachments",JsonArray(emptyList()));put("quotes",JsonArray(emptyList()));put("replyToThreadMessageId",JsonNull);putJsonObject("execution"){put("taskId",JsonNull);put("purpose",text.trim());put("completionRole","required")}};execute(RovaiOperation.THREAD_MESSAGES_SEND,params)}
    fun command(operation:RovaiOperation,command:JsonObject){execute(operation,buildJsonObject{put("commandId",UUID.randomUUID().toString());put("command",command)})}
    fun logout(){viewModelScope.launch{events?.cancel();try{client?.logout();mutable.update{it.copy(connected=false)}}catch(error:Exception){mutable.update{it.copy(error=error.message)}}}}
    override fun onCleared(){events?.cancel();super.onCleared()}
}
