package app.codeg.android.feature.main

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.weight
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import app.codeg.android.core.network.RovaiOperation
import kotlinx.serialization.json.*

@Composable fun RovaiMissionAttachmentRow(model:RovaiWorkspaceViewModel,mission:JsonObject,attachment:JsonObject,onOpen:()->Unit){
    val state=model.state.collectAsState().value
    var removal by remember(mission.coreText("missionId"),attachment.coreText("id")){mutableStateOf<JsonObject?>(null)}
    Row(Modifier.fillMaxWidth()){
        TextButton(onClick=onOpen,modifier=Modifier.weight(1f)){Text(attachment.coreText("displayName"))}
        TextButton(enabled=state.pending==null&&!state.busy,onClick={removal=buildJsonObject{put("missionId",mission.coreText("missionId"));put("attachmentRefId",attachment.coreText("id"));put("expectedDetailsVersion",mission.coreLong("detailsVersion"))}}){Text("移除")}
    }
    removal?.let{frozen->AlertDialog(onDismissRequest={removal=null},title={Text("移除使命附件？")},text={Text("只移除“${attachment.coreText("displayName")}”在此使命中的引用；原文件和已发布消息保留。如果使命已被其他端修改，本次操作会拒绝覆盖。")},confirmButton={TextButton(enabled=state.pending==null&&!state.busy,onClick={model.command(RovaiOperation.MISSIONS_ATTACHMENTS_REMOVE,frozen);removal=null}){Text("移除引用")}},dismissButton={TextButton(onClick={removal=null}){Text("取消")}})}
}
