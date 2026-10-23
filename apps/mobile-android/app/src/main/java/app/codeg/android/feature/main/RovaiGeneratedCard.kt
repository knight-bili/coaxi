package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

import android.annotation.SuppressLint
import android.webkit.*
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.lifecycle.*
import androidx.lifecycle.compose.LocalLifecycleOwner
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*
import java.io.ByteArrayInputStream
import java.net.URI
import java.util.UUID

/** 原版卡片放在本地资源页面；桥只准当前卡片的 targets / preview / submit。 */
@SuppressLint("SetJavaScriptEnabled")
@Composable fun RovaiGeneratedCard(model:RovaiWorkspaceViewModel,threadId:String,message:JsonObject,conversationId:String?=null,child:Boolean=false,onClose:()->Unit){
    val context=LocalContext.current;val coroutine=rememberCoroutineScope();val lifecycle=LocalLifecycleOwner.current
    val scope=remember(threadId,conversationId,child){feedbackScope(threadId,conversationId,child)};val generation=remember(scope){model.connectionGeneration};val agentId=message.coreText("authorId")
    var index by remember(message.coreText("id")){mutableIntStateOf(0)};var card by remember(message.coreText("id"),index){mutableStateOf<JsonObject?>(null)};var issue by remember{mutableStateOf<String?>(null)}
    val foreground=remember{mutableStateOf(lifecycle.lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED))}
    DisposableEffect(lifecycle){val observer=LifecycleEventObserver{_,event->foreground.value=event!=Lifecycle.Event.ON_STOP&&lifecycle.lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED)};lifecycle.lifecycle.addObserver(observer);onDispose{lifecycle.lifecycle.removeObserver(observer)}}
    LaunchedEffect(scope,message.coreText("id"),index){card=null;try{
        context.assets.open("card-preview/index.html").use{it.read()}
        val result=model.query(RovaiOperation.FEEDBACK_OPERATION,buildJsonObject{put("action","cards.read");putJsonObject("payload"){put("scope",scope);put("agentId",agentId);put("messageId",message.coreText("id"));put("cardOrdinal",index)}}).coreObject()
        require(generation==model.connectionGeneration&&result["scope"]==scope&&result.coreText("messageId")==message.coreText("id")&&result.coreText("agentId")==agentId){"卡片归属已变化"};card=result;issue=null
    }catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message?:"交互卡片资源尚未准备"}}
    Dialog(onDismissRequest=onClose,properties=DialogProperties(usePlatformDefaultWidth=false)){
        Surface(Modifier.fillMaxSize(),color=CodegTheme.colors.bgElevated){Column(Modifier.fillMaxSize()){
            Row(Modifier.fillMaxWidth().padding(12.dp)){Text(stringResource(R.string.rovai_fusion_2a63d74963d5ed3d),Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(onClick=onClose){Text(stringResource(R.string.rovai_fusion_3fd47edce45b3603))}}
            issue?.let{Text(it,color=CodegTheme.colors.danger,modifier=Modifier.padding(16.dp))}
            val value=card
            if(value==null&&issue==null)LinearProgressIndicator(Modifier.fillMaxWidth())
            if(value!=null){
                if(value.coreLong("cardCount")>1)Row(Modifier.padding(horizontal=12.dp)){TextButton(enabled=index>0,onClick={index--}){Text(stringResource(R.string.rovai_fusion_bcf200709dbc7ff4))};Text("${index+1}/${value.coreLong("cardCount")}");TextButton(enabled=index+1<value.coreLong("cardCount"),onClick={index++}){Text(stringResource(R.string.rovai_fusion_dcd4eb0273699416))}}
                key(value){
                    val channel=remember{UUID.randomUUID().toString()};val previews=remember{mutableSetOf<String>()};val requests=remember{mutableSetOf<String>()};val retired=remember{mutableStateOf(false)}
                    AndroidView(Modifier.fillMaxWidth().weight(1f),factory={nativeContext->WebView(nativeContext).apply{
                        val web=this;val origin="https://rovai-card.local/";val page=origin+"index.html"
                        settings.javaScriptEnabled=true;settings.allowFileAccess=false;settings.allowContentAccess=false;settings.domStorageEnabled=false;settings.mixedContentMode=WebSettings.MIXED_CONTENT_NEVER_ALLOW;settings.setSupportMultipleWindows(false);CookieManager.getInstance().setAcceptThirdPartyCookies(this,false)
                        fun live()=!retired.value&&foreground.value&&generation==model.connectionGeneration&&web.url==page
                        addJavascriptInterface(object{
                            @JavascriptInterface fun postMessage(raw:String){if(raw.toByteArray(Charsets.UTF_8).size>65536)return;web.post{if(!live())return@post
                                val input=runCatching{Json.parseToJsonElement(raw).coreObject()}.getOrNull()?:return@post;val id=input.coreText("requestId");if(input.coreText("channel")!=channel||input.coreLong("schemaVersion")!=1L||runCatching{UUID.fromString(id)}.isFailure||requests.size>=16||!requests.add(id))return@post
                                coroutine.launch{
                                    val response=try{
                                        val action=input.coreText("action");val payload=input["payload"].coreObject()
                                        require(action in listOf("targets","preview","submit")){"卡片操作不可用"}
                                        if(action!="submit")require(payload["scope"]==scope){"卡片归属已变化"}
                                        if(action=="preview"){val source=payload["source"].coreObject();val sourceOrigin=source["origin"].coreObject();require(payload.coreText("agentId")==agentId&&source.coreText("kind")=="generated_card"&&sourceOrigin.coreText("messageId")==message.coreText("id")&&sourceOrigin.coreLong("cardOrdinal")==index.toLong()){"卡片来源已变化"}}
                                        if(action=="submit")require(payload.coreText("previewId") in previews){"原预览不属于这张卡片"}
                                        val result=model.query(RovaiOperation.FEEDBACK_OPERATION,buildJsonObject{put("action",action);put("payload",payload)}).coreObject();require(live()){ "卡片已关闭" }
                                        val filtered=if(action=="targets")JsonObject(result.toMutableMap().apply{put("targets",JsonArray(result["targets"].coreArray().filter{it.coreText("agentId")==agentId}))})else result
                                        if(action=="preview")previews.add(result.coreText("previewId"))
                                        buildJsonObject{put("channel",channel);put("requestId",id);put("ok",true);put("result",filtered)}
                                    }catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){buildJsonObject{put("channel",channel);put("requestId",id);put("ok",false);put("error",error.message?:"反馈未能完成")}}
                                    finally{requests.remove(id)}
                                    if(live())web.evaluateJavascript("window.RovaiCardPage?.reply(${response});",null)
                                }
                            }}
                        },"RovaiCardNative")
                        webViewClient=object:WebViewClient(){
                            override fun shouldOverrideUrlLoading(view:WebView,request:WebResourceRequest)=true
                            override fun shouldInterceptRequest(view:WebView,request:WebResourceRequest):WebResourceResponse{
                                val uri=runCatching{URI(request.url.toString())}.getOrNull();val path=uri?.rawPath?.removePrefix("/").orEmpty();val allowed=uri?.scheme=="https"&&uri.host=="rovai-card.local"&&uri.port==-1&&uri.rawUserInfo==null&&uri.rawQuery==null&&path.isNotEmpty()&&Regex("[A-Za-z0-9_./-]+").matches(path)&&path.split('/').none{it==".."||it=="."}
                                val bytes=if(allowed)runCatching{context.assets.open("card-preview/$path").use{it.readBytes()}}.getOrNull()else null
                                val mime=when(path.substringAfterLast('.')){"html"->"text/html";"js"->"text/javascript";"css"->"text/css";"woff2"->"font/woff2";"woff"->"font/woff";"png"->"image/png";"svg"->"image/svg+xml";else->"application/octet-stream"}
                                return WebResourceResponse(mime,"UTF-8",if(bytes==null)404 else 200,if(bytes==null)"Not Found" else "OK",mapOf("Cache-Control" to "no-store","X-Content-Type-Options" to "nosniff"),ByteArrayInputStream(bytes?:ByteArray(0)))
                            }
                            override fun onPageFinished(view:WebView,url:String){if(url!=page||!live())return;val init=buildJsonObject{put("channel",channel);put("card",value);put("language",context.resources.configuration.locales[0].toLanguageTag());put("dark",(context.resources.configuration.uiMode and android.content.res.Configuration.UI_MODE_NIGHT_MASK)==android.content.res.Configuration.UI_MODE_NIGHT_YES)};view.evaluateJavascript("window.RovaiCardPage?.mount($init);",null)}
                        };loadUrl(page)
                    }},onRelease={web->retired.value=true;web.removeJavascriptInterface("RovaiCardNative");web.stopLoading();web.loadUrl("about:blank");web.destroy();previews.clear();requests.clear()})
                }
            }
        }}
    }
}
