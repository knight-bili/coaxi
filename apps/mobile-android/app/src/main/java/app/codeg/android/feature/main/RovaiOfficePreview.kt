package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

import android.webkit.*
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
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
import java.util.UUID
import java.net.URI

fun rovaiOfficeFile(path:String)=path.substringAfterLast('.').lowercase() in listOf("docx","xlsx","pptx")
/** Codeg OfficePreview lifecycle with the shared Host's opaque sandbox proxy. */
@Composable
fun RovaiOfficePreview(model:RovaiWorkspaceViewModel,threadId:String,path:String,onClose:()->Unit,attachmentLocator:JsonObject?=null){
    var showInstaller by remember{mutableStateOf(false)}
    var editable by remember{mutableStateOf(false)};var retry by remember{mutableIntStateOf(0)};var url by remember{mutableStateOf<String?>(null)};var issue by remember{mutableStateOf<String?>(null)}
    val owner=LocalLifecycleOwner.current;var active by remember{mutableStateOf(owner.lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED))}
    DisposableEffect(owner){val listener=LifecycleEventObserver{_,event->if(event==Lifecycle.Event.ON_STOP)active=false else if(event==Lifecycle.Event.ON_START)active=true};owner.lifecycle.addObserver(listener);onDispose{owner.lifecycle.removeObserver(listener)}}
    LaunchedEffect(threadId,path,editable,retry,active,attachmentLocator){url=null;if(!active)return@LaunchedEffect;val generation=model.connectionGeneration;val id=UUID.randomUUID().toString();val scope=buildJsonObject{put("threadId",threadId)}
        try{val lease=model.query(RovaiOperation.OFFICE_WATCH,buildJsonObject{put("scope",scope);if(attachmentLocator!=null)put("attachmentLocator",attachmentLocator);put("operation","start");put("holdId",id);put("path",path);put("editable",editable&&attachmentLocator==null)}).coreObject();require(generation==model.connectionGeneration&&lease.coreText("holdId")==id){"工作区或预览归属已变化"};url=model.officePreviewUrl(lease.coreLong("port").toInt(),lease.coreText("cap"));issue=null
            while(isActive){delay(30000);model.query(RovaiOperation.OFFICE_WATCH,buildJsonObject{put("scope",scope);if(attachmentLocator!=null)put("attachmentLocator",attachmentLocator);put("operation","heartbeat");put("holdId",id)})}
        }catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message;url=null}finally{url=null;if(generation==model.connectionGeneration)withContext(NonCancellable){runCatching{model.query(RovaiOperation.OFFICE_WATCH,buildJsonObject{put("scope",scope);if(attachmentLocator!=null)put("attachmentLocator",attachmentLocator);put("operation","stop");put("holdId",id)})}}}}
    Dialog(onDismissRequest=onClose,properties=DialogProperties(usePlatformDefaultWidth=false)){Surface(Modifier.fillMaxSize(),color=CodegTheme.colors.bgElevated){Column(Modifier.fillMaxSize().padding(12.dp),verticalArrangement=Arrangement.spacedBy(8.dp)){
        Row{Text(path.substringAfterLast('/'),Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(onClick=onClose){Text(stringResource(R.string.rovai_fusion_3fd47edce45b3603))}}
        Row{if(attachmentLocator==null){Checkbox(checked=editable,onCheckedChange={editable=it});Text(stringResource(R.string.rovai_fusion_b1887b948b3759d3))}else Text(stringResource(R.string.rovai_fusion_428a00a80d4bb1cf));Spacer(Modifier.weight(1f));TextButton(onClick={retry++}){Text(stringResource(R.string.rovai_fusion_68891ab35cab9663))}}
        issue?.let{Text(it,color=CodegTheme.colors.danger)}
        TextButton(onClick={showInstaller=!showInstaller}){Text(if(showInstaller)"收起Office程序管理" else "Office程序管理")}
        if(showInstaller)RovaiOfficeCliSettings(model)
        val preview=url
        if(preview==null&&issue==null&&active)LinearProgressIndicator(Modifier.fillMaxWidth())
        if(preview!=null)key(preview){AndroidView(Modifier.fillMaxSize(),factory={context->WebView(context).apply{settings.javaScriptEnabled=true;settings.allowFileAccess=false;settings.allowContentAccess=false;settings.domStorageEnabled=false;settings.setSupportMultipleWindows(false);settings.mixedContentMode=WebSettings.MIXED_CONTENT_NEVER_ALLOW;CookieManager.getInstance().setAcceptThirdPartyCookies(this,false)
            val target=URI(preview);fun admitted(raw:String):Boolean=runCatching{val uri=URI(raw);uri.scheme==target.scheme&&uri.host==target.host&&uri.port==target.port&&uri.rawUserInfo==null&&uri.path.startsWith(target.path)&&!uri.path.split('/').any{it==".."||it=="."}&&uri.rawQuery?.split('&')?.count{it=="cap="+target.rawQuery.substringAfter("cap=")}==1}.getOrDefault(false)
            webViewClient=object:WebViewClient(){override fun shouldOverrideUrlLoading(view:WebView,request:WebResourceRequest)=request.isForMainFrame||!admitted(request.url.toString());override fun shouldInterceptRequest(view:WebView,request:WebResourceRequest):WebResourceResponse?{return if(admitted(request.url.toString()))null else WebResourceResponse("text/plain","UTF-8",java.io.ByteArrayInputStream(ByteArray(0)))}}
            val origin=target.scheme+"://"+target.rawAuthority;val escaped=preview.replace("&","&amp;").replace("\"","&quot;")
            loadDataWithBaseURL(null,"<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; frame-src $origin; style-src 'unsafe-inline'\"><style>html,body,iframe{width:100%;height:100%;margin:0;border:0}</style><iframe sandbox=\"allow-scripts\" referrerpolicy=\"no-referrer\" src=\"$escaped\"></iframe>","text/html","UTF-8",null)
        }},onRelease={it.stopLoading();it.loadUrl("about:blank");it.destroy()})}
    }}}
}
