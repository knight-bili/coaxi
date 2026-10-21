package app.codeg.android.core.common

import android.net.Uri
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import java.util.UUID

/** 只解析导航目的地；不接受地址、凭据、命令或自动登录参数。 */
data class RovaiDeepLink(val kind:String,val value:String,val requestId:String=UUID.randomUUID().toString()) {
    companion object {
        fun parse(uri:Uri?):RovaiDeepLink? {
            if(uri==null||uri.scheme?.lowercase() !in setOf("codeg","rovai")||uri.query!=null||uri.fragment!=null||uri.userInfo!=null||uri.port != -1)return null
            val kind=uri.host?.lowercase()?:return null;val parts=uri.pathSegments
            if(parts.size!=1||kind !in setOf("tab","settings","thread","mission","canvas","conversation","project","folder"))return null
            val value=parts.single();if(value.isBlank()||value.length>128||value.any{it.isISOControl()||it=='/'||it=='\\'})return null
            return RovaiDeepLink(kind,value)
        }
    }
}
object RovaiDeepLinkBus {
    private val value=MutableStateFlow<RovaiDeepLink?>(null)
    val pending=value.asStateFlow()
    fun dispatch(uri:Uri?){RovaiDeepLink.parse(uri)?.let{value.value=it}}
    fun consume(link:RovaiDeepLink){value.compareAndSet(link,null)}
}
