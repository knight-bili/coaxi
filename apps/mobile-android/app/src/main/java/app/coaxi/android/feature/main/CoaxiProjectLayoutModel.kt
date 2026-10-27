package app.coaxi.android.feature.main

import kotlinx.serialization.json.*

data class CoaxiProjectLayoutItem(val kind:String,val target:String){val key:String get()=if(kind=="group")"folder-group:$target"else"directory:$target"}
data class CoaxiProjectLayoutLocation(val groupId:String?,val index:Int)
data class CoaxiProjectLayout(val top:List<CoaxiProjectLayoutItem>,val members:Map<String,List<String>>){
    fun value():JsonObject=buildJsonObject{put("top",JsonArray(top.map{buildJsonObject{put("kind",it.kind);put(if(it.kind=="group")"groupId"else"projectPath",it.target)}}));putJsonObject("members"){members.forEach{(id,paths)->put(id,JsonArray(paths.map(::JsonPrimitive)))}}}
    fun entries():JsonArray=JsonArray(top.flatMap{item->if(item.kind=="project")listOf(buildJsonObject{put("kind","project");put("projectPath",item.target);put("groupId",JsonNull)})else listOf(buildJsonObject{put("kind","group");put("groupId",item.target)})+members[item.target].orEmpty().map{path->buildJsonObject{put("kind","project");put("projectPath",path);put("groupId",item.target)}}})
    fun locate(item:CoaxiProjectLayoutItem):CoaxiProjectLayoutLocation?{val index=top.indexOf(item);if(index>=0)return CoaxiProjectLayoutLocation(null,index);if(item.kind=="project")members.forEach{(id,paths)->val found=paths.indexOf(item.target);if(found>=0)return CoaxiProjectLayoutLocation(id,found)};return null}
    fun move(item:CoaxiProjectLayoutItem,target:CoaxiProjectLayoutLocation):CoaxiProjectLayout{
        val source=locate(item)?:return this;if(target.index<0||item.kind=="group"&&target.groupId!=null||target.groupId!=null&&target.groupId !in members)return this
        val nextTop=top.toMutableList();val nextMembers=members.mapValues{it.value.toMutableList()}.toMutableMap()
        if(source.groupId==null)nextTop.removeAt(source.index)else nextMembers.getValue(source.groupId).removeAt(source.index)
        if(target.groupId==null)nextTop.add(target.index.coerceAtMost(nextTop.size),item)else nextMembers.getValue(target.groupId).add(target.index.coerceAtMost(nextMembers.getValue(target.groupId).size),item.target)
        return CoaxiProjectLayout(nextTop,nextMembers)
    }
}
/** Native port of createProjectLayout; worktrees following a main project never become movable roots. */
fun createProjectLayout(snapshot:JsonObject,active:List<String>,folders:Map<String,CoaxiCanvasFolderRelation>):CoaxiProjectLayout{
    val projects=active.distinct().filter{folders[it]?.parentPath==null};val groups=snapshot["projectGroups"].coreArray();val grouped=groups.flatMap{(it["projectPaths"]as?JsonArray)?.mapNotNull{(it as?JsonPrimitive)?.contentOrNull}.orEmpty()}.toSet();val order=(snapshot["projectOrder"]as?JsonArray)?.mapNotNull{(it as?JsonPrimitive)?.contentOrNull}.orEmpty();val ranks=order.withIndex().associate{it.value to it.index}
    val top=(projects.filter{it !in grouped}.map{CoaxiProjectLayoutItem("project",it)}+groups.map{CoaxiProjectLayoutItem("group",it.coreText("id"))}).sortedBy{ranks[it.key]?:Int.MAX_VALUE}
    val members=groups.associate{row->row.coreText("id") to (row["projectPaths"]as?JsonArray)?.mapNotNull{(it as?JsonPrimitive)?.contentOrNull?.takeIf{it in projects}}.orEmpty()}
    return CoaxiProjectLayout(top,members)
}
fun parseProjectLayout(value:JsonElement):CoaxiProjectLayout{
    val objectValue=value as?JsonObject?:error("原项目排版草稿无法读取");require(objectValue.keys==setOf("top","members"));val top=(objectValue["top"]as?JsonArray)?.map{entry->val row=entry as?JsonObject?:error("原项目排版项目无效");val kind=row.coreText("kind");require(kind in listOf("group","project")&&row.keys==setOf("kind",if(kind=="group")"groupId"else"projectPath"));CoaxiProjectLayoutItem(kind,row.coreText(if(kind=="group")"groupId"else"projectPath"))}?:error("原项目排版清单无效");val rawMembers=objectValue["members"]as?JsonObject?:error("原分组成员排版无效")
    val members=rawMembers.mapValues{(_,value)->(value as?JsonArray)?.map{(it as?JsonPrimitive)?.takeIf{it.isString}?.content?:error("原项目路径无效")}?:error("原成员列表无效")};val groupIds=top.filter{it.kind=="group"}.map{it.target};val paths=top.filter{it.kind=="project"}.map{it.target}+members.values.flatten()
    require(top.map{it.key}.distinct().size==top.size&&members.keys==groupIds.toSet()&&groupIds.size==groupIds.distinct().size&&paths.all(::coaxiProjectPath)&&paths.size==paths.distinct().size){"原项目排版包含重复或未知项目"};return CoaxiProjectLayout(top,members)
}
fun projectLayoutRelationIssue(base:CoaxiProjectLayout,next:CoaxiProjectLayout,folders:Map<String,CoaxiCanvasFolderRelation>):Boolean=next.entries().coreArray().filter{it.coreText("kind")=="project"}.any{entry->val path=entry.coreText("projectPath");val item=CoaxiProjectLayoutItem("project",path);val relation=folders[path];relation?.parentPath!=null||(base.locate(item)!=next.locate(item)&&relation?.complete!=true)}
