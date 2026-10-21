package app.codeg.android.feature.main

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.network.RovaiOperation
import kotlinx.serialization.json.*
import java.util.UUID

/** Rovai identity fields in the original native editor form; version is frozen on opening. */
@Composable
fun RovaiMemberEditor(model:RovaiWorkspaceViewModel,member:JsonObject?,onDismiss:()->Unit){
    val state by model.state.collectAsStateWithLifecycle();val fields=listOf("displayName" to "名称","teamRole" to "团队角色","professionalResponsibilities" to "专业职责","workingPrinciples" to "工作准则","growthTopic" to "成长课题")
    val initial=remember(member?.coreText("agentId")){fields.associate{it.first to member?.coreText(it.first).orEmpty()}}
    var values by remember(member?.coreText("agentId")){mutableStateOf(initial)}
    val originalTraits=remember(member?.coreText("agentId")){(member?.get("personalityTraits") as? JsonArray)?.mapNotNull{(it as? JsonPrimitive)?.contentOrNull}?.joinToString("，").orEmpty()}
    var traits by remember(member?.coreText("agentId")){mutableStateOf(originalTraits)};var issue by remember{mutableStateOf<String?>(null)};var discard by remember{mutableStateOf(false)};var submitted by remember{mutableStateOf<String?>(null)}
    val dirty=values!=initial||traits!=originalTraits;val disabled=state.busy||state.pending!=null
    val changed=member!=null&&state.members.find{it.coreText("agentId")==member.coreText("agentId")}?.coreLong("version")?.let{it!=member.coreLong("version")}==true
    fun close(){if(dirty)discard=true else onDismiss()}
    LaunchedEffect(state.lastResult){val receipt=state.lastResult.coreObject()["commandResult"].coreObject().ifEmpty{state.lastResult.coreObject()};if(receipt.coreText("commandId")==submitted){if(receipt.coreText("status")=="applied")onDismiss()else issue=receipt.coreText("code")}}
    AlertDialog(onDismissRequest={if(!disabled)close()},title={Text(if(member==null)"创建队员" else "队员信息")},text={LazyColumn(verticalArrangement=Arrangement.spacedBy(10.dp)){
        if(changed)item{Text("信息已在其他位置更新，当前草稿保留；请关闭后重新打开再编辑。",color=CodegTheme.colors.danger)}
        issue?.let{item{Text(it,color=CodegTheme.colors.danger)}}
        fields.forEach{(key,title)->item{OutlinedTextField(value=values[key].orEmpty(),onValueChange={values=values+(key to it)},label={Text(title)},maxLines=if(key in listOf("displayName","teamRole"))1 else 4,enabled=!disabled)}}
        item{OutlinedTextField(value=traits,onValueChange={traits=it},label={Text("性格底色（逗号分隔，最多6项）")},maxLines=3,enabled=!disabled)}
    }},confirmButton={TextButton(enabled=!disabled&&!changed&&values["displayName"].orEmpty().isNotBlank()&&(member==null||dirty),onClick={
        val limits=mapOf("displayName" to 80,"teamRole" to 120,"professionalResponsibilities" to 300,"workingPrinciples" to 300,"growthTopic" to 300)
        val tags=traits.split(',', '，').map{it.trim()}.filter{it.isNotEmpty()}.distinct()
        val invalid=fields.firstOrNull{(key,_)->val text=values[key].orEmpty();text.codePointCount(0,text.length)>(limits[key]?:300)}
        if(invalid!=null)issue="${invalid.second}超过${limits[invalid.first]}个字符" else if(tags.size>6||tags.any{it.codePointCount(0,it.length)>16||it.any{character->character.isISOControl()}})issue="性格标签最多6项，每项最多16个字符，不能包含控制字符。"
        else{val id=UUID.randomUUID().toString();submitted=id;model.execute(if(member==null)RovaiOperation.MEMBERS_CREATE else RovaiOperation.MEMBERS_UPDATE,buildJsonObject{put("commandId",id);putJsonObject("command"){values.forEach{(key,value)->put(key,value.trim())};put("personalityTraits",JsonArray(tags.map(::JsonPrimitive)));if(member==null)put("avatarRef",JsonNull)else{put("agentId",member.coreText("agentId"));put("expectedVersion",member.coreLong("version"))}}})}
    }){Text(if(member==null)"创建队员" else "保存")}},dismissButton={TextButton(enabled=!disabled,onClick={close()}){Text("关闭")}})
    if(discard)AlertDialog(onDismissRequest={discard=false},title={Text("放弃未保存的队员信息？")},confirmButton={TextButton(onClick={onDismiss()}){Text("放弃")}},dismissButton={TextButton(onClick={discard=false}){Text("继续编辑")}})
}
