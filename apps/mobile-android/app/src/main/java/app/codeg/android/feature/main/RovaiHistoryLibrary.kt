package app.codeg.android.feature.main

import android.content.Context
import android.util.Base64
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.unit.dp
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.markdown.MarkdownContent
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*
import java.io.File
import java.security.MessageDigest
import java.util.UUID

private data class HistoryDocument(val file:File,val transcript:JsonObject?,val bytes:Long)
private suspend fun readImportedHistory(model:RovaiWorkspaceViewModel,context:Context,threadId:String,id:String,onProgress:(Long,Long)->Unit):HistoryDocument{
    val generation=model.connectionGeneration;val file=File(context.cacheDir,"rovai-import-${UUID.randomUUID()}.json");val digest=MessageDigest.getInstance("SHA-256");var offset=0L;var total:Long?=null;var expected:String?=null
    try{
        withContext(Dispatchers.IO){file.createNewFile();file.setReadable(false,false);file.setWritable(false,false);file.setReadable(true,true);file.setWritable(true,true)}
        withContext(Dispatchers.IO){file.outputStream()}.use{output->while(true){
            currentCoroutineContext().ensureActive();require(generation==model.connectionGeneration){"工作区已切换"}
            val page=model.query(RovaiOperation.HISTORY_LIBRARY,buildJsonObject{putJsonObject("scope"){put("threadId",threadId)};put("operation","read");put("importId",id);put("offset",offset)}).coreObject()
            val size=page.coreLong("totalBytes");require(size in 0..536870912L&&page.coreLong("offset")==offset&&(total==null||total==size)&&(expected==null||expected==page.coreText("digest"))){"导入记录快照发生变化"};total=size;expected=page.coreText("digest")
            val raw=page.coreText("bytes");require(raw.length<=720896){"历史分页超过限制"};val chunk=Base64.decode(raw,Base64.DEFAULT);require(offset+chunk.size<=size){"历史分页范围不正确"}
            withContext(Dispatchers.IO){output.write(chunk);digest.update(chunk)};offset+=chunk.size;onProgress(offset,size)
            val next=page["next"];if(next==null||next==JsonNull){require(offset==size){"历史快照读取不完整"};break};require((next as?JsonPrimitive)?.longOrNull==offset&&chunk.isNotEmpty()){ "历史分页无效" }
        }}
        require(digest.digest().joinToString(""){"%02x".format(it)}==expected&&generation==model.connectionGeneration){"导入记录摘要不一致或工作区已切换"}
        val transcript=if(offset<=32L*1024*1024)withContext(Dispatchers.Default){Json.parseToJsonElement(file.readText(Charsets.UTF_8)).coreObject().also{require(it.coreLong("schema_version")==1L){"未知的原生记录格式"}}}else null
        return HistoryDocument(file,transcript,offset)
    }catch(error:Throwable){withContext(NonCancellable+Dispatchers.IO){file.delete()};throw error}
}

