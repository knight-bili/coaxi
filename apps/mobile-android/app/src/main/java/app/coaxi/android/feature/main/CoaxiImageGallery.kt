package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

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
import app.coaxi.android.core.designsystem.theme.CoaxiTheme
import app.coaxi.android.core.designsystem.markdown.MarkdownContent
import app.coaxi.android.core.network.CoaxiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*

/** Same last-public-message / terminal-only fallback rule as Coaxi ThreadWorkspace. */
fun coaxiImageGroups(thread:JsonObject,message:JsonObject):List<JsonObject>{
    if(message.coreText("authorType")!="agent"||message["withdrawn"]==JsonPrimitive(true))return emptyList()
    val id=message.coreText("sourceAgentRunId");if(id.isEmpty())return emptyList()
    val last=thread["messages"].coreArray().lastOrNull{it.coreText("authorType")=="agent"&&it.coreText("sourceAgentRunId")==id&&it["withdrawn"]!=JsonPrimitive(true)}
    if(last?.coreText("id")!=message.coreText("id"))return emptyList()
    return thread["agentRunImages"].coreArray().filter{it.coreText("agentRunId")==id}
}
fun coaxiOrphanImages(thread:JsonObject):List<JsonObject>{val public=thread["messages"].coreArray().filter{it.coreText("authorType")=="agent"&&it["withdrawn"]!=JsonPrimitive(true)}.map{it.coreText("sourceAgentRunId")}.toSet();return thread["agentRunImages"].coreArray().filter{group->group.coreText("agentRunId") !in public&&thread["agentRuns"].coreArray().any{it.coreText("id")==group.coreText("agentRunId")&&it.coreText("status") in listOf("completed","failed","cancelled","interrupted","succeeded")}}}
@Composable
fun CoaxiRuntimeImageGallery(model:CoaxiWorkspaceViewModel,threadId:String,groups:List<JsonObject>){
    Column(verticalArrangement=Arrangement.spacedBy(8.dp)){groups.forEach{group->group["images"].coreArray().forEach{image->key(group.coreText("agentRunId"),group.coreLong("executionEpoch"),image.coreText("id")){CoaxiRuntimeImage(model,threadId,image)}}}}
}
@Composable
private fun CoaxiRuntimeImage(model:CoaxiWorkspaceViewModel,threadId:String,image:JsonObject){
    var bitmap by remember(threadId,image.coreText("id")){mutableStateOf<ImageBitmap?>(null)};var issue by remember{mutableStateOf<String?>(null)};var large by remember{mutableStateOf(false)};var retry by remember{mutableIntStateOf(0)}
    LaunchedEffect(threadId,image.coreText("id"),retry){val generation=model.connectionGeneration;try{require(image.coreLong("byteSize")<=20*1024*1024){"图片超过20MB，请在工程中查看"};val content=model.query(CoaxiOperation.AGENTRUNIMAGES_READ,buildJsonObject{put("threadId",threadId);put("imageId",image.coreText("id"))}).coreObject();require(content.coreText("mediaType").startsWith("image/")&&content.coreText("data").length<=28*1024*1024){"图片格式或大小无法读取"};val decoded=withContext(Dispatchers.Default){val bytes=Base64.decode(content.coreText("data"),Base64.DEFAULT);val bounds=BitmapFactory.Options().apply{inJustDecodeBounds=true};BitmapFactory.decodeByteArray(bytes,0,bytes.size,bounds);require(bounds.outWidth in 1..100000&&bounds.outHeight in 1..100000){"图片尺寸无法读取"};val options=BitmapFactory.Options().apply{inSampleSize=1};while(bounds.outWidth/options.inSampleSize>2048||bounds.outHeight/options.inSampleSize>2048)options.inSampleSize*=2;BitmapFactory.decodeByteArray(bytes,0,bytes.size,options)?.asImageBitmap()?:error("图片无法解码")};if(generation==model.connectionGeneration){bitmap=decoded;issue=null}}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(generation==model.connectionGeneration)issue=error.message}}
    if(bitmap!=null)Image(bitmap!!,contentDescription="队员生成的图片",contentScale=ContentScale.Fit,modifier=Modifier.fillMaxWidth().heightIn(max=320.dp).clip(RoundedCornerShape(10.dp)).clickable{large=true})else if(issue!=null)TextButton(onClick={retry++}){Text(stringResource(R.string.coaxi_fusion_d4be33bd431387c9),color=CoaxiTheme.colors.textSecondary)}else LinearProgressIndicator(Modifier.fillMaxWidth())
    if(large&&bitmap!=null)Dialog(onDismissRequest={large=false},properties=DialogProperties(usePlatformDefaultWidth=false)){Surface(Modifier.fillMaxSize(),color=CoaxiTheme.colors.bgElevated){Column(Modifier.padding(16.dp)){TextButton(onClick={large=false}){Text(stringResource(R.string.coaxi_fusion_3fd47edce45b3603))};Image(bitmap!!,contentDescription="队员生成的图片",contentScale=ContentScale.Fit,modifier=Modifier.fillMaxWidth().weight(1f))}}}
}

