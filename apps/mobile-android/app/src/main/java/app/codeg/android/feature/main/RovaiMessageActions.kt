package app.codeg.android.feature.main

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

@Composable fun RovaiMessageActions(model:RovaiWorkspaceViewModel,threadId:String,message:JsonObject,enabled:Boolean,onReply:(JsonObject)->Unit){
    var menu by remember{mutableStateOf(false)};var withdrawal by remember{mutableStateOf<JsonObject?>(null)}
    val clipboard=LocalClipboardManager.current;val state=model.state.collectAsState().value
    Box{TextButton(onClick={menu=true}){Text("更多")};DropdownMenu(expanded=menu,onDismissRequest={menu=false}){
        DropdownMenuItem(text={Text("回复")},enabled=enabled,onClick={menu=false;onReply(buildJsonObject{put("id",message.coreText("id"));put("authorId",message.coreText("authorId"));put("authorDisplayName",message.coreText("authorDisplayName"));put("excerpt",message.coreText("body").take(320))})})
        DropdownMenuItem(text={Text("复制消息")},onClick={menu=false;clipboard.setText(AnnotatedString(message.coreText("body")))})
        if(message["canWithdraw"]==JsonPrimitive(true))DropdownMenuItem(text={Text("撤回")},enabled=enabled,onClick={menu=false;withdrawal=buildJsonObject{put("threadId",threadId);put("messageId",message.coreText("id"));put("expectedVersion",message.coreLong("version"))}})
    }}
    withdrawal?.let{frozen->AlertDialog(onDismissRequest={withdrawal=null},title={Text("撤回这条消息？")},text={Text("仅在消息版本仍一致、且允许撤回时执行。已开始执行的工作请另外停止。")},confirmButton={TextButton(enabled=state.pending==null&&!state.busy,onClick={model.command(RovaiOperation.THREAD_MESSAGES_WITHDRAW,frozen);withdrawal=null}){Text("撤回")}},dismissButton={TextButton(onClick={withdrawal=null}){Text("取消")}})}
}
@Composable fun RovaiReplyPreview(reply:JsonObject,enabled:Boolean,onRemove:()->Unit){
    Surface(color=CodegTheme.colors.bgElevated){Row(Modifier.fillMaxWidth().padding(horizontal=12.dp,vertical=6.dp)){
        Column(Modifier.weight(1f)){Text("回复 · "+reply.coreText("authorDisplayName").ifEmpty{"原消息"},style=MaterialTheme.typography.labelSmall);Text(reply.coreText("excerpt"),maxLines=2,color=CodegTheme.colors.textSecondary,style=MaterialTheme.typography.bodySmall)}
        TextButton(enabled=enabled,onClick=onRemove){Text("取消回复")}
    }}
}
