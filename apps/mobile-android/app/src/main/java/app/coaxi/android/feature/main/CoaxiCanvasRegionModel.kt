package app.coaxi.android.feature.main

import app.coaxi.android.core.network.CoaxiOperation
import kotlinx.coroutines.*
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withPermit
import kotlinx.serialization.json.*
import java.time.Instant

data class CoaxiCanvasFolderRelation(val paths:List<String>,val complete:Boolean,val parentPath:String?=null,val issue:String?=null,val projectPath:String?=null,val worktrees:List<JsonObject> = emptyList())
data class CoaxiCanvasRegionMember(val reference:JsonObject,val label:String,val available:Boolean?,val updatedAt:String=""){val key:String get()=CoaxiCanvasReferenceChoice(reference,label).key}
data class CoaxiCanvasRegionResolution(val members:List<CoaxiCanvasRegionMember>,val complete:Boolean,val issue:String?=null)
fun canvasRegionRoots(node:JsonObject,groups:List<JsonObject>):List<String>{val data=node["data"].coreObject();return when(data.coreText("kind")){"folder"->listOf(data.coreText("folderPath")).filter{it.isNotEmpty()};"group"->(groups.firstOrNull{it.coreText("id")==data.coreText("folderGroupId")}?.get("projectPaths")as?JsonArray)?.mapNotNull{(it as?JsonPrimitive)?.contentOrNull}.orEmpty();else->emptyList()}}
/** Port of canvasFolderResolution: no prefix guess and no promotion of a subdirectory/linked checkout. */
suspend fun readCanvasFolderRelations(model:CoaxiWorkspaceViewModel,roots:List<String>):Map<String,CoaxiCanvasFolderRelation> = coroutineScope{
    val generation=model.connectionGeneration;val gate=Semaphore(4)
    roots.distinct().map{root->async{gate.withPermit{root to try{
        val value=model.query(CoaxiOperation.WORKSPACES_WORKTREES,buildJsonObject{put("path",root)}).coreObject();val worktrees=value["worktrees"]as?JsonArray?:error("工程工作树清单缺失");val project=value.coreText("projectPath");val repository=(value["repositoryRoot"]as?JsonPrimitive)?.contentOrNull;val main=(value["mainWorktreePath"]as?JsonPrimitive)?.contentOrNull
        require(value["schemaVersion"]==JsonPrimitive(1)&&value.coreText("requestedPath")==root&&project.isNotEmpty()&&worktrees.size<=4096){"工程工作树关系不一致"}
        val seen=mutableSetOf<String>();val rows=worktrees.mapIndexed{index,item->val row=item as?JsonObject?:error("工程工作树项目无效");require(row.coreText("projectPath").isNotEmpty()&&seen.add(row.coreText("projectPath"))&&listOf("available","bare","locked","prunable").all{row[it]==JsonPrimitive(true)||row[it]==JsonPrimitive(false)}&&row["isMain"]==JsonPrimitive(index==0)){"工程工作树关系不一致"};val branch=(row["branch"]as?JsonPrimitive)?.contentOrNull;require(branch==null||branch.isNotEmpty());require(row["detached"]!=JsonPrimitive(true)||branch==null);row}
        require(if(repository==null)rows.isEmpty()&&main==null else rows.any{it.coreText("projectPath")==repository&&it["available"]==JsonPrimitive(true)&&it["bare"]==JsonPrimitive(false)}&&main==rows.firstOrNull()?.takeUnless{it["bare"]==JsonPrimitive(true)}?.coreText("projectPath")){"工程工作树关系不一致"}
        require(generation==model.connectionGeneration)
        val paths=mutableSetOf(root,project);if(project==repository&&repository==main)rows.filter{it["bare"]==JsonPrimitive(false)}.forEach{paths.add(it.coreText("projectPath"))}
        CoaxiCanvasFolderRelation(paths.toList(),true,if(project==repository&&main!=null&&main!=repository)main else null,projectPath=project,worktrees=rows)
    }catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){CoaxiCanvasFolderRelation(listOf(root),false,issue="工作树关系读取失败，已保留已知对话。")}}}}.awaitAll().toMap()
}
fun resolveCanvasRegion(node:JsonObject,rows:List<JsonObject>,catalogComplete:Boolean,groups:List<JsonObject>,folders:Map<String,CoaxiCanvasFolderRelation>):CoaxiCanvasRegionResolution{
    val data=node["data"].coreObject();val kind=data.coreText("kind");var complete=catalogComplete;var issue:String?=if(!catalogComplete)"对话清单读取未完成"else null
    fun options(thread:JsonObject,memberIds:Set<String>?=null):List<CoaxiCanvasRegionMember>{val options=canvasReferenceChoices(listOf(thread));val private=thread["privateConversations"].coreArray().associateBy{it.coreText("conversationId")};return options.filter{val id=(it.reference["conversationId"]as?JsonPrimitive)?.contentOrNull;id==null||memberIds==null||private[id]?.coreText("agentId")in memberIds}.map{choice->val id=(choice.reference["conversationId"]as?JsonPrimitive)?.contentOrNull;CoaxiCanvasRegionMember(choice.reference,choice.label,true,if(id==null)thread.coreText("updatedAt")else private[id]?.coreText("updatedAt").orEmpty())}}
    fun sorted(values:List<CoaxiCanvasRegionMember>)=values.sortedWith(compareByDescending<CoaxiCanvasRegionMember>{runCatching{Instant.parse(it.updatedAt).toEpochMilli()}.getOrDefault(0L)}.thenBy{it.key})
    if(kind=="custom"){val known=rows.flatMap{options(it)}.associateBy{it.key};val values=data["conversations"].coreArray().map{reference->val key=CoaxiCanvasReferenceChoice(reference,"").key;known[key]?:CoaxiCanvasRegionMember(reference,if(catalogComplete)"对话已结束或不可用"else"尚未核对的对话",if(catalogComplete)false else null)};return CoaxiCanvasRegionResolution(sorted(values),complete,issue)}
    if(kind=="agent")return CoaxiCanvasRegionResolution(sorted(rows.flatMap{thread->val ids=thread["members"].coreArray().filter{(data.coreText("agentId").isEmpty()||it.coreText("agentId")==data.coreText("agentId"))&&(data.coreText("runtimeKind").isEmpty()||it.coreText("runtimeKind")==data.coreText("runtimeKind"))}.map{it.coreText("agentId")}.toSet();if(ids.isEmpty())emptyList()else options(thread,ids)}),complete,issue)
    if(kind!in listOf("folder","group"))return CoaxiCanvasRegionResolution(emptyList(),false,"此卡片不是区域")
    if(kind=="group"&&groups.none{it.coreText("id")==data.coreText("folderGroupId")})return CoaxiCanvasRegionResolution(emptyList(),false,"项目分组在当前工作区不可用")
    val paths=mutableSetOf<String>();canvasRegionRoots(node,groups).forEach{root->val related=folders[root];if(related?.complete!=true){complete=false;issue=related?.issue?:"工作树关系尚未核对"};if(kind!="group"||related?.parentPath==null)paths.addAll(related?.paths?:listOf(root))}
    return CoaxiCanvasRegionResolution(sorted(rows.filter{it.coreText("projectBindingKind")=="directory"&&it.coreText("projectPath")in paths}.flatMap{options(it)}),complete,issue)
}
/** A derived card is re-resolved from the current region and bounded Core metadata before opening. */
suspend fun requireCanvasRegionReference(model:CoaxiWorkspaceViewModel,boardId:String,nodeId:String,reference:JsonObject){
    val generation=model.connectionGeneration;val graph=model.query(CoaxiOperation.CANVAS_NODES_SNAPSHOT,buildJsonObject{put("boardId",boardId)}).coreObject();val node=graph["nodes"].coreArray().firstOrNull{it.coreText("id")==nodeId}?:error("原区域已删除")
    val groups=model.query(CoaxiOperation.NAVIGATION_PROJECTGROUPS,buildJsonObject{put("action","get")}).coreObject()["projectGroups"].coreArray();val folders=readCanvasFolderRelations(model,canvasRegionRoots(node,groups));val key=CoaxiCanvasReferenceChoice(reference,"").key
    var cursor:CoaxiCanvasReferenceCursor?=null
    do{val page=readCanvasReferencePage(model,cursor);val target=page.items.firstOrNull{it.coreText("threadId")==reference.coreText("threadId")};if(target!=null){require(resolveCanvasRegion(node,listOf(target),false,groups,folders).members.any{it.key==key&&it.available==true}){"原区域与会话关系已变化"};require(generation==model.connectionGeneration);return};cursor=page.next}while(cursor!=null)
    error("原区域会话已结束或不可用，引用保留")
}
