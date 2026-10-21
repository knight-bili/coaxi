package app.codeg.android.feature.main

import app.codeg.android.core.network.RovaiOperation
import kotlinx.serialization.json.*

data class RovaiCanvasReferenceCursor(val after:String,val upper:String,val minimumSequence:Long)
data class RovaiCanvasReferencePage(val items:List<JsonObject>,val next:RovaiCanvasReferenceCursor?,val sequence:Long)

/** canvas-reference-controller.ts: bounded metadata pages, one frozen upper identity, no transcript or draft reads. */
suspend fun readCanvasReferencePage(model:RovaiWorkspaceViewModel,cursor:RovaiCanvasReferenceCursor?=null):RovaiCanvasReferencePage{
    val generation=model.connectionGeneration
    val value=model.query(RovaiOperation.CANVAS_REFERENCES_PAGE,buildJsonObject{put("afterThreadId",cursor?.after?.let(::JsonPrimitive)?:JsonNull);put("upperThreadId",cursor?.upper?.let(::JsonPrimitive)?:JsonNull);put("limit",200)}).coreObject()
    require(generation==model.connectionGeneration){"工作区已切换"}
    val sequence=(value["throughGlobalSequence"]as?JsonPrimitive)?.takeUnless{it.isString}?.longOrNull
    val upper=(value["upperThreadId"]as?JsonPrimitive)?.contentOrNull
    val next=(value["nextAfterThreadId"]as?JsonPrimitive)?.contentOrNull
    val items=value["items"]as?JsonArray?:error("对话目录列表格式不受支持")
    require(value["schemaVersion"]==JsonPrimitive(1)&&sequence!=null&&sequence>=0&&sequence>=(cursor?.minimumSequence?:0)&&items.size<=200&&(upper==null||upper.isNotBlank())&&(cursor==null||upper==cursor.upper)){"对话目录版本或分页范围不一致，请重新读取"}
    var previous=cursor?.after.orEmpty()
    val rows=items.map{item->val row=item as?JsonObject?:error("对话目录项目格式不受支持");val id=row.coreText("threadId")
        require(id>previous&&upper!=null&&id<=upper&&row["title"]is JsonPrimitive&&row["projectPath"]is JsonPrimitive){"对话目录分页未前进或归属不一致"}
        val members=row["members"]as?JsonArray?:error("对话成员目录无法读取");val privateRows=row["privateConversations"]as?JsonArray?:error("私聊目录无法读取")
        val memberIds=members.map{it.coreObject().coreText("agentId")}.toSet();require(memberIds.none{it.isEmpty()}){"对话成员标识缺失"}
        fun validModel(value:JsonElement?):Boolean{if(value==null||value==JsonNull)return true;val model=value as?JsonObject?:return false;return model.coreText("runId").isNotBlank()&&model.coreText("adapterKind").isNotBlank()&&(model["modelId"]==JsonNull||model.coreText("modelId").isNotBlank())&&(model["reasoningEffort"]==JsonNull||(model["reasoningEffort"]as?JsonPrimitive)?.isString==true)}
        require(members.all{it is JsonObject&&validModel(it["lastRunModel"])}){"会话模型来源无法读取"}
        require(privateRows.all{privateValue->val private=privateValue as?JsonObject;private!=null&&private.coreText("conversationId").isNotBlank()&&private.coreText("agentId")in memberIds&&(private["hasActiveRun"]==null||private["hasActiveRun"]==JsonPrimitive(true)||private["hasActiveRun"]==JsonPrimitive(false))&&validModel(private["lastRunModel"])}){"私聊与成员归属不一致"}
        row["publicActiveRunCount"]?.let{count->require((count as?JsonPrimitive)?.takeUnless{it.isString}?.longOrNull?.let{it>=0}==true){"公开会话运行数量无效"}}
        previous=id;row
    }
    require(next==null||rows.isNotEmpty()&&next==previous&&next!=cursor?.after){"对话目录分页未前进"}
    return RovaiCanvasReferencePage(rows,if(next==null)null else RovaiCanvasReferenceCursor(next,upper?:error("对话目录范围缺失"),sequence),sequence)
}
