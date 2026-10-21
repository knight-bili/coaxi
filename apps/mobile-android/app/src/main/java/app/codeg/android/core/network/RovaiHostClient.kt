package app.codeg.android.core.network

import io.ktor.client.engine.okhttp.OkHttp
import android.os.SystemClock
import io.ktor.utils.io.readUTF8Line
import io.ktor.client.HttpClient
import io.ktor.client.request.*
import io.ktor.client.statement.*
import io.ktor.http.*
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.*
import java.net.URI
import java.util.UUID
import java.util.concurrent.atomic.AtomicLong

@Serializable
data class RovaiHostSession(val token:String,val clientId:String,val editorProof:String,val ownerId:String,val epoch:String,val expiresAt:Long=0)
class RovaiHostFailure(val code:String,val unknownResult:Boolean=false):Exception(code)

/** One Host/editor lifetime. Writes are never repeated after transport failure. */
class RovaiHostClient(rawBaseUrl:String,private val administratorToken:String,
    private val loadSession:suspend ()->String?={null},private val saveSession:suspend (String?)->Unit={}) {
    private val json=Json{ignoreUnknownKeys=true;encodeDefaults=true}
    private val http=HttpClient(OkHttp){expectSuccess=false;followRedirects=false;engine{config{retryOnConnectionFailure(false);followRedirects(false);followSslRedirects(false);connectTimeout(java.time.Duration.ofSeconds(30));readTimeout(java.time.Duration.ofSeconds(90));writeTimeout(java.time.Duration.ofSeconds(120))}}}
    private val mutex=Mutex()
    private val lifetime=AtomicLong(0)
    @Volatile private var session:RovaiHostSession?=null
    private var loaded=false
    private var denied=false
    private var deadline=0L
    val baseUrl:String
    init {val uri=URI(rawBaseUrl.trim());require(uri.scheme in listOf("http","https")&&!uri.host.isNullOrBlank()&&uri.rawUserInfo==null&&uri.rawQuery==null&&uri.rawFragment==null){"host.invalid_url"};baseUrl=uri.toString().trimEnd('/')}
    private suspend fun post(path:String,body:JsonElement,bearer:String?=null):JsonElement {
        val response=http.post("$baseUrl/api/v1/$path") {contentType(ContentType.Application.Json);bearer?.let{header(HttpHeaders.Authorization,"Bearer $it")};setBody(body.toString())}
        if(response.status.value !in 200..299)throw RovaiHostFailure(if(response.status.value==401)"host.unauthorized" else "host.http_${response.status.value}")
        val text=response.bodyAsText();if(text.length>32*1024*1024)throw RovaiHostFailure("host.response_limit")
        return json.parseToJsonElement(text)
    }
    private fun deadline(value:JsonObject):Long {val ttl=value["expiresInSeconds"]?.jsonPrimitive?.longOrNull?:throw RovaiHostFailure("host.invalid_timing");return SystemClock.elapsedRealtime()+ttl.coerceIn(0,86400)*1000}
    suspend fun connect(authenticate:Boolean=false):RovaiHostSession=mutex.withLock {
        val generation=lifetime.get()
        if(!loaded){loaded=true;session=loadSession()?.let{runCatching{json.decodeFromString<RovaiHostSession>(it)}.getOrNull()}}
        if(denied&&!authenticate)throw RovaiHostFailure("host.login_required")
        val previous=session
        if(!authenticate){
            if(previous==null)throw RovaiHostFailure("host.login_required")
            if(deadline>SystemClock.elapsedRealtime()+60_000)return@withLock previous
            val resumed=post("session",buildJsonObject{putJsonObject("editor"){put("clientId",previous.clientId);put("proof",previous.editorProof)}},previous.token).jsonObject
            deadline=deadline(resumed)
            if(deadline<=SystemClock.elapsedRealtime()+60_000){val renewed=post("session/renew",buildJsonObject{},previous.token).jsonObject;deadline=deadline(renewed)}
            return@withLock previous
        }
        val editor=previous?.let{buildJsonObject{put("clientId",it.clientId);put("proof",it.editorProof)}}
        val value=post("login",buildJsonObject{put("protocolVersion",4);put("administratorToken",administratorToken);editor?.let{put("editor",it)}}).jsonObject
        fun required(key:String)=value[key]?.jsonPrimitive?.contentOrNull?:throw RovaiHostFailure("host.invalid_login")
        if(value["protocolVersion"]?.jsonPrimitive?.intOrNull!=4)throw RovaiHostFailure("host.protocol_incompatible")
        val next=RovaiHostSession(required("token"),required("clientId"),required("editorProof"),required("ownerId"),required("epoch"),value["expiresAt"]?.jsonPrimitive?.longOrNull?:0)
        if(generation!=lifetime.get()){runCatching{post("logout",buildJsonObject{},next.token)};throw CancellationException("Host lifetime changed")}
        session=next;deadline=deadline(value);denied=false;saveSession(json.encodeToString(RovaiHostSession.serializer(),next));next
    }
    suspend fun request(operation:RovaiOperation,params:JsonElement=buildJsonObject{}):JsonElement {
        val current=connect()
        val response=try{post("request",buildJsonObject{put("operation",operation.wire);put("params",params)},current.token)}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(error is RovaiHostFailure&&error.code=="host.unauthorized"){denied=true;deadline=0};throw RovaiHostFailure(error.message?:"host.transport_failed",unknownResult=true)}
        val envelope=response.jsonObject
        envelope["error"]?.takeIf{it !is JsonNull}?.let{throw RovaiHostFailure(it.toString())}
        return envelope["result"]?:JsonNull
    }
    suspend fun command(operation:RovaiOperation,command:JsonObject,commandId:String=UUID.randomUUID().toString()):JsonElement=
        request(operation,buildJsonObject{put("commandId",commandId);put("command",command)})
    suspend fun reconcile(operation:RovaiOperation,params:JsonElement):JsonElement=request(RovaiOperation.COMMANDS_RECONCILE,buildJsonObject{put("operation",operation.wire);put("params",params)})
    /** Invalidation only; every reconnect requires fresh authorized snapshots. */
    fun invalidations():Flow<JsonObject> = flow {
        val current=connect()
        http.prepareGet("$baseUrl/api/v1/events"){header(HttpHeaders.Authorization,"Bearer ${current.token}")}.execute {response->
            if(response.status.value!=200)throw RovaiHostFailure("host.events_denied")
            val channel=response.bodyAsChannel();var event="";val data=StringBuilder()
            while(!channel.isClosedForRead){val line=channel.readUTF8Line(1024*1024)?:break
                if(line.isEmpty()){if(event in listOf("resync","invalidate")&&data.isNotEmpty()){emit(buildJsonObject{put("event",event);put("data",json.parseToJsonElement(data.toString()))})};event="";data.clear()}
                else if(line.startsWith("event:"))event=line.substringAfter(':').trim()
                else if(line.startsWith("data:")){if(data.isNotEmpty())data.append('\n');data.append(line.substringAfter(':').trimStart());if(data.length>1024*1024)throw RovaiHostFailure("host.event_limit")}
            }
        }
    }
    suspend fun renew(){val current=session?:return;val value=post("session/renew",buildJsonObject{},current.token).jsonObject;val expiry=value["expiresAt"]?.jsonPrimitive?.longOrNull?:throw RovaiHostFailure("host.invalid_renewal");deadline=deadline(value);session=current.copy(expiresAt=expiry);saveSession(json.encodeToString(RovaiHostSession.serializer(),session!!))}
    suspend fun logout(){lifetime.incrementAndGet();val current=session;session=null;denied=true;deadline=0;saveSession(null);if(current!=null)post("logout",buildJsonObject{},current.token)}
    fun close(){lifetime.incrementAndGet();session=null;http.close()}
}
