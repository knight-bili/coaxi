package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

import android.net.Uri
import androidx.compose.material3.*
import androidx.compose.runtime.*
import kotlinx.serialization.json.*

/** canvas-file.ts: encode a literal path, never reinterpret #, ? or a :line suffix as a target. */
fun canvasFileSource(node:JsonObject):JsonObject{
    val data=node["data"].coreObject();val thread=data["fileReference"].coreObject().coreText("threadId");val path=data.coreText("path")
    require(data.coreText("kind")=="file"&&thread.isNotBlank()&&thread==thread.trim()&&thread.length<=128&&!thread.contains('\u0000')&&path.isNotEmpty()&&!path.contains('\u0000')){"文件节点尚未绑定原工程会话，请先选择来源"}
    val windows=Regex("^[A-Za-z]:[\\\\/]").containsMatchIn(path);val unc=path.startsWith("\\\\")||path.startsWith("//");val normalized=if(windows||unc)path.replace('\\','/')else path
    require(windows||unc||normalized.startsWith('/')){"文件节点必须保存绝对路径"}
    val encoded=normalized.split('/').mapIndexed{index,part->if(windows&&index==0)part else Uri.encode(part)}.joinToString("/")
    val reference=if(unc)encoded else "file://"+(if(windows)"/" else "")+encoded
    require(reference.length<=4096){"文件引用过长"}
    return buildJsonObject{put("kind","camp_workspace");put("threadId",thread);put("rawReference",reference)}
}

@Composable fun CoaxiCanvasFileNode(model:CoaxiWorkspaceViewModel,node:JsonObject,onClose:()->Unit){
    val value=remember(node){runCatching{canvasFileSource(node)}};val source=value.getOrNull()
    if(source==null)AlertDialog(onDismissRequest=onClose,title={Text(stringResource(R.string.coaxi_fusion_efd002826bc5437a))},text={Text(value.exceptionOrNull()?.message?:"原文件引用无法读取；卡片保留。")},confirmButton={TextButton(onClick=onClose){Text(stringResource(R.string.coaxi_fusion_3fd47edce45b3603))}})
    else if(coaxiOfficeFile(node["data"].coreObject().coreText("path")))CoaxiOfficePreview(model,source.coreText("threadId"),node["data"].coreObject().coreText("path"),onClose)
    else CoaxiFilePreview(model,source,onClose)
}
