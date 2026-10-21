package app.codeg.android.feature.main

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.model.QuickMessage
import app.codeg.android.core.network.RovaiOperation
import app.codeg.android.feature.settings.QuickMessageEditor
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import java.util.UUID

@Composable
fun RovaiMessageLibrary(model:RovaiWorkspaceViewModel,threadId:String?=null,onUse:((String)->Boolean)?=null){
    var level by remember{mutableStateOf("global")};var value by remember{mutableStateOf<JsonObject?>(null)};var digest by remember{mutableStateOf("")};var issue by remember{mutableStateOf<String?>(null)};var busy by remember{mutableStateOf(false)}
    var editing by remember{mutableStateOf<JsonObject?>(null)};var creating by remember{mutableStateOf(false)};var tag by remember{mutableStateOf("")};val scope=rememberCoroutineScope();val generation=model.connectionGeneration
    fun args(next:JsonObject?=null)=buildJsonObject{put("level",level);threadId?.let{putJsonObject("scope"){put("threadId",it)}};if(next!=null){put("expectedDigest",digest);put("value",next)}}
    fun save(next:JsonObject,onSaved:()->Unit={}){if(busy||model.state.value.pending!=null)return;val frozen=args(next);busy=true;issue=null;scope.launch{try{val result=model.query(RovaiOperation.WORKBENCH_ORGANIZER_SET,frozen).coreObject();if(generation==model.connectionGeneration){value=result["value"].coreObject();digest=result.coreText("digest");onSaved()}}catch(cancelled:kotlinx.coroutines.CancellationException){throw cancelled}catch(error:Exception){if(generation==model.connectionGeneration)issue=error.message}finally{busy=false}}}
    fun withMessages(messages:List<JsonObject>)=buildJsonObject{value?.forEach{(key,item)->put(key,item)};put("quickMessages",JsonArray(messages))}
    LaunchedEffect(level,generation){busy=true;value=null;issue=null;try{val result=model.query(RovaiOperation.WORKBENCH_ORGANIZER_GET,args()).coreObject();if(generation==model.connectionGeneration){value=result["value"].coreObject();digest=result.coreText("digest")}}catch(cancelled:kotlinx.coroutines.CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    val messages=value?.get("quickMessages").coreArray();val tags=(value?.get("tags")as?JsonArray)?.mapNotNull{(it as?JsonPrimitive)?.contentOrNull}?:emptyList()
    Column(Modifier.fillMaxSize()){
        Row(Modifier.fillMaxWidth().padding(12.dp),horizontalArrangement=Arrangement.spacedBy(8.dp)){FilterChip(selected=level=="global",enabled=!busy&&!creating&&editing==null,onClick={level="global"},label={Text("全局")});if(threadId!=null){FilterChip(selected=level=="project",enabled=!busy&&!creating&&editing==null,onClick={level="project"},label={Text("当前工程")});FilterChip(selected=level=="thread",enabled=!busy&&!creating&&editing==null,onClick={level="thread"},label={Text("当前对话")})};Spacer(Modifier.weight(1f));IconButton(enabled=value!=null&&!busy,onClick={creating=true}){Icon(Icons.Outlined.Add,"新建快捷消息")}}
        issue?.let{Text(it,Modifier.padding(horizontal=12.dp),color=CodegTheme.colors.danger)}
        LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(12.dp),verticalArrangement=Arrangement.spacedBy(8.dp)){
            item{Text("快捷消息",style=MaterialTheme.typography.titleMedium);Text("选择后只填入输入框，由你发送。",color=CodegTheme.colors.textSecondary)}
            itemsIndexed(messages,key={_,item->item.coreText("id")}){index,message->GlassRow(Modifier.fillMaxWidth()){
                Text(message.coreText("title"),style=MaterialTheme.typography.titleSmall);Text(message.coreText("text"),maxLines=3,color=CodegTheme.colors.textSecondary)
                Row(horizontalArrangement=Arrangement.spacedBy(8.dp)){if(onUse!=null)TextButton(enabled=!busy,onClick={if(!onUse(message.coreText("text")))issue="当前输入暂时不能插入，请先核对原操作。"}){Text("填入")};TextButton(enabled=!busy,onClick={editing=message}){Text("编辑")};IconButton(enabled=!busy&&index>0,onClick={val next=messages.toMutableList();next[index]=next[index-1].also{next[index-1]=message};save(withMessages(next))}){Icon(Icons.Outlined.ArrowUpward,"上移")};IconButton(enabled=!busy&&index<messages.lastIndex,onClick={val next=messages.toMutableList();next[index]=next[index+1].also{next[index+1]=message};save(withMessages(next))}){Icon(Icons.Outlined.ArrowDownward,"下移")};IconButton(enabled=!busy,onClick={save(withMessages(messages.filter{it.coreText("id")!=message.coreText("id")}))}){Icon(Icons.Outlined.DeleteOutline,"删除")}}
            }}
            item{Text("标签",style=MaterialTheme.typography.titleMedium);tags.forEach{label->Row{Text(label,Modifier.weight(1f));IconButton(enabled=!busy,onClick={save(buildJsonObject{value?.forEach{(key,item)->put(key,item)};put("tags",JsonArray(tags.filter{it!=label}.map(::JsonPrimitive)))})}){Icon(Icons.Outlined.Close,"移除标签")}}};Row{OutlinedTextField(tag,{tag=it},enabled=!busy,modifier=Modifier.weight(1f),label={Text("新标签")});TextButton(enabled=value!=null&&!busy&&tag.isNotBlank()&&tag.trim() !in tags,onClick={save(buildJsonObject{value?.forEach{(key,item)->put(key,item)};put("tags",JsonArray((tags+tag.trim()).map(::JsonPrimitive)))}){tag=""}}){Text("添加")}}}
        }
    }
    if(creating||editing!=null){val frozen=editing;QuickMessageEditor(existing=frozen?.let{QuickMessage(0,it.coreText("title"),it.coreText("text"),0)},onDismiss={if(!busy){creating=false;editing=null}},busy=busy,error=issue,onSave={title,body->val item=buildJsonObject{put("id",frozen?.coreText("id")?:UUID.randomUUID().toString());put("title",title);put("text",body)};val next=if(frozen==null)messages+item else messages.map{if(it.coreText("id")==frozen.coreText("id"))item else it};save(withMessages(next)){creating=false;editing=null}})}
}
