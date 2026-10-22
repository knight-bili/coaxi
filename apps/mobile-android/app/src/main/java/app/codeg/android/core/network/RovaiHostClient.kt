package app.codeg.android.core.network

import io.ktor.client.engine.okhttp.OkHttp
import android.os.SystemClock
import io.ktor.utils.io.readUTF8Line
import io.ktor.client.HttpClient
import io.ktor.client.request.*
import io.ktor.client.request.forms.*
import io.ktor.client.statement.*
import io.ktor.http.*
import io.ktor.http.content.OutgoingContent
import io.ktor.utils.io.ByteWriteChannel
import io.ktor.utils.io.writeFully
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
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
    suspend fun files(action:String,request:JsonObject):JsonObject {
        require(action in listOf("open","restore","reopen","readText","readPage","readHtml","resolveLine","reload","release")){"预览操作无效"}
        val current=connect();val generation=lifetime.get();val value=post("files",buildJsonObject{put("action",action);put("request",request)},current.token).jsonObject
        if(generation!=lifetime.get())throw CancellationException("Host lifetime changed")
        if(value["ok"]!=JsonPrimitive(true))throw RovaiHostFailure(value["error"]?.toString()?:"预览失败")
        return value["value"]?.jsonObject?:throw RovaiHostFailure("预览响应格式无效")
    }

    /** 密钥只经专用入口发送；不进入普通命令持久队列或错误正文。 */
    suspend fun privateConfiguration(action:String,input:JsonObject):JsonObject {
        require(action in listOf("provider","secrets","receipt")){"配置私有操作无效"}
        require(input.toString().toByteArray(Charsets.UTF_8).size<=2*1024*1024){"配置输入过大"}
        val current=connect();val generation=lifetime.get()
        val envelope=try{post("configuration/$action",input,current.token).jsonObject}
        catch(cancelled:CancellationException){throw cancelled}
        catch(error:Exception){if(error is RovaiHostFailure&&error.code=="host.unauthorized"){denied=true;deadline=0};throw RovaiHostFailure("configuration.private_result_unknown",unknownResult=true)}
        if(generation!=lifetime.get())throw CancellationException("Host lifetime changed")
        if(envelope["error"]?.let{it !is JsonNull}==true)throw RovaiHostFailure("configuration.private_input_rejected")
        return envelope["result"]?.jsonObject?:throw RovaiHostFailure("configuration.private_response_invalid")
    }

    suspend fun configurationArchive(metadata:JsonObject,source:File):JsonObject=withContext(Dispatchers.IO){
        require(source.isFile&&source.length()<=64L*1024*1024){"配置文件超过64MB或不可读取"}
        val size=source.length();val modified=source.lastModified();val hash=java.security.MessageDigest.getInstance("SHA-256")
        source.inputStream().use{input->val buffer=ByteArray(64*1024);var observed=0L;while(true){val count=input.read(buffer);if(count<0)break;observed+=count;require(observed<=64L*1024*1024){"配置文件过大"};hash.update(buffer,0,count)};require(observed==size){"配置文件已变化"}}
        val digest="sha256:"+hash.digest().joinToString(""){"%02x".format(it.toInt() and 255)}
        require(metadata["byteSize"]?.jsonPrimitive?.longOrNull==size&&metadata["sourceDigest"]?.jsonPrimitive?.contentOrNull==digest&&source.length()==size&&source.lastModified()==modified){"配置文件已变化"}
        val raw=metadata.toString().toByteArray(Charsets.UTF_8);require(raw.size<=128*1024){"配置说明过大"}
        val current=connect();val generation=lifetime.get();val boundary="RovaiConfig-"+UUID.randomUUID()
        val prefix=("--$boundary\r\nContent-Disposition: form-data; name=\"metadata\"\r\nContent-Type: application/json\r\n\r\n").toByteArray()+raw+("\r\n--$boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"configuration\"\r\nContent-Type: application/octet-stream\r\n\r\n").toByteArray()
        val suffix="\r\n--$boundary--\r\n".toByteArray()
        val response=try{http.post("$baseUrl/api/v1/configuration/archive"){
            header(HttpHeaders.Authorization,"Bearer ${current.token}");header(HttpHeaders.CacheControl,"no-store")
            setBody(object:OutgoingContent.WriteChannelContent(){
                override val contentType=ContentType.MultiPart.FormData.withParameter("boundary",boundary)
                override val contentLength:Long=prefix.size.toLong()+size+suffix.size
                override suspend fun writeTo(channel:ByteWriteChannel){
                    channel.writeFully(prefix);source.inputStream().use{input->val buffer=ByteArray(64*1024);var count=0L;while(true){val read=input.read(buffer);if(read<0)break;count+=read;if(count>size)throw RovaiHostFailure("configuration.source_changed");channel.writeFully(buffer,0,read)};if(count!=size)throw RovaiHostFailure("configuration.source_changed")};channel.writeFully(suffix)
                }
            })
        }}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){throw RovaiHostFailure("configuration.archive_result_unknown",unknownResult=true)}
        if(response.status.value !in 200..299){if(response.status.value==401){denied=true;deadline=0};throw RovaiHostFailure("configuration.archive_result_unknown",unknownResult=true)}
        val text=response.bodyAsText();if(text.length>2*1024*1024)throw RovaiHostFailure("configuration.archive_response_limit",unknownResult=true)
        if(generation!=lifetime.get())throw CancellationException("Host lifetime changed")
        val envelope=json.parseToJsonElement(text).jsonObject
        if(envelope["error"]?.let{it !is JsonNull}==true)throw RovaiHostFailure("configuration.archive_rejected")
        envelope["result"]?.jsonObject?:throw RovaiHostFailure("configuration.archive_response_invalid")
    }
    fun officePreviewUrl(port:Int,cap:String):String{require(port in 1..65535&&cap.length in 32..256&&cap.all{it.isLetterOrDigit()&&it.code<128||it=='_'||it=='-'}){"预览授权无效"};return "$baseUrl/api/office-watch-proxy/$port/?cap=$cap"}
    suspend fun fileBytes(action:String,request:JsonObject):RovaiFileBytes {
        require(action in listOf("readBinary","readChildImage","download")){"文件操作无效"}
        val current=connect();val generation=lifetime.get()
        return http.preparePost("$baseUrl/api/v1/files/bytes"){header(HttpHeaders.Authorization,"Bearer ${current.token}");contentType(ContentType.Application.Json);setBody(buildJsonObject{put("action",action);put("request",request)}.toString())}.execute {response->
            if(response.status.value !in 200..299){if(response.status.value==401){denied=true;deadline=0};throw RovaiHostFailure("文件读取被拒绝")}
            val channel=response.bodyAsChannel();val output=java.io.ByteArrayOutputStream();val chunk=ByteArray(65536)
            while(true){val count=channel.readAvailable(chunk,0,chunk.size);if(count<0)break;if(count==0)continue;require(output.size()+count<=20*1024*1024){"文件超过手机预览的20MB限制"};output.write(chunk,0,count)}
            if(generation!=lifetime.get())throw CancellationException("Host lifetime changed")
            val mime=response.headers[HttpHeaders.ContentType]?.substringBefore(';')?:"application/octet-stream"
            if(mime=="application/json")throw RovaiHostFailure("文件已变化或来源失效，请重新打开")
            val contentGeneration=response.headers["x-rovai-content-generation"]?:throw RovaiHostFailure("文件版本缺失")
            require(request["expectedGeneration"]?.jsonPrimitive?.content==contentGeneration){"文件已变化，请重新打开"}
            RovaiFileBytes(output.toByteArray(),mime,contentGeneration)
        }
    }
    suspend fun upload(intent:JsonObject,bytes:ByteArray):JsonObject {
        require(bytes.size<=20*1024*1024){"文件超过20MB"}
        val digest=java.security.MessageDigest.getInstance("SHA-256").digest(bytes).joinToString(""){"%02x".format(it)}
        require(intent["sha256"]?.jsonPrimitive?.content==digest&&intent["byteSize"]?.jsonPrimitive?.long==bytes.size.toLong()){ "上传文件已变化" }
        val current=connect()
        val response=http.post("$baseUrl/api/v1/uploads"){
            header(HttpHeaders.Authorization,"Bearer ${current.token}")
            setBody(MultiPartFormDataContent(formData{append("intent",intent.toString());append("file",bytes,Headers.build{append(HttpHeaders.ContentType,"application/octet-stream");append(HttpHeaders.ContentDisposition,"filename=\"source\"")})}))
        }
        if(response.status.value !in 200..299){if(response.status.value==401){denied=true;deadline=0};throw RovaiHostFailure("host.upload_http_${response.status.value}",unknownResult=true)}
        val text=response.bodyAsText();require(text.length<=32*1024*1024){"上传响应超过限制"};return json.parseToJsonElement(text).jsonObject
    }
    suspend fun uploadMissionBatch(intent:JsonObject,files:List<Pair<String,File>>):JsonObject {
        require(files.size<=10&&files.map{it.first}.toSet().size==files.size)
        val expected=intent["files"]!!.jsonArray;require(files.size==expected.size&&expected.all{entry->files.any{it.first==entry.jsonObject["id"]!!.jsonPrimitive.content}})
        val current=connect();val version=lifetime.get();val boundary="RovaiMission-"+UUID.randomUUID().toString()
        val initial=("--$boundary\r\nContent-Disposition: form-data; name=\"intent\"\r\nContent-Type: application/json\r\n\r\n"+intent.toString()+"\r\n").toByteArray(Charsets.UTF_8)
        val headers=files.map{(id,_)->("--$boundary\r\nContent-Disposition: form-data; name=\"file:$id\"; filename=\"source\"\r\nContent-Type: application/octet-stream\r\n\r\n").toByteArray(Charsets.UTF_8)}
        val closing="--$boundary--\r\n".toByteArray(Charsets.UTF_8)
        val response=http.post("$baseUrl/api/v1/mission-files"){
            header(HttpHeaders.Authorization,"Bearer ${current.token}")
            setBody(object:OutgoingContent.WriteChannelContent(){
                override val contentType=ContentType.MultiPart.FormData.withParameter("boundary",boundary)
                override val contentLength=initial.size.toLong()+closing.size+files.mapIndexed{index,pair->headers[index].size+pair.second.length()+2}.sum()
                override suspend fun writeTo(channel:ByteWriteChannel){withContext(Dispatchers.IO){channel.writeFully(initial);files.forEachIndexed{index,(_,file)->channel.writeFully(headers[index]);file.inputStream().use{input->val buffer=ByteArray(65536);while(true){val count=input.read(buffer);if(count<0)break;channel.writeFully(buffer,0,count)}};channel.writeFully(byteArrayOf(13,10))};channel.writeFully(closing)}}
            })
        }
        if(response.status.value !in 200..299){if(response.status.value==401){denied=true;deadline=0};throw RovaiHostFailure("host.mission_upload_http_${response.status.value}",unknownResult=true)}
        val text=response.bodyAsText();require(version==lifetime.get()&&text.length<=1024*1024){"使命上传连接或结果已变化"};return json.parseToJsonElement(text).jsonObject
    }
    suspend fun uploadStatus(intent:JsonObject,action:String="reconcile"):JsonObject {
        require(action in listOf("reconcile","receipt","retry","finish")){"上传操作无效"}
        val current=connect();return post("uploads/$action",intent,current.token).jsonObject
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

data class RovaiFileBytes(val bytes:ByteArray,val mime:String,val contentGeneration:String)
