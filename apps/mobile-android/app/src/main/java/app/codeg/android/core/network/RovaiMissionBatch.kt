package app.codeg.android.core.network

import android.content.Context
import android.net.Uri
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.*
import java.io.File
import java.security.MessageDigest

/** Reuses private frozen-source storage, without creating a Thread/draft first. */
object RovaiMissionBatch {
    suspend fun prepare(context:Context,uri:Uri):JsonObject=RovaiUploadSource.prepare(context,uri,"",buildJsonObject{put("kind","mission_batch_local_only")},0)
    fun intent(operation:RovaiOperation,commandId:String,command:JsonObject,kept:List<String>,sources:List<JsonObject>):JsonObject {
        require(operation in listOf(RovaiOperation.MISSIONS_CREATE,RovaiOperation.MISSIONS_UPDATE)&&sources.size+kept.size<=10){"使命最多保留10份附件"}
        return buildJsonObject{put("operation",operation.wire);put("commandId",commandId);put("command",command);put("keepAttachmentIds",JsonArray(kept.map(::JsonPrimitive)));put("files",JsonArray(sources.map{source->val value=source["intent"]!!.jsonObject;buildJsonObject{put("id",value["commandId"]!!);put("displayName",value["displayName"]!!);put("byteSize",value["byteSize"]!!);put("sha256",value["sha256"]!!)}}))}
    }
    suspend fun files(context:Context,sources:List<JsonObject>):List<Pair<String,File>> =withContext(Dispatchers.IO){
        require(sources.size<=10);sources.map{source->val intent=source["intent"]!!.jsonObject;val file=RovaiUploadSource.frozenFile(context,source);require(file.length()==intent["byteSize"]!!.jsonPrimitive.long&&file.length()<=20L*1024*1024){"原使命附件大小已变化"};val hash=MessageDigest.getInstance("SHA-256");file.inputStream().use{stream->val buffer=ByteArray(65536);while(true){val size=stream.read(buffer);if(size<0)break;hash.update(buffer,0,size)}};require(hash.digest().joinToString(""){"%02x".format(it)}==intent["sha256"]!!.jsonPrimitive.content){"原使命附件内容已变化"};intent["commandId"]!!.jsonPrimitive.content to file}
    }
}
