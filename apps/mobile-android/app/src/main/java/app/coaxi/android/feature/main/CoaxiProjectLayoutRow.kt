package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.coaxi.android.core.designsystem.theme.CoaxiTheme
import kotlinx.serialization.json.*

/** Native target controls for the original per-container project layout; group nesting is not offered. */
@Composable fun CoaxiProjectLayoutRow(entry:CoaxiProjectLayoutItem,layout:CoaxiProjectLayout,groups:List<JsonObject>,folders:Map<String,CoaxiCanvasFolderRelation>,blocked:Boolean,onMove:(CoaxiProjectLayoutItem,CoaxiProjectLayoutLocation)->Unit){
    val location=layout.locate(entry)?:return;val count=if(location.groupId==null)layout.top.size else layout.members[location.groupId].orEmpty().size;val project=entry.kind=="project";val relation=folders[entry.target];val disabled=blocked||project&&(relation?.complete!=true||relation.parentPath!=null)
    Column(Modifier.fillMaxWidth().padding(start=if(location.groupId==null)0.dp else 20.dp)){
        Row(Modifier.fillMaxWidth(),horizontalArrangement=Arrangement.spacedBy(6.dp)){
            Text(if(project)entry.target else groups.firstOrNull{it.coreText("id")==entry.target}?.coreText("name")?:"原分组暂不可用",Modifier.weight(1f),maxLines=2,overflow=TextOverflow.Ellipsis,style=if(project)MaterialTheme.typography.bodySmall else MaterialTheme.typography.titleSmall)
            TextButton(enabled=!disabled&&location.index>0,onClick={onMove(entry,location.copy(index=location.index-1))}){Text(stringResource(R.string.coaxi_fusion_f853a70b120418be))}
            TextButton(enabled=!disabled&&location.index<count-1,onClick={onMove(entry,location.copy(index=location.index+1))}){Text(stringResource(R.string.coaxi_fusion_e75e8b4e5c971227))}
        }
        if(project){CoaxiCanvasChoice("所属分组",location.groupId.orEmpty(),listOf("" to "顶层")+groups.map{it.coreText("id") to it.coreText("name")},!disabled){target->if(target!=location.groupId.orEmpty())onMove(entry,CoaxiProjectLayoutLocation(target.takeIf{it.isNotEmpty()},if(target.isEmpty())layout.top.size else layout.members[target].orEmpty().size))};if(relation?.parentPath!=null)Text(stringResource(R.string.coaxi_fusion_82fe0ebc9fe245c1),style=MaterialTheme.typography.labelSmall,color=CoaxiTheme.colors.textSecondary)}
    }
}