fun coaxiAttachmentSource(threadId:String,message:JsonObject,attachment:JsonObject,conversationId:String?,child:Boolean):JsonObject=buildJsonObject{put("kind","attachment");put("threadId",threadId);putJsonObject("locator"){put("threadId",threadId);put("attachmentRefId",attachment.coreText("id"));put("owner",if(conversationId==null)"message" else if(child)"delegation_child_message" else "single_chat_message");if(conversationId==null)put("messageId",message.coreText("id"))else{put("conversationId",conversationId);put("conversationMessageId",message.coreText("id"))}}}
@Composable
fun CoaxiMessageBody(model:CoaxiWorkspaceViewModel,threadId:String,message:JsonObject,conversationId:String?=null,child:Boolean=false){
    val user=message.coreText("authorType")=="user"
    var card by remember(message.coreText("id")){mutableStateOf(false)}
    @Composable fun pictures(){message["attachments"].coreArray().filter{it.coreText("previewKind")=="image"}.forEach{attachment->key(attachment.coreText("id")){CoaxiAttachmentImage(model,coaxiAttachmentSource(threadId,message,attachment,conversationId,child),attachment,user)}}}
    if(user)pictures()
    if(user)CoaxiMessageArtifacts(model,threadId,message,conversationId,child)
    MarkdownContent(if(message["withdrawn"]==JsonPrimitive(true))"消息已撤回" else message.coreText("body"))
    if(message.coreText("authorType")=="agent"&&message["withdrawn"]!=JsonPrimitive(true)&&Regex("(?m)(`{3,}|~{3,})spec(?:[ \\t].*)?$").containsMatchIn(message.coreText("body")))TextButton(onClick={card=true}){Text(stringResource(R.string.coaxi_fusion_0e0b247611d37eec))}
    if(card)CoaxiGeneratedCard(model,threadId,message,conversationId,child){card=false}
    if(!user){CoaxiMessageArtifacts(model,threadId,message,conversationId,child);pictures()}
}
@Composable
private fun CoaxiAttachmentImage(model:CoaxiWorkspaceViewModel,source:JsonObject,attachment:JsonObject,user:Boolean){
    var bitmap by remember(source){mutableStateOf<ImageBitmap?>(null)};var issue by remember(source){mutableStateOf<String?>(null)};var retry by remember{mutableIntStateOf(0)};var expanded by remember{mutableStateOf(false)}
    LaunchedEffect(source,retry){val generation=model.connectionGeneration;var handle:String?=null;try{require(attachment.coreText("availability") !in listOf("missing","unreadable","kind_changed")){"图片来源不可用"};val opened=model.files("open",source);val selected=opened["file"].coreObject();handle=selected.coreText("handleId").takeIf{it.isNotEmpty()};require(opened.coreText("kind")=="file_preview"&&selected.coreText("kind")=="image"&&selected.coreLong("size")<=20*1024*1024){"图片无法预览"};val data=model.fileBytes("readBinary",buildJsonObject{put("handleId",handle);put("expectedGeneration",selected.coreText("contentGeneration"))});val decoded=withContext(Dispatchers.Default){val bytes=data.bytes;val bounds=BitmapFactory.Options().apply{inJustDecodeBounds=true};BitmapFactory.decodeByteArray(bytes,0,bytes.size,bounds);require(bounds.outWidth in 1..100000&&bounds.outHeight in 1..100000){"图片尺寸无法读取"};val options=BitmapFactory.Options().apply{inSampleSize=1};while(bounds.outWidth/options.inSampleSize>2048||bounds.outHeight/options.inSampleSize>2048)options.inSampleSize*=2;BitmapFactory.decodeByteArray(bytes,0,bytes.size,options)?.asImageBitmap()?:error("图片无法解码")};if(generation==model.connectionGeneration){bitmap=decoded;issue=null}}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(generation==model.connectionGeneration)issue=error.message}finally{if(handle!=null&&generation==model.connectionGeneration)withContext(NonCancellable){runCatching{model.files("release",buildJsonObject{put("handleId",handle)})}}}}
    if(bitmap!=null)Image(bitmap!!,contentDescription="消息图片",contentScale=if(user)ContentScale.Crop else ContentScale.Fit,modifier=(if(user)Modifier.size(72.dp)else Modifier.fillMaxWidth().heightIn(max=320.dp)).clip(RoundedCornerShape(10.dp)).clickable{expanded=true})else if(issue!=null)TextButton(onClick={retry++}){Text(stringResource(R.string.coaxi_fusion_d4be33bd431387c9),color=CoaxiTheme.colors.textSecondary)}else LinearProgressIndicator(if(user)Modifier.width(72.dp)else Modifier.fillMaxWidth())
    if(expanded)CoaxiFilePreview(model,source){expanded=false}
}
