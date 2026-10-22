package app.codeg.android.feature.main

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.security.MessageDigest
import java.util.UUID

data class RovaiConfigurationSource(val file:File,val displayName:String,val sourceDigest:String,val byteSize:Long)

/** 用户所选文件有界复制到应用私有缓存，口令不写缓存，页面释放后删除本次源副本。 */
suspend fun prepareRovaiConfiguration(context:Context,uri:Uri):RovaiConfigurationSource=withContext(Dispatchers.IO){
    val directory=File(context.cacheDir,"portable-configuration-source").apply{mkdirs()}.canonicalFile
    require(directory.parentFile==context.cacheDir.canonicalFile){"配置缓存目录无效"}
    val file=File(directory,"source-${UUID.randomUUID()}.bin")
    var name="配置文件"
    context.contentResolver.query(uri,arrayOf(OpenableColumns.DISPLAY_NAME),null,null,null)?.use{cursor->if(cursor.moveToFirst())name=cursor.getString(0)?.take(512)?:name}
    val hash=MessageDigest.getInstance("SHA-256");var bytes=0L
    try{
        val input=context.contentResolver.openInputStream(uri)?:error("无法读取配置文件")
        input.use{source->file.outputStream().use{target->val buffer=ByteArray(64*1024);while(true){val count=source.read(buffer);if(count<0)break;bytes+=count;require(bytes<=64L*1024*1024){"配置文件超过64MB"};hash.update(buffer,0,count);target.write(buffer,0,count)}}}
        file.setReadable(false,false);file.setWritable(false,false);file.setReadable(true,true);file.setWritable(true,true)
        RovaiConfigurationSource(file,name,"sha256:"+hash.digest().joinToString(""){"%02x".format(it.toInt() and 255)},bytes)
    }catch(error:Exception){file.delete();throw error}
}

suspend fun releaseRovaiConfiguration(context:Context,value:RovaiConfigurationSource)=withContext(Dispatchers.IO){
    val directory=File(context.cacheDir,"portable-configuration-source").canonicalFile
    if(directory.parentFile==context.cacheDir.canonicalFile&&value.file.canonicalFile.parentFile==directory&&value.file.name.startsWith("source-"))value.file.delete()
    Unit
}
