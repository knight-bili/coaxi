package app.codeg.android.feature.main

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*

/** Native text selection over the exact Core projection; the capture API fences the source. */
@Composable fun RovaiQuoteSelector(model:RovaiWorkspaceViewModel,threadId:String,messageId:String,onChosen:(JsonObject)->Boolean,conversationId:String?=null,onSelection:((JsonObject)->Boolean)?=null,onClose:()->Unit){
    val generation=model.connectionGeneration;val scope=rememberCoroutineScope()
    var projection by remember{mutableStateOf<JsonObject?>(null)};var value by remember{mutableStateOf(TextFieldValue())}
    var busy by remember{mutableStateOf(true)};var issue by remember{mutableStateOf<String?>(null)}
    LaunchedEffect(threadId,messageId){try{val result=model.query(if(conversationId==null)RovaiOperation.MESSAGEQUOTES_PROJECTION else RovaiOperation.MESSAGEQUOTES_PRIVATEPROJECTION,buildJsonObject{put("threadId",threadId);put("messageId",messageId);if(conversationId!=null)put("conversationId",conversationId)}).coreObject();require(generation==model.connectionGeneration);projection=result;val text=result.coreText("text");value=TextFieldValue(text,if(text.codePointCount(0,text.length)<=result.coreLong("maxScalars"))TextRange(0,text.length)else TextRange.Zero)}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    val start=minOf(value.selection.start,value.selection.end);val end=maxOf(value.selection.start,value.selection.end)
    val selected=value.text.substring(start,end);val count=selected.codePointCount(0,selected.length)
    Dialog(onDismissRequest=onClose,properties=DialogProperties(usePlatformDefaultWidth=false)){Surface(Modifier.fillMaxSize(),color=CodegTheme.colors.bgElevated){Column(Modifier.fillMaxSize().padding(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)){
        Row{Text("选择引用内容",Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(onClick=onClose){Text("关闭")}}
        Text("长按文字选择一段内容。引用保持当时的快照，原消息变化时拒绝捕获。",style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)
        issue?.let{Text(it,color=CodegTheme.colors.danger)}
        OutlinedTextField(value=value,onValueChange={next->value=value.copy(selection=next.selection)},readOnly=true,modifier=Modifier.fillMaxWidth().weight(1f),enabled=!busy)
        Text("已选 $count / ${projection?.coreLong("maxScalars")?:12000} 字符",style=MaterialTheme.typography.labelSmall)
        Button(enabled=!busy&&projection!=null&&selected.isNotBlank()&&count<=(projection?.coreLong("maxScalars")?:0),onClick={val original=projection?:return@Button;val frozen=buildJsonObject{put("threadId",threadId);putJsonObject("selection"){put("messageId",messageId);put("bodyAtSelection",original.coreText("bodyAtSelection"));put("startScalar",value.text.codePointCount(0,start));put("endScalar",value.text.codePointCount(0,end));put("text",selected)}};busy=true;scope.launch{try{require(generation==model.connectionGeneration){"工作区已切换"};if(conversationId!=null){require(onSelection?.invoke(frozen["selection"].coreObject())==true){"输入框当前无法接收引用，请稍后重试"}}else{val quote=model.query(RovaiOperation.MESSAGEQUOTES_CAPTURE,frozen).coreObject();require(generation==model.connectionGeneration){"工作区已切换"};require(onChosen(quote)){"输入框当前无法接收引用，请稍后重试"}};onClose()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}){Text("加入引用")}
    }}}
}

@Composable fun RovaiComposerQuotes(quotes:List<JsonObject>,enabled:Boolean,onRemove:(String)->Unit){
    quotes.forEach{quote->Row(Modifier.fillMaxWidth().padding(horizontal=12.dp,vertical=4.dp)){
        Column(Modifier.weight(1f)){Text("引用 · "+quote["authorAtCapture"].coreObject().coreText("displayName"),style=MaterialTheme.typography.labelSmall);Text(quote.coreText("text"),maxLines=2,style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)}
        TextButton(enabled=enabled,onClick={onRemove(quote.coreText("quoteId"))}){Text("移除")}
    }}
}
