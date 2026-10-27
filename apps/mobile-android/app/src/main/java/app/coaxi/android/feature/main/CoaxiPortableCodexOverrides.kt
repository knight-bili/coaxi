package app.coaxi.android.feature.main

import androidx.compose.ui.res.stringResource
import app.coaxi.android.R

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.coaxi.android.core.designsystem.component.CoaxiTextField
import kotlinx.serialization.json.*

private data class PortableCodexField(val key:String,val label:String,val options:List<JsonElement>)
// Coaxi aff1e6c codex-model-list-editor.tsx 原协议/行为字段与闭集。
private val portableCodexFields=listOf(
    PortableCodexField("tool_mode","工具调用方式",listOf(JsonNull,JsonPrimitive("direct"),JsonPrimitive("code_mode"),JsonPrimitive("code_mode_only"))),
    PortableCodexField("multi_agent_version","多智能体协议",listOf(JsonNull,JsonPrimitive("v1"),JsonPrimitive("v2"))),
    PortableCodexField("use_responses_lite","使用精简 Responses 协议",listOf(JsonPrimitive(true),JsonPrimitive(false))),
    PortableCodexField("default_reasoning_summary","推理摘要",listOf(JsonPrimitive("auto"),JsonPrimitive("concise"),JsonPrimitive("detailed"),JsonPrimitive("none"))),
    PortableCodexField("default_verbosity","回答详细程度",listOf(JsonNull,JsonPrimitive("low"),JsonPrimitive("medium"),JsonPrimitive("high"))),
    PortableCodexField("shell_type","终端工具类型",listOf(JsonPrimitive("default"),JsonPrimitive("local"),JsonPrimitive("unified_exec"),JsonPrimitive("disabled"),JsonPrimitive("shell_command"))),
    PortableCodexField("apply_patch_tool_type","文件补丁工具",listOf(JsonNull,JsonPrimitive("freeform"))),
    PortableCodexField("web_search_tool_type","网页搜索内容",listOf(JsonPrimitive("text"),JsonPrimitive("text_and_image"))),
    PortableCodexField("supports_reasoning_summary_parameter","支持推理摘要参数",listOf(JsonPrimitive(true),JsonPrimitive(false))),
    PortableCodexField("support_verbosity","支持详细程度参数",listOf(JsonPrimitive(true),JsonPrimitive(false))),
    PortableCodexField("supports_parallel_tool_calls","支持并行工具调用",listOf(JsonPrimitive(true),JsonPrimitive(false))),
    PortableCodexField("supports_search_tool","支持搜索工具",listOf(JsonPrimitive(true),JsonPrimitive(false))),
    PortableCodexField("supports_image_detail_original","支持原始图片细节",listOf(JsonPrimitive(true),JsonPrimitive(false))),
    PortableCodexField("include_apps_usage_instructions","包含应用使用说明",listOf(JsonPrimitive(true),JsonPrimitive(false))),
    PortableCodexField("include_plugin_usage_instructions","包含插件使用说明",listOf(JsonPrimitive(true),JsonPrimitive(false))),
    PortableCodexField("include_skills_usage_instructions","包含技能使用说明",listOf(JsonPrimitive(true),JsonPrimitive(false)))
)

@Composable
fun CoaxiPortableCodexOverrides(value:JsonObject,disabled:Boolean,onChange:(JsonObject)->Unit){
    var expanded by remember{mutableStateOf(false)}
    fun set(key:String,next:JsonElement?){onChange(JsonObject(value.toMutableMap().apply{if(next==null)remove(key)else put(key,next)}))}
    fun label(option:JsonElement)=when(option){JsonNull->"不使用";JsonPrimitive(true)->"开启";JsonPrimitive(false)->"关闭";else->option.jsonPrimitive.content}
    TextButton(onClick={expanded=!expanded}){Text(if(expanded)"收起高级设置" else "高级设置")}
    if(expanded)Column(Modifier.fillMaxWidth(),verticalArrangement=Arrangement.spacedBy(8.dp)){
        Text(stringResource(R.string.coaxi_fusion_30109fb43b29941b),style=MaterialTheme.typography.bodySmall)
        portableCodexFields.forEachIndexed{index,field->
            if(index==0||index==3)Text(if(index==0)"协议" else "模型行为",style=MaterialTheme.typography.titleSmall)
            var menu by remember(field.key){mutableStateOf(false)}
            Text(field.label,style=MaterialTheme.typography.bodySmall)
            Box{
                val current=value[field.key]
                OutlinedButton(enabled=!disabled,onClick={menu=true}){Text(if(current==null)"继承基准模型" else if(field.options.contains(current))label(current) else "保留原设置（由原程序核对）")}
                DropdownMenu(expanded=menu,onDismissRequest={menu=false}){
                    DropdownMenuItem(text={Text(stringResource(R.string.coaxi_fusion_7c61147e3fbed0d1))},onClick={set(field.key,null);menu=false})
                    field.options.forEach{option->DropdownMenuItem(text={Text(label(option))},onClick={set(field.key,option);menu=false})}
                }
            }
        }
        Text(stringResource(R.string.coaxi_fusion_9922bf267d75d312),style=MaterialTheme.typography.titleSmall)
        listOf("description" to "模型说明","base_instructions" to "模型基础说明").forEach{(key,title)->
            Row{Text("覆盖$title",modifier=Modifier.weight(1f));Switch(checked=value.containsKey(key),enabled=!disabled,onCheckedChange={set(key,if(it)JsonPrimitive("")else null)})}
            if(value.containsKey(key))CoaxiTextField(value.coreText(key),{if(!disabled)set(key,JsonPrimitive(it))},title,singleLine=false)
        }
        val known=portableCodexFields.map{it.key}.toSet()+setOf("description","base_instructions")
        val other=value.keys.count{it !in known}
        if(other>0)Text("另有 $other 项原覆盖设置完整保留，未编辑字段不会删除。",style=MaterialTheme.typography.bodySmall)
    }
}
