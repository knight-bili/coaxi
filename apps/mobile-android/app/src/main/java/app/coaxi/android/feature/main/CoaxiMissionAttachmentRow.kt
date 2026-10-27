package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import app.coaxi.android.core.network.CoaxiOperation
import kotlinx.serialization.json.*

@Composable fun CoaxiMissionAttachmentRow(model:CoaxiWorkspaceViewModel,mission:JsonObject,attachment:JsonObject,onOpen:()->Unit){
    val state=model.state.collectAsState().value
    var removal by remember(mission.coreText("missionId"),attachment.coreText("id")){mutableStateOf<JsonObject?>(null)}
    Row(Modifier.fillMaxWidth()){
        TextButton(onClick=onOpen,modifier=Modifier.weight(1f)){Text(attachment.coreText("displayName"))}
        TextButton(enabled=state.pending==null&&!state.busy,onClick={removal=buildJsonObject{put("missionId",mission.coreText("missionId"));put("attachmentRefId",attachment.coreText("id"));put("expectedDetailsVersion",mission.coreLong("detailsVersion"))}}){Text(stringResource(R.string.coaxi_fusion_6135d4159e892541))}
    }
    removal?.let{frozen->AlertDialog(onDismissRequest={removal=null},title={Text(stringResource(R.string.coaxi_fusion_a1469f89f2860175))},text={Text("只移除“${attachment.coreText("displayName")}”在此使命中的引用；原文件和已发布消息保留。如果使命已被其他端修改，本次操作会拒绝覆盖。")},confirmButton={TextButton(enabled=state.pending==null&&!state.busy,onClick={model.command(CoaxiOperation.MISSIONS_ATTACHMENTS_REMOVE,frozen);removal=null}){Text(stringResource(R.string.coaxi_fusion_0f7f6afb91e8922f))}},dismissButton={TextButton(onClick={removal=null}){Text(stringResource(R.string.coaxi_fusion_2cd0f3be8738a86c))}})}
}
