package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

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
    var aliasEditor by remember{mutableStateOf<Int?>(null)};var aliasCreating by remember{mutableStateOf(false)}
    fun args(next:JsonObject?=null)=buildJsonObject{put("level",level);threadId?.let{putJsonObject("scope"){put("threadId",it)}};if(next!=null){put("expectedDigest",digest);put("value",next)}}
    fun save(next:JsonObject,onSaved:()->Unit={}){if(busy||model.state.value.pending!=null)return;val frozen=args(next);busy=true;issue=null;scope.launch{try{val result=model.query(RovaiOperation.WORKBENCH_ORGANIZER_SET,frozen).coreObject();if(generation==model.connectionGeneration){value=result["value"].coreObject();digest=result.coreText("digest");onSaved()}}catch(cancelled:kotlinx.coroutines.CancellationException){throw cancelled}catch(error:Exception){if(generation==model.connectionGeneration)issue=error.message}finally{busy=false}}}
    fun withMessages(messages:List<JsonObject>)=buildJsonObject{value?.forEach{(key,item)->put(key,item)};put("quickMessages",JsonArray(messages))}
    LaunchedEffect(level,generation){busy=true;value=null;issue=null;try{val result=model.query(RovaiOperation.WORKBENCH_ORGANIZER_GET,args()).coreObject();if(generation==model.connectionGeneration){value=result["value"].coreObject();digest=result.coreText("digest")}}catch(cancelled:kotlinx.coroutines.CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    val aliases=value?.get("directoryAliases").coreArray()
    fun withAliases(rows:List<JsonObject>)=buildJsonObject{value?.forEach{(key,item)->put(key,item)};put("directoryAliases",JsonArray(rows))}
    val messages=value?.get("quickMessages").coreArray();val tags=(value?.get("tags")as?JsonArray)?.mapNotNull{(it as?JsonPrimitive)?.contentOrNull}?:emptyList()
    Column(Modifier.fillMaxSize()){
        Row(Modifier.fillMaxWidth().padding(12.dp),horizontalArrangement=Arrangement.spacedBy(8.dp)){FilterChip(selected=level=="global",enabled=!busy&&!creating&&editing==null&&!aliasCreating&&aliasEditor==null,onClick={level="global"},label={Text(stringResource(R.string.rovai_fusion_63d6b47116de363c))});if(threadId!=null){FilterChip(selected=level=="project",enabled=!busy&&!creating&&editing==null&&!aliasCreating&&aliasEditor==null,onClick={level="project"},label={Text(stringResource(R.string.rovai_fusion_1e78be389d98be1c))});FilterChip(selected=level=="thread",enabled=!busy&&!creating&&editing==null&&!aliasCreating&&aliasEditor==null,onClick={level="thread"},label={Text(stringResource(R.string.rovai_fusion_b0ad5f9df7437a33))})};Spacer(Modifier.weight(1f));IconButton(enabled=value!=null&&!busy,onClick={creating=true}){Icon(Icons.Outlined.Add,"新建快捷消息")}}
        issue?.let{Text(it,Modifier.padding(horizontal=12.dp),color=CodegTheme.colors.danger)}
        LazyColumn(Modifier.weight(1f),contentPadding=PaddingValues(12.dp),verticalArrangement=Arrangement.spacedBy(8.dp)){
            item{Text(stringResource(R.string.rovai_fusion_bb50dc4a5d875e8e),style=MaterialTheme.typography.titleMedium);Text(stringResource(R.string.rovai_fusion_e84cc89064038901),color=CodegTheme.colors.textSecondary)}
            itemsIndexed(messages,key={_,item->item.coreText("id")}){index,message->GlassRow(Modifier.fillMaxWidth()){
                Text(message.coreText("title"),style=MaterialTheme.typography.titleSmall);Text(message.coreText("text"),maxLines=3,color=CodegTheme.colors.textSecondary)
                Row(horizontalArrangement=Arrangement.spacedBy(8.dp)){if(onUse!=null)TextButton(enabled=!busy,onClick={if(!onUse(message.coreText("text")))issue="当前输入暂时不能插入，请先核对原操作。"}){Text(stringResource(R.string.rovai_fusion_83b5eb109dea6768))};TextButton(enabled=!busy,onClick={editing=message}){Text(stringResource(R.string.rovai_fusion_051836569928a9f9))};IconButton(enabled=!busy&&index>0,onClick={val next=messages.toMutableList();next[index]=next[index-1].also{next[index-1]=message};save(withMessages(next))}){Icon(Icons.Outlined.ArrowUpward,"上移")};IconButton(enabled=!busy&&index<messages.lastIndex,onClick={val next=messages.toMutableList();next[index]=next[index+1].also{next[index+1]=message};save(withMessages(next))}){Icon(Icons.Outlined.ArrowDownward,"下移")};IconButton(enabled=!busy,onClick={save(withMessages(messages.filter{it.coreText("id")!=message.coreText("id")}))}){Icon(Icons.Outlined.DeleteOutline,"删除")}}
            }}
            item{Text(stringResource(R.string.rovai_fusion_5e82bd99e1fa691b),style=MaterialTheme.typography.titleMedium);Text(stringResource(R.string.rovai_fusion_efeed8c9d9e4fd7f),color=CodegTheme.colors.textSecondary)}
            itemsIndexed(aliases,key={index,_->"alias:$index"}){index,alias->GlassRow(Modifier.fillMaxWidth()){
                Text(alias.coreText("name").ifBlank{alias.coreText("path")},style=MaterialTheme.typography.titleSmall);Text(alias.coreText("path"),color=CodegTheme.colors.textSecondary);if(alias.coreText("group").isNotEmpty())Text(alias.coreText("group"),style=MaterialTheme.typography.labelSmall)
                Row{TextButton(enabled=!busy,onClick={aliasEditor=index}){Text(stringResource(R.string.rovai_fusion_051836569928a9f9))};IconButton(enabled=!busy&&index>0,onClick={val next=aliases.toMutableList();next[index]=next[index-1].also{next[index-1]=alias};save(withAliases(next))}){Icon(Icons.Outlined.ArrowUpward,"上移目录")};IconButton(enabled=!busy&&index<aliases.lastIndex,onClick={val next=aliases.toMutableList();next[index]=next[index+1].also{next[index+1]=alias};save(withAliases(next))}){Icon(Icons.Outlined.ArrowDownward,"下移目录")};IconButton(enabled=!busy,onClick={save(withAliases(aliases.filterIndexed{i,_->i!=index}))}){Icon(Icons.Outlined.DeleteOutline,"移除关联目录")}}
            }}
            item{TextButton(enabled=!busy&&value!=null,onClick={aliasCreating=true}){Icon(Icons.Outlined.Add,null);Text(stringResource(R.string.rovai_fusion_3e0c695339e871bd))}}
            item{Text(stringResource(R.string.rovai_fusion_1d0fd5f9336d9103),style=MaterialTheme.typography.titleMedium);tags.forEach{label->Row{Text(label,Modifier.weight(1f));IconButton(enabled=!busy,onClick={save(buildJsonObject{value?.forEach{(key,item)->put(key,item)};put("tags",JsonArray(tags.filter{it!=label}.map(::JsonPrimitive)))})}){Icon(Icons.Outlined.Close,"移除标签")}}};Row{OutlinedTextField(tag,{tag=it},enabled=!busy,modifier=Modifier.weight(1f),label={Text(stringResource(R.string.rovai_fusion_7b545b05f46129d5))});TextButton(enabled=value!=null&&!busy&&tag.isNotBlank()&&tag.trim() !in tags,onClick={save(buildJsonObject{value?.forEach{(key,item)->put(key,item)};put("tags",JsonArray((tags+tag.trim()).map(::JsonPrimitive)))}){tag=""}}){Text(stringResource(R.string.rovai_fusion_7a8a11ead50742a2))}}}
        }
    }
    if(creating||editing!=null){val frozen=editing;QuickMessageEditor(existing=frozen?.let{QuickMessage(0,it.coreText("title"),it.coreText("text"),0)},onDismiss={if(!busy){creating=false;editing=null}},busy=busy,error=issue,onSave={title,body->val item=buildJsonObject{put("id",frozen?.coreText("id")?:UUID.randomUUID().toString());put("title",title);put("text",body)};val next=if(frozen==null)messages+item else messages.map{if(it.coreText("id")==frozen.coreText("id"))item else it};save(withMessages(next)){creating=false;editing=null}})}
    if(aliasCreating||aliasEditor!=null){val index=aliasEditor;val original=index?.let{aliases.getOrNull(it)};RovaiDirectoryAliasEditor(original,busy,issue,onDismiss={if(!busy){aliasCreating=false;aliasEditor=null}},onSave={path,name,group->val next=buildJsonObject{put("path",path);put("name",name);put("group",group)};save(withAliases(if(index==null)aliases+next else aliases.mapIndexed{i,old->if(i==index)next else old})){aliasCreating=false;aliasEditor=null}})}

}

@Composable private fun RovaiDirectoryAliasEditor(value:JsonObject?,busy:Boolean,error:String?,onDismiss:()->Unit,onSave:(String,String,String)->Unit){
    var path by remember(value){mutableStateOf(value?.coreText("path").orEmpty())};var name by remember(value){mutableStateOf(value?.coreText("name").orEmpty())};var group by remember(value){mutableStateOf(value?.coreText("group").orEmpty())};var discard by remember{mutableStateOf(false)}
    val dirty=path!=value?.coreText("path").orEmpty()||name!=value?.coreText("name").orEmpty()||group!=value?.coreText("group").orEmpty()
    fun close(){if(!busy){if(dirty)discard=true else onDismiss()}}
    AlertDialog(onDismissRequest={close()},title={Text(if(value==null)"添加关联目录" else "编辑关联目录")},text={Column(verticalArrangement=Arrangement.spacedBy(10.dp)){error?.let{Text(it,color=CodegTheme.colors.danger)};OutlinedTextField(path,{path=it},enabled=!busy,label={Text(stringResource(R.string.rovai_fusion_ea82e7802a945b4f))},singleLine=true);OutlinedTextField(name,{name=it},enabled=!busy,label={Text(stringResource(R.string.rovai_fusion_55c4cb3d74643b57))},singleLine=true);OutlinedTextField(group,{group=it},enabled=!busy,label={Text(stringResource(R.string.rovai_fusion_d4163ce3b0784049))},singleLine=true)}},confirmButton={TextButton(enabled=!busy&&path.isNotBlank(),onClick={onSave(path.trim(),name.trim(),group.trim())}){Text(stringResource(R.string.rovai_fusion_a3030bf8f16dc63c))}},dismissButton={TextButton(enabled=!busy,onClick={close()}){Text(stringResource(R.string.rovai_fusion_2cd0f3be8738a86c))}})
    if(discard)AlertDialog(onDismissRequest={discard=false},title={Text(stringResource(R.string.rovai_fusion_9a99dd7b3f05aa2d))},confirmButton={TextButton(onClick={discard=false;onDismiss()}){Text(stringResource(R.string.rovai_fusion_e7f51b070effdbe9))}},dismissButton={TextButton(onClick={discard=false}){Text(stringResource(R.string.rovai_fusion_fd4b9e3b6c685bae))}})
}
