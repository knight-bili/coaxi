package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.*
import kotlinx.serialization.json.JsonObject

private data class FrozenUploadPicker(val threadId:String,val target:JsonObject,val revision:Long,val hostGeneration:Long)
@Composable
fun CoaxiAttachmentPicker(model:CoaxiWorkspaceViewModel,threadId:String,target:JsonObject,revision:Long,disabled:Boolean){
    var frozen by remember{mutableStateOf<FrozenUploadPicker?>(null)}
    val picker=rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()){uri->val request=frozen;frozen=null;if(uri!=null&&request!=null)model.upload(uri,request.threadId,request.target,request.revision,request.hostGeneration)}
    TextButton(enabled=!disabled&&frozen==null,onClick={frozen=FrozenUploadPicker(threadId,target,revision,model.connectionGeneration);picker.launch(arrayOf("*/*"))}){Text(stringResource(R.string.coaxi_fusion_c8a47d0018480abe))}
}
