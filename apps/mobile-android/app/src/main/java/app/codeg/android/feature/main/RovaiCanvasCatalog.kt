package app.codeg.android.feature.main

import androidx.compose.runtime.*
import app.codeg.android.core.network.RovaiOperation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.sync.Mutex
import kotlinx.serialization.json.*

data class RovaiCanvasCatalogSnapshot(val rows:List<JsonObject> = emptyList(),val groups:List<JsonObject> = emptyList(),val folders:Map<String,RovaiCanvasFolderRelation> = emptyMap(),val complete:Boolean=false,val issue:String?=null)
/** One mounted canvas owns reference pages and folder relations; cards only derive from this snapshot. */
class RovaiCanvasCatalog {
    var snapshot by mutableStateOf(RovaiCanvasCatalogSnapshot());private set
    private val gate=Mutex();private var scope="";private var epoch=0L
    fun configure(next:String){if(next==scope)return;scope=next;epoch++;snapshot=RovaiCanvasCatalogSnapshot()}
    suspend fun load(model:RovaiWorkspaceViewModel,nodes:List<JsonObject>){
        if(!gate.tryLock())return;val generation=model.connectionGeneration;val stamp=epoch
        fun current()=stamp==epoch&&generation==model.connectionGeneration
        try{
            if(!current())return;snapshot=snapshot.copy(complete=false,issue=null)
            val value=model.query(RovaiOperation.NAVIGATION_PROJECTGROUPS,buildJsonObject{put("action","get")}).coreObject();require(value["schemaVersion"]==JsonPrimitive(1)&&value["projectGroups"]is JsonArray){"项目分组目录无法读取"};val groups=value["projectGroups"].coreArray()
            var cursor:RovaiCanvasReferenceCursor?=null;val rows=mutableListOf<JsonObject>();val seen=mutableSetOf<String>()
            do{val page=readCanvasReferencePage(model,cursor);if(!current())return;require(page.items.all{seen.add(it.coreText("threadId"))}){"对话目录分页重复，请重新读取"};rows.addAll(page.items);cursor=page.next
                val known=snapshot.rows.associateBy{it.coreText("threadId")}.toMutableMap();rows.forEach{known[it.coreText("threadId")]=it};snapshot=snapshot.copy(rows=known.values.toList(),groups=groups,complete=false)
            }while(cursor!=null)
            val roots=nodes.flatMap{canvasRegionRoots(it,groups)}.toMutableSet();val summaryThreads=mutableSetOf<String>()
            nodes.forEach{node->val data=node["data"].coreObject();if(data.coreText("kind")=="conversation"&&data["objectReference"] !is JsonObject)data["conversation"].coreObject().coreText("threadId").takeIf{it.isNotEmpty()}?.let{summaryThreads.add(it)};if(data.coreText("kind")=="custom")data["conversations"].coreArray().forEach{summaryThreads.add(it.coreText("threadId"))};if(data.coreText("kind")=="agent")rows.filter{thread->thread["members"].coreArray().any{(data.coreText("agentId").isEmpty()||it.coreText("agentId")==data.coreText("agentId"))&&(data.coreText("runtimeKind").isEmpty()||it.coreText("runtimeKind")==data.coreText("runtimeKind"))}}.forEach{summaryThreads.add(it.coreText("threadId"))}}
            roots.addAll(rows.filter{it.coreText("threadId")in summaryThreads&&it.coreText("projectBindingKind")=="directory"}.map{it.coreText("projectPath")})
            val read=readCanvasFolderRelations(model,roots.toList());if(!current())return
            val folders=read.mapValues{(root,value)->if(value.complete)value else snapshot.folders[root]?.copy(complete=false,issue=value.issue)?:value}
            snapshot=RovaiCanvasCatalogSnapshot(rows.toList(),groups,folders,true)
        }catch(cancelled:CancellationException){throw cancelled}catch(error:Exception){if(current())snapshot=snapshot.copy(complete=false,issue=error.message?:"关联目录读取未完成")}finally{gate.unlock()}
    }
}
