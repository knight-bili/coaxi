package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

import android.graphics.BitmapFactory
import android.webkit.*
import androidx.compose.foundation.Image
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import app.coaxi.android.core.designsystem.markdown.MarkdownContent
import app.coaxi.android.core.designsystem.theme.CoaxiTheme
import app.coaxi.android.core.designsystem.component.GlassRow
import kotlinx.coroutines.*
import kotlinx.serialization.json.*

/** Native Coaxi rows and Markdown; Core/Host retains source and version authority. */
@Composable
fun CoaxiMessageArtifacts(model:CoaxiWorkspaceViewModel,threadId:String,message:JsonObject,conversationId:String?=null,child:Boolean=false){
    var source by remember(threadId,message.coreText("id")){mutableStateOf<JsonObject?>(null)}
    message["quotes"].coreArray().forEach{quote->GlassRow{Text(stringResource(R.string.coaxi_fusion_3bcf3c1e64b2165d)+quote["authorAtCapture"].coreObject().coreText("displayName"),style=MaterialTheme.typography.labelSmall);MarkdownContent(quote.coreText("text"));val origin=quote["source"].coreObject();if(origin.coreText("scope")=="camp"&&origin.coreText("threadId")==threadId&&origin.coreText("messageId").isNotEmpty())CoaxiPublicMessageOrigin(model,threadId,origin.coreText("messageId"),"查看引用原消息")}}
    if(conversationId==null&&message.coreText("replyToThreadMessageId").isNotEmpty())CoaxiPublicMessageOrigin(model,threadId,message.coreText("replyToThreadMessageId"),"查看回复原消息")
    message["attachments"]
        .coreArray()
        .filter { it.coreText("previewKind") != "image" }
        .forEach { attachment ->
            TextButton(
                onClick = {
                    source = buildJsonObject {
                        put("kind", "attachment")
                        put("threadId", threadId)
                        putJsonObject("locator") {
                            put("threadId", threadId)
                            put("attachmentRefId", attachment.coreText("id"))
                            if (conversationId == null) {
                                put("owner", "message")
                                put("messageId", message.coreText("id"))
                            } else {
                                put("owner", if (child) "delegation_child_message" else "single_chat_message")
                                put("conversationId", conversationId)
                                put("conversationMessageId", message.coreText("id"))
                            }
                        }
                    }
                },
                enabled = attachment.coreText("availability") !in listOf("missing", "unreadable", "kind_changed")
            ) {
                Text(
                    (if (attachment.coreText("previewKind") == "image") "图片 · " else "文件 · ") +
                        attachment.coreText("displayName")
                )
            }
        }
    source?.let{request->CoaxiFilePreview(model,request){source=null}}
}
@Composable
fun CoaxiFilePreview(model:CoaxiWorkspaceViewModel,source:JsonObject,onClose:()->Unit){
    val scope=rememberCoroutineScope();val generation=remember(source){model.connectionGeneration}
    var office by remember(source){mutableStateOf(false)}
    var file by remember(source){mutableStateOf<JsonObject?>(null)};var page by remember(source){mutableStateOf<JsonObject?>(null)};var image by remember(source){mutableStateOf<ImageBitmap?>(null)}
    var issue by remember{mutableStateOf<String?>(null)};var busy by remember{mutableStateOf(false)};var html by remember{mutableStateOf<String?>(null)};var renderHtml by remember{mutableStateOf(false)};var previous by remember{mutableStateOf<List<Long>>(emptyList())}
    suspend fun read(offset:Long=0){val selected=file?:return;busy=true;try{require(generation==model.connectionGeneration){"工作区已切换"};val value=model.files("readPage",buildJsonObject{put("handleId",selected.coreText("handleId"));put("expectedGeneration",selected.coreText("contentGeneration"));put("offset",offset);put("maxBytes",128*1024)});page=value;issue=null}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    LaunchedEffect(source){busy=true;var handle:String?=null;try{val opened=model.files("open",source);require(opened.coreText("kind")=="file_preview"){"此来源暂不能预览"};val selected=opened["file"].coreObject();handle=selected.coreText("handleId");require(generation==model.connectionGeneration){"工作区已切换"};file=selected
        if(selected.coreText("kind")=="image"){require(selected.coreLong("size")<=20*1024*1024){"图片超过20MB，请在工程中查看"};val data=model.fileBytes("readBinary",buildJsonObject{put("handleId",handle);put("expectedGeneration",selected.coreText("contentGeneration"))});image=withContext(Dispatchers.Default){val bounds=BitmapFactory.Options().apply{inJustDecodeBounds=true};BitmapFactory.decodeByteArray(data.bytes,0,data.bytes.size,bounds);require(bounds.outWidth in 1..100000&&bounds.outHeight in 1..100000){"图片尺寸无法读取"};val options=BitmapFactory.Options();while(bounds.outWidth/options.inSampleSize.coerceAtLeast(1)>2048||bounds.outHeight/options.inSampleSize.coerceAtLeast(1)>2048)options.inSampleSize=options.inSampleSize.coerceAtLeast(1)*2;BitmapFactory.decodeByteArray(data.bytes,0,data.bytes.size,options)?.asImageBitmap()?:error("图片无法解码")}}
        else if(!coaxiOfficeFile(selected.coreText("fileName")))read()
        issue=null
        busy=false
        awaitCancellation()
    }catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false;val owned=handle;if(owned!=null&&generation==model.connectionGeneration)withContext(NonCancellable){runCatching{model.files("release",buildJsonObject{put("handleId",owned)})}}}}
    LaunchedEffect(model.connectionGeneration){if(generation!=model.connectionGeneration)onClose()}
    Dialog(onDismissRequest=onClose,properties=DialogProperties(usePlatformDefaultWidth=false)){Surface(Modifier.fillMaxSize(),color=CoaxiTheme.colors.bgElevated){Column(Modifier.fillMaxSize().padding(16.dp),verticalArrangement=Arrangement.spacedBy(10.dp)){
        Row{Text(file?.coreText("fileName")?:"文件预览",Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(onClick=onClose){Text(stringResource(R.string.coaxi_fusion_3fd47edce45b3603))}}
        if(source.coreText("kind")=="attachment"&&file?.coreText("fileName")?.let(::coaxiOfficeFile)==true)TextButton(onClick={office=true}){Text(stringResource(R.string.coaxi_fusion_dcffc3beb81d318b))}
        if(busy)LinearProgressIndicator(Modifier.fillMaxWidth());issue?.let{Text(it,color=CoaxiTheme.colors.danger);TextButton(enabled=!busy&&file!=null,onClick={scope.launch{read(page?.coreLong("startOffset")?:0)}}){Text(stringResource(R.string.coaxi_fusion_1133d6759e101e21))}}
        image?.let{Image(it,contentDescription=file?.coreText("fileName"),modifier=Modifier.fillMaxWidth().weight(1f),contentScale=ContentScale.Fit)}
        if(file?.coreText("kind")=="html"){TextButton(enabled=!busy,onClick={if(renderHtml)renderHtml=false else scope.launch{busy=true;try{val selected=file?:return@launch;val value=model.files("readHtml",buildJsonObject{put("handleId",selected.coreText("handleId"));put("expectedGeneration",selected.coreText("contentGeneration"))});html=value.coreText("text");renderHtml=true}catch(error:Exception){issue=error.message}finally{busy=false}}}){Text(if(renderHtml)"查看源码" else "查看HTML页面")}}
        if(renderHtml&&html!=null){AndroidView(modifier=Modifier.weight(1f).fillMaxWidth(),factory={context->WebView(context).apply{settings.javaScriptEnabled=false;settings.allowFileAccess=false;settings.allowContentAccess=false;settings.blockNetworkLoads=true;settings.domStorageEnabled=false;settings.setSupportMultipleWindows(false);webViewClient=object:WebViewClient(){override fun shouldOverrideUrlLoading(view:WebView,request:WebResourceRequest)=true};loadDataWithBaseURL("https://preview.invalid/","<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data:; style-src 'unsafe-inline'\">"+html,"text/html","UTF-8",null)}},onRelease={it.stopLoading();it.destroy()})}
        else page?.let{value->SelectionContainer(Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState())){if(file?.coreText("kind")=="markdown")MarkdownContent(value.coreText("text"))else Text(value.coreText("text"),fontFamily=FontFamily.Monospace,modifier=Modifier.horizontalScroll(rememberScrollState()))}}
        if(page!=null&&!renderHtml)Row{TextButton(enabled=!busy&&previous.isNotEmpty(),onClick={val offset=previous.lastOrNull()?:return@TextButton;previous=previous.dropLast(1);scope.launch{read(offset)}}){Text(stringResource(R.string.coaxi_fusion_c9b9ae7a61444ab7))};Spacer(Modifier.weight(1f));TextButton(enabled=!busy&&page?.get("hasNext")==JsonPrimitive(true),onClick={val current=page?:return@TextButton;previous=previous+current.coreLong("startOffset");scope.launch{read(current.coreLong("endOffset"))}}){Text(stringResource(R.string.coaxi_fusion_8a8542f6964852dc))}}
    }}}
    if(office)CoaxiOfficePreview(model,source.coreText("threadId"),file?.coreText("fileName").orEmpty(),onClose={office=false},attachmentLocator=source["locator"].coreObject())
}