/** Codeg imported sessions + native message/tool rows. Imported locators stay inert. */
@Composable fun RovaiHistoryLibrary(model:RovaiWorkspaceViewModel,threadId:String,onClose:()->Unit){
    val context=LocalContext.current;val coroutine=rememberCoroutineScope();val generation=model.connectionGeneration
    var archived by remember(threadId){mutableStateOf(false)};var search by remember(threadId){mutableStateOf("")};var entries by remember(threadId){mutableStateOf<List<JsonObject>>(emptyList())};var selected by remember(threadId){mutableStateOf<JsonObject?>(null)};var tags by remember{mutableStateOf("")}
    var document by remember{mutableStateOf<HistoryDocument?>(null)};var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)};var progress by remember{mutableStateOf("")};var turnLimit by remember{mutableIntStateOf(50)};var confirm by remember{mutableStateOf(false)}
    var execution by remember(threadId){mutableStateOf<Pair<String,String?>?>(null)}
    var creationRecords by remember(threadId){mutableStateOf(false)}
    if(creationRecords&&selected!=null)RovaiNativeCreationRecords(model,selected!!){creationRecords=false}
    execution?.let{request->val row=selected;val transcript=document?.transcript;if(row!=null&&transcript!=null)RovaiNativeContinuation(model,row,transcript,request.first,request.second){execution=null}}
    val export=rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/json")){uri->val frozen=document;if(uri!=null&&frozen!=null)coroutine.launch{busy=true;try{require(generation==model.connectionGeneration){"工作区已切换"};withContext(Dispatchers.IO){context.contentResolver.openOutputStream(uri,"w")?.use{output->frozen.file.inputStream().use{it.copyTo(output)}}?:error("无法写入导出位置")}}catch(error:Exception){issue=error.message}finally{busy=false}}}
    suspend fun load(){val result=model.query(RovaiOperation.HISTORY_LIBRARY,buildJsonObject{putJsonObject("scope"){put("threadId",threadId)};put("operation","list");put("includeArchived",archived)}).coreObject();require(generation==model.connectionGeneration){"工作区已切换"};entries=result["entries"].coreArray();selected?.let{old->selected=entries.find{it.coreText("id")==old.coreText("id")}}}
    fun mutate(operation:String){val row=selected?:return;if(busy)return;busy=true;issue=null;coroutine.launch{try{model.query(RovaiOperation.HISTORY_LIBRARY,buildJsonObject{putJsonObject("scope"){put("threadId",threadId)};put("operation",operation);put("importId",row.coreText("id"));put("expectedRevision",row.coreText("revision"));if(operation=="tags")put("tags",JsonArray(tags.split(',','，').map{it.trim()}.filter{it.isNotEmpty()}.distinct().map(::JsonPrimitive)))});load();if(operation!="tags"){document?.file?.delete();document=null;selected=null};confirm=false}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}
    LaunchedEffect(threadId,generation,archived){busy=true;try{load()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    DisposableEffect(document?.file){val current=document?.file;onDispose{current?.delete()}}
    Column(Modifier.fillMaxSize()){
        Row(Modifier.fillMaxWidth().padding(12.dp)){TextButton(enabled=!busy,onClick={if(selected!=null){selected=null;document=null}else onClose()}){Text(if(selected!=null)"返回记录" else "返回对话")};Text("已导入会话",Modifier.weight(1f),style=MaterialTheme.typography.titleMedium)}
        issue?.let{Text(it,Modifier.padding(horizontal=12.dp),color=CodegTheme.colors.danger)};if(progress.isNotEmpty())Text(progress,Modifier.padding(horizontal=12.dp),style=MaterialTheme.typography.bodySmall)
        val row=selected
        if(row==null){Row(Modifier.padding(horizontal=12.dp)){OutlinedTextField(search,{search=it},label={Text("搜索标题、来源或标签")},modifier=Modifier.weight(1f));FilterChip(selected=archived,enabled=!busy,onClick={archived=!archived},label={Text("显示归档")})}
            LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(12.dp),verticalArrangement=Arrangement.spacedBy(8.dp)){items(entries.filter{(it["summary"].coreObject().coreText("title")+" "+it.coreText("sourceName")+" "+it["tags"].toString()).contains(search,ignoreCase=true)},key={it.coreText("id")}){item->GlassRow(Modifier.fillMaxWidth()){
                Text(item["summary"].coreObject().coreText("title").ifBlank{item.coreText("sourceName")},style=MaterialTheme.typography.titleSmall);Text(item.coreText("provider")+" · "+item.coreText("importedAt")+(if(item["archived"]==JsonPrimitive(true))" · 已归档" else ""),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary)
                TextButton(enabled=!busy,onClick={selected=item;tags=(item["tags"]as?JsonArray)?.joinToString(", "){(it as?JsonPrimitive)?.contentOrNull.orEmpty()}.orEmpty();document=null;turnLimit=50;busy=true;issue=null;coroutine.launch{try{val result=readImportedHistory(model,context,threadId,item.coreText("id")){done,total->progress="$done / $total"};if(generation==model.connectionGeneration)document=result else result.file.delete()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false;progress=""}}}){Text("查看原始消息和工具记录")}
            }};if(entries.isEmpty()&&!busy)item{Text("还没有导入记录。可在桌面端选择原生会话文件夹导入。",color=CodegTheme.colors.textSecondary)}}
        }else{
            OutlinedTextField(tags,{tags=it},Modifier.fillMaxWidth().padding(horizontal=12.dp),enabled=!busy,label={Text("逗号分隔标签")})
            Row(Modifier.padding(horizontal=12.dp)){TextButton(enabled=!busy,onClick={mutate("tags")}){Text("保存标签")};TextButton(enabled=!busy,onClick={confirm=true}){Text(if(row["archived"]==JsonPrimitive(true))"恢复记录" else "归档记录")};TextButton(enabled=!busy&&document!=null,onClick={export.launch("imported-${row.coreText("id").take(12)}.json")}){Text("导出会话记录")}}
            val transcript=document?.transcript
            TextButton(enabled=!busy,onClick={creationRecords=true}){Text("继续与分叉的原创建记录")}
            if(transcript!=null&&row["archived"]!=JsonPrimitive(true))Row(Modifier.padding(horizontal=12.dp)){TextButton(enabled=!busy,onClick={execution="continue" to null}){Text("从原生会话继续")};TextButton(enabled=!busy,onClick={execution="fork_tail" to null}){Text("从末尾分叉")}}
            LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(12.dp),verticalArrangement=Arrangement.spacedBy(10.dp)){
                if(document!=null&&transcript==null)item{Text("记录较大，已完整校验并保存为临时文件。请导出查看完整内容，或在桌面打开。")}
                val turns=transcript?.get("turns").coreArray();items(turns.take(turnLimit),key={it.coreText("id")}){turn->GlassRow(Modifier.fillMaxWidth()){
                    Text(when(turn.coreText("role")){"user"->"你";"assistant"->"队员";else->"系统"},style=MaterialTheme.typography.labelMedium);Text(turn.coreText("timestamp"),style=MaterialTheme.typography.labelSmall,color=CodegTheme.colors.textSecondary)
                    turn["blocks"].coreArray().forEach{block->RovaiImportedBlock(block)}
                    if(!busy&&row["archived"]!=JsonPrimitive(true)&&turn.coreText("role")=="assistant"&&row.coreText("provider")in listOf("codex","claude","deepseek"))TextButton(onClick={execution="fork_point" to turn.coreText("id")}){Text("从这条回答分叉")}
                }};if(turns.size>turnLimit)item{TextButton(onClick={turnLimit+=50}){Text("更多消息")}}
                val notes=transcript?.get("notes").coreObject();if(notes.coreLong("malformed_records")>0||notes["incomplete_tail"]==JsonPrimitive(true))item{Text("原文件含无法解析或尚未写完的记录，已保留可解析部分。",color=CodegTheme.colors.textSecondary)}
            }
        }
    }
    if(confirm)AlertDialog(onDismissRequest={if(!busy)confirm=false},title={Text(if(selected?.get("archived")==JsonPrimitive(true))"恢复这条记录？" else "归档这条记录？")},text={Text("只调整导入库中的记录，保留来源资料。")},confirmButton={TextButton(enabled=!busy,onClick={mutate(if(selected?.get("archived")==JsonPrimitive(true))"restore" else "archive")}){Text("确认")}},dismissButton={TextButton(enabled=!busy,onClick={confirm=false}){Text("取消")}})
}

