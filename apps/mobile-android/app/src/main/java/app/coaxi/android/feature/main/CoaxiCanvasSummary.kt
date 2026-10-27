package app.coaxi.android.feature.main

import kotlinx.serialization.json.*

data class CoaxiCanvasSummary(val title:String,val kind:String,val model:String?,val modelTitle:String?,val project:String?,val branch:String?,val active:Boolean?,val available:Boolean?)
/** canvasSummaryMetadata: distinguish historical run model from configuration and trust only verified checkout facts. */
fun canvasSummary(reference:JsonObject,catalog:CoaxiCanvasCatalogSnapshot,profiles:List<JsonObject>,fallbackTitle:String=""):CoaxiCanvasSummary{
    val thread=catalog.rows.firstOrNull{it.coreText("threadId")==reference.coreText("threadId")};val privateId=(reference["conversationId"]as?JsonPrimitive)?.contentOrNull
    val private=thread?.get("privateConversations").coreArray().firstOrNull{it.coreText("conversationId")==privateId};val members=thread?.get("members").coreArray().filter{privateId==null||it.coreText("agentId")==private?.coreText("agentId")}
    data class Model(val name:String,val model:String,val historical:Boolean)
    val models=members.mapNotNull{member->val historical=(if(privateId==null)member["lastRunModel"]else private?.get("lastRunModel")).coreObject().coreText("modelId");if(historical.isNotEmpty())Model(member.coreText("displayName"),historical,true)else{
        val profile=profiles.firstOrNull{it.coreText("agentId")==member.coreText("agentId")&&it.coreText("presence")!="removed"&&(it["removedAt"]==null||it["removedAt"]==JsonNull)};val config=profile?.get("runtimeConfiguration").coreObject();val model=config["model"].coreObject()
        if(config.coreText("adapterKind")==member.coreText("runtimeKind")&&model.coreText("mode")=="explicit"&&model.coreText("modelId").isNotEmpty())Model(member.coreText("displayName"),model.coreText("modelId"),false)else null
    }}
    val label=listOf(true,false).mapNotNull{historical->val values=models.filter{it.historical==historical}.map{it.model}.distinct();if(values.isEmpty())null else (if(historical)"最近："else"配置：")+values.joinToString(" · ")}.joinToString(" · ").takeIf{it.isNotEmpty()}
    val modelTitle=models.joinToString("；"){(if(it.historical)"最近运行模型："else"模型配置：")+it.name+": "+it.model}.takeIf{it.isNotEmpty()}
    val project=thread?.takeIf{it.coreText("projectBindingKind")=="directory"}?.coreText("projectPath")
    var branch:String?=null
    if(project!=null){val exact=catalog.folders[project];val candidates=if(exact!=null)listOf(exact)else catalog.folders.values.filter{it.worktrees.any{row->row.coreText("projectPath")==project}};val branches=candidates.filter{it.complete}.mapNotNull{relation->val path=if(relation===exact)relation.projectPath else project;relation.worktrees.firstOrNull{it.coreText("projectPath")==path&&it["available"]==JsonPrimitive(true)&&it["bare"]==JsonPrimitive(false)&&it["prunable"]==JsonPrimitive(false)&&it["detached"]!=JsonPrimitive(true)}?.coreText("branch")?.takeIf{it.isNotEmpty()}}.distinct();if(branches.size==1)branch=branches.first()}
    val active=if(privateId!=null)(private?.get("hasActiveRun")as?JsonPrimitive)?.booleanOrNull else (thread?.get("publicActiveRunCount")as?JsonPrimitive)?.longOrNull?.let{it>0}
    val available=if(thread!=null&&(privateId==null||private!=null))true else if(catalog.complete)false else null
    val title=thread?.let{(if(it.coreText("missionId").isEmpty())""else"使命：")+it.coreText("title")}.orEmpty().ifEmpty{fallbackTitle.ifEmpty{if(catalog.complete)"对话已结束或不可用"else"尚未核对的对话"}}
    return CoaxiCanvasSummary(title,if(privateId==null)"公开对话"else"私有对话",label,modelTitle,project,branch,active,available)
}
