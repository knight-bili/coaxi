package app.codeg.android.feature.main

import kotlinx.serialization.json.*

data class RovaiProjectNavigationSection(val id:String,val name:String,val color:String?,val groupId:String?,val projects:List<JsonObject>)

/** Port of resolveSidebarProjectGroups: real worktree relations take precedence over child grouping. */
fun projectNavigationSections(snapshot:JsonObject,projects:List<JsonObject>,folders:Map<String,RovaiCanvasFolderRelation>):List<RovaiProjectNavigationSection>{
    val groups=snapshot["projectGroups"].coreArray()
    fun paths(group:JsonObject)=(group["projectPaths"]as?JsonArray)?.mapNotNull{(it as?JsonPrimitive)?.contentOrNull}.orEmpty()
    val owners=mutableMapOf<String,String>()
    groups.forEach{group->paths(group).filter{folders[it]?.parentPath==null}.forEach{owners[it]=group.coreText("id")}}
    groups.forEach{group->paths(group).forEach{root->val relation=folders[root];if(relation?.parentPath==null)relation?.paths.orEmpty().filter{it!=root}.forEach{owners[it]=group.coreText("id")}}}
    val byPath=projects.associateBy{it.coreText("projectPath")}
    val grouped=groups.map{group->val id=group.coreText("id");val expanded=paths(group).flatMap{folders[it]?.paths?:listOf(it)}.distinct().filter{owners[it]==id};RovaiProjectNavigationSection("folder-group:$id",group.coreText("name"),(group["color"]as?JsonPrimitive)?.contentOrNull,id,expanded.mapNotNull{byPath[it]})}
    val loose=projects.filter{it.coreText("projectPath") !in owners}.map{RovaiProjectNavigationSection("directory:${it.coreText("projectPath")}",it.coreText("name"),null,null,listOf(it))}
    val ranks=(snapshot["projectOrder"]as?JsonArray)?.mapNotNull{(it as?JsonPrimitive)?.contentOrNull}?.withIndex()?.associate{it.value to it.index}.orEmpty()
    return (loose+grouped).sortedBy{ranks[it.id]?:Int.MAX_VALUE}
}