@Composable private fun RovaiImportedBlock(block:JsonObject){
    var expanded by remember(block){mutableStateOf(false)}
    when(block.coreText("type")){
        "text"->MarkdownContent(block.coreText("text"))
        "thinking"->{TextButton(onClick={expanded=!expanded}){Text(if(expanded)"收起思考" else "查看思考")};if(expanded)MarkdownContent(block.coreText("text"))}
        "turn_error"->Text(block.coreText("message"),color=CodegTheme.colors.danger)
        "tool_use","tool_result"->{val use=block.coreText("type")=="tool_use";TextButton(onClick={expanded=!expanded}){Text(if(use)block.coreText("tool_name").ifBlank{"工具调用"} else "工具结果")};if(expanded)SelectionContainerCompat(if(use)block.coreText("input_preview")else block.coreText("output_preview"))}
        "image"->RovaiImportedImage(block)
        "image_generation"->{if(block.coreText("revised_prompt").isNotEmpty())Text(block.coreText("revised_prompt"),style=MaterialTheme.typography.bodySmall);RovaiImportedImage(block["image"].coreObject())}
    }
}
@Composable private fun SelectionContainerCompat(text:String){androidx.compose.foundation.text.selection.SelectionContainer{Text(text,style=MaterialTheme.typography.bodySmall)}}
@Composable private fun RovaiImportedImage(value:JsonObject){
    var bitmap by remember(value){mutableStateOf<android.graphics.Bitmap?>(null)};var issue by remember{mutableStateOf<String?>(null)};var open by remember{mutableStateOf(false)}
    LaunchedEffect(value){try{val raw=value.coreText("data");if(raw.isEmpty()){issue="原记录只保留图片引用，未读取外部地址";return@LaunchedEffect};require(raw.length<=28*1024*1024){"图片较大，请导出记录查看"};val decoded=withContext(Dispatchers.Default){val bytes=Base64.decode(raw,Base64.DEFAULT);val bounds=android.graphics.BitmapFactory.Options().apply{inJustDecodeBounds=true};android.graphics.BitmapFactory.decodeByteArray(bytes,0,bytes.size,bounds);require(bounds.outWidth>0&&bounds.outHeight>0){"图片格式不支持"};var sample=1;while(bounds.outWidth/sample>2048||bounds.outHeight/sample>2048)sample*=2;android.graphics.BitmapFactory.decodeByteArray(bytes,0,bytes.size,android.graphics.BitmapFactory.Options().apply{inSampleSize=sample})};bitmap=decoded}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}}
    val image=bitmap;if(image!=null){TextButton(onClick={open=true}){androidx.compose.foundation.Image(image.asImageBitmap(),contentDescription="原生会话图片",modifier=Modifier.size(120.dp))};if(open)androidx.compose.ui.window.Dialog(onDismissRequest={open=false}){Surface{Column{TextButton(onClick={open=false}){Text("关闭")};androidx.compose.foundation.Image(image.asImageBitmap(),contentDescription="原生会话图片",modifier=Modifier.fillMaxWidth())}}}}else issue?.let{Text(it,color=CodegTheme.colors.textSecondary)}
}
