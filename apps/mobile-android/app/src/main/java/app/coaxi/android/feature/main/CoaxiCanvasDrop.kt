package app.coaxi.android.feature.main

import kotlinx.serialization.json.*

/** canvas-member-interaction/computeDropHint: same-region no-op, derived regions retain business relationships. */
fun canvasDropMutation(source:JsonObject,member:JsonObject?,x:Double,y:Double,nodes:List<JsonObject>,memberTransferVersion:Long):JsonObject?{
    val sourceData=source["data"].coreObject();val sourceId=source.coreText("id");val reference=member?:sourceData["conversation"]as?JsonObject
    fun move()=buildJsonObject{if(member==null){put("type","move");putJsonArray("moves"){add(buildJsonObject{put("nodeId",sourceId);put("x",x);put("y",y)})}}else{put("type","detach");put("regionId",sourceId);put("conversation",member);put("x",x);put("y",y)}}
    if(member==null&&(sourceData.coreText("kind")!="conversation"||sourceData["objectReference"]is JsonObject)||reference==null)return move()
    fun n(node:JsonObject,key:String,fallback:Double)=(node["data"].coreObject()[key]as?JsonPrimitive)?.doubleOrNull?:fallback
    fun hit(node:JsonObject)=x+112>=n(node,"x",0.0)&&x+112<=n(node,"x",0.0)+n(node,"width",320.0)&&y+66>=n(node,"y",0.0)&&y+66<=n(node,"y",0.0)+n(node,"height",220.0)
    val region=nodes.lastOrNull{it["data"].coreObject().coreText("kind")in listOf("folder","group","agent","custom")&&hit(it)}
    if(region!=null&&member!=null&&region.coreText("id")==sourceId)return null
    if(region!=null&&region["data"].coreObject().coreText("kind")!="custom")return move()
    val key=CoaxiCanvasReferenceChoice(reference,"").key
    val pin=if(region!=null)null else nodes.lastOrNull{candidate->val data=candidate["data"].coreObject();candidate.coreText("id")!=sourceId&&data.coreText("kind")=="conversation"&&data["objectReference"] !is JsonObject&&data["conversation"]is JsonObject&&CoaxiCanvasReferenceChoice(data["conversation"].coreObject(),"").key!=key&&hit(candidate)}
    if(region==null&&pin==null)return move()
    require(member==null||memberTransferVersion==1L){"当前Host尚未提供原子成员转移，请重新连接更新后的工作区"}
    return buildJsonObject{
        put("type","group");put("targetNodeId",region?.get("id")?:JsonNull)
        put("newRegion",if(pin==null)JsonNull else buildJsonObject{put("kind","custom");put("x",n(pin,"x",0.0)-12);put("y",n(pin,"y",0.0)-52);put("width",484);put("height",196)})
        put("conversations",JsonArray(listOfNotNull(pin?.get("data").coreObject()["conversation"],reference)))
        put("consumeNodeIds",JsonArray(listOfNotNull(pin?.get("id"),if(member==null)source["id"]else null)))
        if(member!=null)putJsonArray("sourceMembers"){add(buildJsonObject{put("regionId",sourceId);put("conversation",member)})}
    }
}
/** Rendered region footprints are presentation-only inputs for fit, minimap, packing and hit testing. */
fun canvasPresentationNodes(nodes:List<JsonObject>,catalog:CoaxiCanvasCatalogSnapshot,expanded:Set<String>):List<JsonObject> = nodes.map{node->val data=node["data"].coreObject();val region=data.coreText("kind")in listOf("folder","group","agent","custom");val height=if(data["collapsed"]==JsonPrimitive(true))40f else if(region)canvasRegionHeight(data,resolveCanvasRegion(node,catalog.rows,catalog.complete,catalog.groups,catalog.folders),node.coreText("id")in expanded)else return@map node;JsonObject(node+mapOf("data" to JsonObject(data+mapOf("height" to JsonPrimitive(height))))) }
