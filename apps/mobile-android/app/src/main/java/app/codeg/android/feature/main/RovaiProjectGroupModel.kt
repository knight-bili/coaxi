package app.codeg.android.feature.main

import app.codeg.android.core.network.RovaiOperation
import kotlinx.serialization.json.*

private val projectGroupId=Regex("[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}")
fun rovaiProjectPath(value:String):Boolean=value.isNotEmpty()&&value.length<=8192&&!value.contains('\u0000')&&(value.startsWith('/')||value.startsWith("\\\\")||Regex("^[A-Za-z]:[\\\\/].*").matches(value))&&value.split('/','\\').none{it=="."||it==".."}
private fun strings(value:JsonElement?):List<String>{val rows=value as?JsonArray?:error("项目分组清单无法读取");return rows.map{(it as?JsonPrimitive)?.takeIf{it.isString}?.content?:error("项目分组路径无法读取")}}
private fun group(value:JsonElement?):JsonObject{
    val value=value as?JsonObject?:error("项目分组无法读取");require(value.keys==setOf("id","name","color","projectPaths")&&projectGroupId.matches(value.coreText("id"))){"项目分组身份无效"}
    val name=value.coreText("name").trim().replace(Regex("\\s+")," ");require(name.isNotEmpty()&&name.codePointCount(0,name.length)<=80){"分组名称需为1到80个字符"};val color=value["color"];require(color==JsonNull||rovaiCanvasColors.drop(1).any{color==JsonPrimitive(it.first)}){"项目分组颜色无效"};val paths=strings(value["projectPaths"]);require(paths.all(::rovaiProjectPath)&&paths.distinct().size==paths.size){"项目分组工程路径无效"};return JsonObject(value+mapOf("name" to JsonPrimitive(name)))
}
fun parseProjectGroups(value:JsonElement):JsonObject{
    val result=value as?JsonObject?:error("项目分组来源无法读取");val revision=(result["projectGroupsRevision"]as?JsonPrimitive)?.takeUnless{it.isString}?.longOrNull
    require(result["schemaVersion"]==JsonPrimitive(1)&&revision!=null&&revision in 0..9_007_199_254_740_991L&&result["initialized"]==JsonPrimitive(revision>0)){"项目分组版本无法读取"}
    val rows=result["projectGroups"]as?JsonArray?:error("项目分组清单无法读取");val ids=mutableSetOf<String>();val paths=mutableSetOf<String>();rows.forEach{entry->val row=group(entry);require(row==entry&&ids.add(row.coreText("id"))&&strings(row["projectPaths"]).all{paths.add(it)}){"项目分组目录重复或格式不一致"}}
    if(result["projectOrder"]!=JsonNull){val order=strings(result["projectOrder"]);require(order.distinct().size==order.size&&order.all{key->if(key.startsWith("folder-group:"))key.removePrefix("folder-group:")in ids else key.startsWith("directory:")&&rovaiProjectPath(key.removePrefix("directory:"))&&key.removePrefix("directory:")!in paths}){"项目分组排序无效"}}
    return result
}
suspend fun readProjectGroups(model:RovaiWorkspaceViewModel):JsonObject{val generation=model.connectionGeneration;val value=model.query(RovaiOperation.NAVIGATION_PROJECTGROUPS,buildJsonObject{put("action","get")});require(generation==model.connectionGeneration){"工作区已切换"};return parseProjectGroups(value)}
/** Port of applyNavigationProjectLayout; UI-only changes preserve original UUIDs and unmentioned members/order. */
fun projectGroupPlan(snapshot:JsonObject,expectedRevision:Long,input:JsonObject,activePaths:List<String>):JsonObject{
    parseProjectGroups(snapshot);require(expectedRevision==snapshot.coreLong("projectGroupsRevision")){"项目分组已有新修改，请核对后再保存"};require(activePaths.all(::rovaiProjectPath)){"可选工程路径无效"}
    var groups=snapshot["projectGroups"].coreArray();var order=if(snapshot["projectOrder"]==JsonNull)activePaths.map{"directory:$it"}else strings(snapshot["projectOrder"]);val active=activePaths.toSet()
    when(input.coreText("type")){
        "create","update"->{require(input.keys==setOf("type","group"));val group=group(input["group"]);val id=group.coreText("id");val previous=groups.firstOrNull{it.coreText("id")==id};require(if(input.coreText("type")=="create")previous==null else previous!=null){"项目分组身份已变化，请重新读取"};val selected=strings(group["projectPaths"]);val previousPaths=previous?.get("projectPaths")?.let(::strings).orEmpty();require(selected.all{it in active||it in previousPaths}){"请选择已登记且未移除的工程"}
            groups=groups.map{row->if(row.coreText("id")==id)group else JsonObject(row+mapOf("projectPaths" to JsonArray(strings(row["projectPaths"]).filter{it !in selected}.map(::JsonPrimitive))))}
            if(previous==null){groups=groups+group;order=order+"folder-group:$id"}else order=order+previousPaths.filter{it in active&&it !in selected}.map{"directory:$it"}
        }
        "delete"->{require(input.keys==setOf("type","groupId"));val id=input.coreText("groupId");val group=groups.firstOrNull{it.coreText("id")==id}?:error("项目分组身份已变化，请重新读取");val members=strings(group["projectPaths"]).map{"directory:$it"};groups=groups.filter{it.coreText("id")!=id};order=order.filter{it!="folder-group:$id"&&it !in members}+strings(group["projectPaths"]).filter{it in active}.map{"directory:$it"}}
        "layout"->{require(input.keys==setOf("type","entries"));val entries=input["entries"]as?JsonArray?:error("项目布局无效");if(entries.isEmpty())return buildJsonObject{put("projectGroups",snapshot["projectGroups"]?:JsonArray(emptyList()));put("projectOrder",snapshot["projectOrder"]?:JsonNull)};val ids=groups.map{it.coreText("id")}.toSet();val named=mutableSetOf<String>();val top=mutableListOf<String>();val members=mutableMapOf<String,MutableList<String>>()
            entries.forEach{entry->val row=entry as?JsonObject?:error("项目布局无效");if(row.coreText("kind")=="group"&&row.keys==setOf("kind","groupId")&&row.coreText("groupId")in ids){val key="folder-group:${row.coreText("groupId")}";require(named.add(key)){"项目布局有重复项"};top.add(key)}else if(row.coreText("kind")=="project"&&row.keys==setOf("kind","projectPath","groupId")&&row.coreText("projectPath")in active&&(row["groupId"]==JsonNull||row.coreText("groupId")in ids)){val path=row.coreText("projectPath");require(named.add("directory:$path")){"项目布局有重复项"};if(row["groupId"]==JsonNull)top.add("directory:$path")else members.getOrPut(row.coreText("groupId")){mutableListOf()}.add(path)}else error("项目布局无效")}
            if(entries.isNotEmpty()){groups=groups.map{row->JsonObject(row+mapOf("projectPaths" to JsonArray((members[row.coreText("id")].orEmpty()+strings(row["projectPaths"]).filter{"directory:$it" !in named}).map(::JsonPrimitive))))};order=top+order.filter{it !in named}}
        }
        else->error("项目分组操作无效")
    }
    val grouped=groups.flatMap{strings(it["projectPaths"])}.map{"directory:$it"}.toSet();return buildJsonObject{put("projectGroups",JsonArray(groups));put("projectOrder",JsonArray(order.filter{it !in grouped}.distinct().map(::JsonPrimitive)))}
}
fun projectGroupPendingMatches(current:JsonObject,pending:JsonObject):Boolean{val wanted=pending["value"].coreObject();val expected=pending.coreLong("expectedRevision");return current.coreLong("projectGroupsRevision")in expected..(expected+1)&&current["projectGroups"]==wanted["projectGroups"]&&current["projectOrder"]==wanted["projectOrder"]}
