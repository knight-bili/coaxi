package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.unit.dp
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.serialization.json.*

@Composable fun RovaiMessageActions(model:RovaiWorkspaceViewModel,threadId:String,message:JsonObject,enabled:Boolean,onQuote:(JsonObject)->Boolean,onReply:(JsonObject)->Unit){
    var menu by remember{mutableStateOf(false)};var quote by remember{mutableStateOf(false)};var withdrawal by remember{mutableStateOf<JsonObject?>(null)}
    val clipboard=LocalClipboardManager.current;val state=model.state.collectAsState().value
    Box{TextButton(onClick={menu=true}){Text(stringResource(R.string.rovai_fusion_38844b135cf70dfc))};DropdownMenu(expanded=menu,onDismissRequest={menu=false}){
        DropdownMenuItem(text={Text(stringResource(R.string.rovai_fusion_cf945e21ddefbb46))},enabled=enabled,onClick={menu=false;onReply(buildJsonObject{put("id",message.coreText("id"));put("authorId",message.coreText("authorId"));put("authorDisplayName",message.coreText("authorDisplayName"));put("excerpt",message.coreText("body").take(320))})})
        DropdownMenuItem(text={Text(stringResource(R.string.rovai_fusion_cda0a153bacd4b29))},enabled=enabled,onClick={menu=false;quote=true})
        DropdownMenuItem(text={Text(stringResource(R.string.rovai_fusion_5d7fd21f1e3ca6b4))},onClick={menu=false;clipboard.setText(AnnotatedString(message.coreText("body")))})
        if(message["canWithdraw"]==JsonPrimitive(true))DropdownMenuItem(text={Text(stringResource(R.string.rovai_fusion_6b58198955259066))},enabled=enabled,onClick={menu=false;withdrawal=buildJsonObject{put("threadId",threadId);put("messageId",message.coreText("id"));put("expectedVersion",message.coreLong("version"))}})
    }}
    if(quote)RovaiQuoteSelector(model,threadId,message.coreText("id"),onQuote){quote=false}
    withdrawal?.let{frozen->AlertDialog(onDismissRequest={withdrawal=null},title={Text(stringResource(R.string.rovai_fusion_661248f8f921fd9e))},text={Text(stringResource(R.string.rovai_fusion_43eb0066355652ae))},confirmButton={TextButton(enabled=state.pending==null&&!state.busy,onClick={model.command(RovaiOperation.THREAD_MESSAGES_WITHDRAW,frozen);withdrawal=null}){Text(stringResource(R.string.rovai_fusion_6b58198955259066))}},dismissButton={TextButton(onClick={withdrawal=null}){Text(stringResource(R.string.rovai_fusion_2cd0f3be8738a86c))}})}
}
@Composable fun RovaiReplyPreview(reply:JsonObject,enabled:Boolean,onRemove:()->Unit){
    Surface(color=CodegTheme.colors.bgElevated){Row(Modifier.fillMaxWidth().padding(horizontal=12.dp,vertical=6.dp)){
        Column(Modifier.weight(1f)){Text(stringResource(R.string.rovai_fusion_5b82265ea61c9807)+reply.coreText("authorDisplayName").ifEmpty{"原消息"},style=MaterialTheme.typography.labelSmall);Text(reply.coreText("excerpt"),maxLines=2,color=CodegTheme.colors.textSecondary,style=MaterialTheme.typography.bodySmall)}
        TextButton(enabled=enabled,onClick=onRemove){Text(stringResource(R.string.rovai_fusion_d346633d3a4e54d3))}
    }}
}
