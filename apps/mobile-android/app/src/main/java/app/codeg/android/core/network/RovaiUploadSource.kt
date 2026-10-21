package app.codeg.android.core.network

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.*
import java.io.File
import java.io.FileOutputStream
import java.nio.file.Files
import java.security.MessageDigest
import java.util.UUID

/** Frozen app-private bytes survive picker revocation and process death until a receipt settles. */
object RovaiUploadSource {
    private fun directory(context:Context)=File(context.filesDir,"rovai-uploads").apply{check(isDirectory||mkdirs()){"无法创建上传暂存目录"}}
    suspend fun prepare(context:Context,uri:Uri,threadId:String,target:JsonObject,revision:Long):JsonObject=withContext(Dispatchers.IO){
        require(uri.scheme=="content"){"请选择本机文件"};require(revision>=0){"附件草稿版本无效"}
        val root=directory(context);require((root.listFiles()?.sumOf{it.length()}?:0)<512L*1024*1024){"暂存上传较多，请先核对未完成的上传"}
        val id=UUID.randomUUID().toString();val file=File(root,"$id.bin");var name="附件"
        context.contentResolver.query(uri,arrayOf(OpenableColumns.DISPLAY_NAME),null,null,null)?.use{cursor->if(cursor.moveToFirst())name=cursor.getString(0)?:name}
        name=name.replace(Regex("[\\\\/\\p{Cntrl}]"),"_").take(240).ifBlank{"附件"}
        val hash=MessageDigest.getInstance("SHA-256");var size=0L
        try{check(file.createNewFile());FileOutputStream(file).use{output->context.contentResolver.openInputStream(uri)?.use{input->val buffer=ByteArray(65536);while(true){val count=input.read(buffer);if(count<0)break;size+=count;require(size<=20L*1024*1024){"文件超过20MB"};hash.update(buffer,0,count);output.write(buffer,0,count)}}?:throw IllegalStateException("无法读取所选文件");output.fd.sync()}
            buildJsonObject{put("sourceFile",file.absolutePath);put("attempted",false);putJsonObject("intent"){put("commandId",id);put("threadId",threadId);put("expectedRevision",revision);put("target",target);put("displayName",name);put("byteSize",size);put("sha256",hash.digest().joinToString(""){"%02x".format(it)})}}
        }catch(error:Exception){file.delete();throw error}
    }
    internal fun frozenFile(context:Context,request:JsonObject):File=file(context,request)
    private fun file(context:Context,request:JsonObject):File{val id=request["intent"]!!.jsonObject["commandId"]!!.jsonPrimitive.content;require(UUID.fromString(id).toString()==id);val expected=File(directory(context),"$id.bin");require(expected.absolutePath==request["sourceFile"]?.jsonPrimitive?.content&&!Files.isSymbolicLink(expected.toPath())&&expected.canonicalFile.parentFile==directory(context).canonicalFile){"上传来源记录无效"};return expected}
    suspend fun bytes(context:Context,request:JsonObject):ByteArray=withContext(Dispatchers.IO){val source=file(context,request);require(source.length()<=20L*1024*1024){"上传文件超过限制"};source.inputStream().use{input->input.readNBytes(20*1024*1024+1).also{require(it.size<=20*1024*1024)}}}
    suspend fun remove(context:Context,request:JsonObject)=withContext(Dispatchers.IO){file(context,request).delete();Unit}
}
