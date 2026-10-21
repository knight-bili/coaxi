package app.codeg.android.feature.main

import android.graphics.BitmapFactory
import android.util.Base64
import androidx.compose.foundation.Image
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*

/** Same last-public-message / terminal-only fallback rule as Rovai ThreadWorkspace. */
fun rovaiImageGroups(thread:JsonObject,message:JsonObject):List<JsonObject>{
    if(message.coreText("authorType")!="agent"||message["withdrawn"]==JsonPrimitive(true))return emptyList()
    val id=message.coreText("sourceAgentRunId");if(id.isEmpty())return emptyList()
    val last=thread["messages"].coreArray().lastOrNull{it.coreText("authorType")=="agent"&&it.coreText("sourceAgentRunId")==id&&it["withdrawn"]!=JsonPrimitive(true)}
    if(last?.coreText("id")!=message.coreText("id"))return emptyList()
    return thread["agentRunImages"].coreArray().filter{it.coreText("agentRunId")==id}
}
fun rovaiOrphanImages(thread:JsonObject):List<JsonObject>{val public=thread["messages"].coreArray().filter{it.coreText("authorType")=="agent"&&it["withdrawn"]!=JsonPrimitive(true)}.map{it.coreText("sourceAgentRunId")}.toSet();return thread["agentRunImages"].coreArray().filter{group->group.coreText("agentRunId") !in public&&thread["agentRuns"].coreArray().any{it.coreText("id")==group.coreText("agentRunId")&&it.coreText("status") in listOf("completed","failed","cancelled","interrupted","succeeded")}}}
@Composable
fun RovaiRuntimeImageGallery(model:RovaiWorkspaceViewModel,threadId:String,groups:List<JsonObject>){
    Column(verticalArrangement=Arrangement.spacedBy(8.dp)){groups.forEach{group->group["images"].coreArray().forEach{image->key(group.coreText("agentRunId"),group.coreLong("executionEpoch"),image.coreText("id")){RovaiRuntimeImage(model,threadId,image)}}}}
}
@Composable
private fun RovaiRuntimeImage(model:RovaiWorkspaceViewModel,threadId:String,image:JsonObject){
    var bitmap by remember(threadId,image.coreText("id")){mutableStateOf<ImageBitmap?>(null)};var issue by remember{mutableStateOf<String?>(null)};var large by remember{mutableStateOf(false)};var retry by remember{mutableIntStateOf(0)}
    LaunchedEffect(threadId,image.coreText("id"),retry){val generation=model.connectionGeneration;try{require(image.coreLong("byteSize")<=20*1024*1024){"图片超过20MB，请在工程中查看"};val content=model.query(RovaiOperation.AGENTRUNIMAGES_READ,buildJsonObject{put("threadId",threadId);put("imageId",image.coreText("id"))}).coreObject();require(content.coreText("mediaType").startsWith("image/")&&content.coreText("data").length<=28*1024*1024){"图片格式或大小无法读取"};val decoded=withContext(Dispatchers.Default){val bytes=Base64.decode(content.coreText("data"),Base64.DEFAULT);val bounds=BitmapFactory.Options().apply{inJustDecodeBounds=true};BitmapFactory.decodeByteArray(bytes,0,bytes.size,bounds);require(bounds.outWidth in 1..100000&&bounds.outHeight in 1..100000){"图片尺寸无法读取"};val options=BitmapFactory.Options().apply{inSampleSize=1};while(bounds.outWidth/options.inSampleSize>2048||bounds.outHeight/options.inSampleSize>2048)options.inSampleSize*=2;BitmapFactory.decodeByteArray(bytes,0,bytes.size,options)?.asImageBitmap()?:error("图片无法解码")};if(generation==model.connectionGeneration){bitmap=decoded;issue=null}}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(generation==model.connectionGeneration)issue=error.message}}
    if(bitmap!=null)Image(bitmap!!,contentDescription="队员生成的图片",contentScale=ContentScale.Fit,modifier=Modifier.fillMaxWidth().heightIn(max=320.dp).clip(RoundedCornerShape(10.dp)).clickable{large=true})else if(issue!=null)TextButton(onClick={retry++}){Text("图片暂不可读，重试",color=CodegTheme.colors.textSecondary)}else LinearProgressIndicator(Modifier.fillMaxWidth())
    if(large&&bitmap!=null)Dialog(onDismissRequest={large=false},properties=DialogProperties(usePlatformDefaultWidth=false)){Surface(Modifier.fillMaxSize(),color=CodegTheme.colors.bgElevated){Column(Modifier.padding(16.dp)){TextButton(onClick={large=false}){Text("关闭")};Image(bitmap!!,contentDescription="队员生成的图片",contentScale=ContentScale.Fit,modifier=Modifier.fillMaxWidth().weight(1f))}}}
}
