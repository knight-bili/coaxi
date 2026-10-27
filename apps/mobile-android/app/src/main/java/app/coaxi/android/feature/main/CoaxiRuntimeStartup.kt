package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import app.coaxi.android.core.designsystem.theme.CoaxiTheme
import app.coaxi.android.core.network.CoaxiOperation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.serialization.json.*
import java.util.UUID

private data class NativeEnvironment(val id:String=UUID.randomUUID().toString(),val name:String="",val value:String="")
/** Same field-level before/after merge as Coaxi's startup editor; all paths name Host files. */
@Composable
fun CoaxiRuntimeStartup(model:CoaxiWorkspaceViewModel,runtimeKind:String,onClose:()->Unit){
    val coroutine=rememberCoroutineScope();var saved by remember{mutableStateOf<JsonObject?>(null)};var path by remember{mutableStateOf("")};var rows by remember{mutableStateOf<List<NativeEnvironment>>(emptyList())}
    var conflicts by remember{mutableStateOf<List<JsonObject>>(emptyList())};var issue by remember{mutableStateOf<String?>(null)};var inspection by remember{mutableStateOf<JsonObject?>(null)};var busy by remember{mutableStateOf(false)};var discard by remember{mutableStateOf(false)}
    fun configuration()=buildJsonObject{put("programPath",path.trim().takeIf{it.isNotEmpty()}?.let(::JsonPrimitive)?:JsonNull);put("environment",JsonArray(rows.map{row->buildJsonObject{put("name",row.name.trim());put("value",row.value)}}))}
    fun accept(configuration:JsonObject){path=configuration.coreText("programPath");rows=configuration["environment"].coreArray().map{NativeEnvironment(name=it.coreText("name"),value=it.coreText("value"))}}
    fun environment(configuration:JsonObject)=configuration["environment"].coreArray().associate{it.coreText("name") to (it["value"]?:JsonNull)}
    fun edits():List<JsonObject>{val before=saved?.get("configuration").coreObject();val after=configuration();val values=mutableListOf<JsonObject>();fun add(parts:List<String>,old:JsonElement,next:JsonElement,label:String){if(old!=next)values.add(buildJsonObject{put("path",JsonArray(parts.map(::JsonPrimitive)));put("before",old);put("after",next);put("label",label)})};add(listOf("programPath"),before["programPath"]?:JsonNull,after["programPath"]?:JsonNull,"程序路径");val old=environment(before);val next=environment(after);(old.keys+next.keys).forEach{name->add(listOf("environment",name),old[name]?:JsonNull,next[name]?:JsonNull,"环境变量 $name")};return values}
    fun apply(configuration:JsonObject,changes:List<JsonObject>):JsonObject{var program=configuration["programPath"]?:JsonNull;val env=environment(configuration).toMutableMap();changes.forEach{edit->val parts=(edit["path"] as JsonArray).map{it.jsonPrimitive.content};val value=edit["after"]?:JsonNull;if(parts.first()=="programPath")program=value else if(value==JsonNull)env.remove(parts[1])else env[parts[1]]=value};return buildJsonObject{put("programPath",program);put("environment",JsonArray(env.map{(name,value)->buildJsonObject{put("name",name);put("value",value)}}))}}
    val dirty=saved!=null&&edits().isNotEmpty()
    LaunchedEffect(runtimeKind){busy=true;try{val result=model.query(CoaxiOperation.RUNTIME_STARTUP_GET,buildJsonObject{put("runtimeKind",runtimeKind)}).coreObject();saved=result;accept(result["configuration"].coreObject())}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    fun inspect(deep:Boolean){coroutine.launch{busy=true;issue=null;try{inspection=model.query(if(deep)CoaxiOperation.RUNTIME_STARTUP_CHECK else CoaxiOperation.RUNTIME_STARTUP_INSPECT,buildJsonObject{put("runtimeKind",runtimeKind);put("configuration",configuration())}).coreObject()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}
    AlertDialog(onDismissRequest={if(!busy){if(dirty)discard=true else onClose()}},title={Text(stringResource(R.string.coaxi_fusion_df48d4204fd4bf93))},text={LazyColumn(verticalArrangement=Arrangement.spacedBy(10.dp)){
        item{Text(runtimeKind,style=MaterialTheme.typography.titleMedium);Text(stringResource(R.string.coaxi_fusion_4b30367ac1ceafeb),color=CoaxiTheme.colors.textSecondary)}
        issue?.let{item{Text(it,color=CoaxiTheme.colors.danger)}}
        item{OutlinedTextField(value=path,onValueChange={path=it},label={Text(stringResource(R.string.coaxi_fusion_466898e183350178))},enabled=!busy)}
        items(rows,key={it.id}){row->Column(verticalArrangement=Arrangement.spacedBy(6.dp)){OutlinedTextField(value=row.name,onValueChange={name->rows=rows.map{if(it.id==row.id)it.copy(name=name)else it}},label={Text(stringResource(R.string.coaxi_fusion_9d9d763731b4b95f))},enabled=!busy);OutlinedTextField(value=row.value,onValueChange={value->rows=rows.map{if(it.id==row.id)it.copy(value=value)else it}},label={Text(stringResource(R.string.coaxi_fusion_32326cee3f56130b))},visualTransformation=PasswordVisualTransformation(),enabled=!busy);TextButton(enabled=!busy,onClick={rows=rows.filter{it.id!=row.id}}){Text(stringResource(R.string.coaxi_fusion_3a7774984d5c278c))}}}
        item{TextButton(enabled=!busy&&rows.size<256,onClick={rows=rows+NativeEnvironment()}){Text(stringResource(R.string.coaxi_fusion_b2e086a495866505))}}
        items(conflicts,key={it["path"].toString()}){conflict->Column{Text(conflict.coreText("label")+"已在外部更新。",color=CoaxiTheme.colors.danger);Row{TextButton(enabled=!busy,onClick={conflicts=conflicts.filter{it!==conflict}}){Text(stringResource(R.string.coaxi_fusion_a4ed6a23f404caf5))};TextButton(enabled=!busy,onClick={accept(apply(configuration(),listOf(buildJsonObject{put("path",conflict["path"]!!);put("after",conflict["current"]?:JsonNull)})));conflicts=conflicts.filter{it!==conflict}}){Text(stringResource(R.string.coaxi_fusion_9275a746cba3278a))}}}}
        item{Row{TextButton(enabled=!busy,onClick={inspect(false)}){Text(stringResource(R.string.coaxi_fusion_9fd4c2a3a12702cc))};TextButton(enabled=!busy,onClick={inspect(true)}){Text(stringResource(R.string.coaxi_fusion_48bb796777f58c6b))}}}
        inspection?.let{result->item{Text(when(result.coreText("status")){"missing"->"未检测到程序";"recognized"->"已识别程序";"version_unverified"->"已识别程序，版本检查未完成";"authentication_required"->"已识别程序，需要登录";"ready"->"草稿检查通过";else->"草稿检查未通过"});Text(result.coreText("executablePath"),style=MaterialTheme.typography.bodySmall)}}
    }},confirmButton={TextButton(enabled=!busy&&dirty&&conflicts.isEmpty(),onClick={
        val names=rows.map{it.name.trim()};if(names.toSet().size!=names.size||names.any{!Regex("[A-Za-z_][A-Za-z0-9_]{0,255}").matches(it)}){issue="变量名需要唯一，只含字母、数字、下划线，且不能以数字开头。";return@TextButton}
        val changes=edits();coroutine.launch{busy=true;issue=null;try{val result=model.query(CoaxiOperation.RUNTIME_STARTUP_SAVE,buildJsonObject{put("runtimeKind",runtimeKind);put("edits",JsonArray(changes))}).coreObject();if(result.coreText("status")=="conflict"){saved=result["latest"].coreObject();accept(apply(saved!!["configuration"].coreObject(),changes));conflicts=result["conflicts"].coreArray()}else{saved=result;accept(result["configuration"].coreObject());conflicts=emptyList()}}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=if(error is app.coaxi.android.core.network.CoaxiHostFailure&&!error.unknownResult)error.message else "保存结果尚未确认，草稿已保留。请核对Host上的设置后重试。"}finally{busy=false}}
    }){Text(stringResource(R.string.coaxi_fusion_a3030bf8f16dc63c))}},dismissButton={TextButton(enabled=!busy,onClick={if(dirty)discard=true else onClose()}){Text(stringResource(R.string.coaxi_fusion_3fd47edce45b3603))}})
    if(discard)AlertDialog(onDismissRequest={discard=false},title={Text(stringResource(R.string.coaxi_fusion_27369606d2939a05))},confirmButton={TextButton(onClick=onClose){Text(stringResource(R.string.coaxi_fusion_e7f51b070effdbe9))}},dismissButton={TextButton(onClick={discard=false}){Text(stringResource(R.string.coaxi_fusion_fd4b9e3b6c685bae))}})
}
