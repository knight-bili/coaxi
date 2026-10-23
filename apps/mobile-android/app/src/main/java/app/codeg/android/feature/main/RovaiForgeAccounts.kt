package app.codeg.android.feature.main

import androidx.compose.ui.res.stringResource
import app.codeg.android.R

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import app.codeg.android.core.designsystem.component.GlassRow
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.*
import kotlinx.serialization.json.*
import java.util.UUID

/** Source Codeg Forge account dialog. Tokens remain ephemeral on the native client. */
@Composable fun RovaiForgeAccounts(model:RovaiWorkspaceViewModel,onChanged:()->Unit){
    var open by remember{mutableStateOf(false)}
    TextButton(onClick={open=true}){Text(stringResource(R.string.rovai_fusion_a6fcc32a34951651))}
    if(open)RovaiForgeAccountSettings(model,onClose={open=false;onChanged()})
}
@Composable private fun RovaiForgeAccountSettings(model:RovaiWorkspaceViewModel,onClose:()->Unit){
    val coroutine=rememberCoroutineScope();val generation=model.connectionGeneration
    var rows by remember{mutableStateOf<List<JsonObject>>(emptyList())};var selected by remember{mutableStateOf<JsonObject?>(null)};var creating by remember{mutableStateOf(false)}
    var id by remember{mutableStateOf("")};var provider by remember{mutableStateOf("github")};var base by remember{mutableStateOf("https://github.com")};var token by remember{mutableStateOf("")};var busy by remember{mutableStateOf(false)};var issue by remember{mutableStateOf<String?>(null)};var removing by remember{mutableStateOf<JsonObject?>(null)}
    fun edit(row:JsonObject?){selected=row;creating=row==null;id=row?.coreText("id")?:UUID.randomUUID().toString();provider=row?.coreText("provider")?:"github";base=row?.coreText("baseUrl")?:"https://github.com";token="";issue=null}
    LaunchedEffect(generation){busy=true;try{rows=model.query(RovaiOperation.FORGE_ACCOUNTS_GET).coreObject()["accounts"].coreArray()}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}
    fun save(remove:JsonObject?=null){if(busy)return;busy=true;issue=null;val capturedToken=token;coroutine.launch{try{val params=buildJsonObject{put("id",remove?.coreText("id")?:id);put("provider",remove?.coreText("provider")?:provider);put("baseUrl",remove?.coreText("baseUrl")?:base);if(remove!=null)put("remove",true)else if(capturedToken.isNotEmpty())put("token",capturedToken)};val result=model.query(RovaiOperation.FORGE_ACCOUNTS_SET,params).coreObject();require(generation==model.connectionGeneration){"工作区已切换"};rows=result["accounts"].coreArray();token="";selected=null;creating=false;removing=null}catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){issue=error.message}finally{busy=false}}}
    Dialog(onDismissRequest={if(!busy){token="";onClose()}},properties=DialogProperties(usePlatformDefaultWidth=false)){Surface(Modifier.fillMaxSize(),color=CodegTheme.colors.bgElevated){Column(Modifier.fillMaxSize().padding(16.dp),verticalArrangement=Arrangement.spacedBy(10.dp)){
        Row{Text(stringResource(R.string.rovai_fusion_a6fcc32a34951651),Modifier.weight(1f),style=MaterialTheme.typography.titleMedium);TextButton(enabled=!busy,onClick={token="";onClose()}){Text(stringResource(R.string.rovai_fusion_3fd47edce45b3603))}}
        Text(stringResource(R.string.rovai_fusion_c0f8bbd3f837b6f3),style=MaterialTheme.typography.bodySmall,color=CodegTheme.colors.textSecondary);issue?.let{Text(it,color=CodegTheme.colors.danger)}
        if(creating||selected!=null){Row{listOf("github","gitlab","gitea","forgejo").forEach{value->FilterChip(selected=provider==value,enabled=!busy&&creating,onClick={provider=value},label={Text(value)})}};OutlinedTextField(base,{base=it},enabled=!busy,label={Text(stringResource(R.string.rovai_fusion_9441b841e50a9a83))},modifier=Modifier.fillMaxWidth());OutlinedTextField(token,{token=it},enabled=!busy,label={Text(stringResource(R.string.rovai_fusion_7013328537a0f605))},modifier=Modifier.fillMaxWidth(),visualTransformation=PasswordVisualTransformation(),singleLine=true);Row{Button(enabled=!busy&&base.isNotBlank(),onClick={save()}){Text(stringResource(R.string.rovai_fusion_a3030bf8f16dc63c))};TextButton(enabled=!busy,onClick={token="";creating=false;selected=null}){Text(stringResource(R.string.rovai_fusion_2cd0f3be8738a86c))}}}
        else{TextButton(enabled=!busy,onClick={edit(null)}){Text(stringResource(R.string.rovai_fusion_91af6e57e7453fbe))};LazyColumn(Modifier.weight(1f),verticalArrangement=Arrangement.spacedBy(8.dp)){items(rows,key={it.coreText("id")}){row->GlassRow(Modifier.fillMaxWidth()){Text(row.coreText("provider"));Text(row.coreText("baseUrl"),color=CodegTheme.colors.textSecondary);Text(if(row["hasToken"]==JsonPrimitive(true))"已保存令牌" else "未设置令牌",style=MaterialTheme.typography.bodySmall);Row{TextButton(enabled=!busy,onClick={edit(row)}){Text(stringResource(R.string.rovai_fusion_051836569928a9f9))};TextButton(enabled=!busy,onClick={removing=row}){Text(stringResource(R.string.rovai_fusion_6135d4159e892541))}}}}}}
    }}}
    removing?.let{row->AlertDialog(onDismissRequest={if(!busy)removing=null},title={Text(stringResource(R.string.rovai_fusion_070fe8a42637907c))},text={Text(row.coreText("provider")+" · "+row.coreText("baseUrl"))},confirmButton={TextButton(enabled=!busy,onClick={save(row)}){Text(stringResource(R.string.rovai_fusion_6135d4159e892541))}},dismissButton={TextButton(enabled=!busy,onClick={removing=null}){Text(stringResource(R.string.rovai_fusion_2cd0f3be8738a86c))}})}
}
