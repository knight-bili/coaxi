package app.codeg.android.core.common

import android.net.Uri
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * A parsed `codeg://` deep link. Mirrors the iOS `Route.from(url:)` cases:
 * `codeg://tab/<name>`, `codeg://conversation/<id>`, `codeg://project/<id>`,
 * `codeg://settings/<slug>`. Unknown hosts / unparseable ids yield null.
 */
sealed interface DeepLinkRoute {
    data class OpenTab(val tab: String) : DeepLinkRoute
    data class OpenConversation(val id: Int) : DeepLinkRoute
    data class OpenProject(val id: Int) : DeepLinkRoute
    data class OpenSettings(val slug: String) : DeepLinkRoute

    companion object {
        fun parse(uri: Uri?): DeepLinkRoute? {
            if (uri == null || !uri.scheme.equals("codeg", ignoreCase = true)) return null
            val host = uri.host?.lowercase() ?: return null
            val seg = uri.pathSegments.firstOrNull()
            return when (host) {
                "tab" -> seg?.let { OpenTab(it.lowercase()) }
                "conversation" -> seg?.toIntOrNull()?.let { OpenConversation(it) }
                "project", "folder" -> seg?.toIntOrNull()?.let { OpenProject(it) }
                "settings" -> seg?.let { OpenSettings(it.lowercase()) }
                else -> null
            }
        }
    }
}

/**
 * Process-wide holder for the latest incoming deep link. [MainActivity] dispatches
 * parsed routes here from `onCreate`/`onNewIntent`; the navigation shell collects
 * [pending] and calls [consume] once it has navigated. A [StateFlow] (not a
 * replay-less SharedFlow) so a link dispatched in `onCreate` — before the shell's
 * collector subscribes — is still seen.
 */
object DeepLinkBus {
    private val _pending = MutableStateFlow<DeepLinkRoute?>(null)
    val pending: StateFlow<DeepLinkRoute?> = _pending.asStateFlow()

    fun dispatch(uri: Uri?) {
        DeepLinkRoute.parse(uri)?.let { _pending.value = it }
    }

    /** Clear [route] if it's still the pending one (called after navigation). */
    fun consume(route: DeepLinkRoute) {
        _pending.compareAndSet(route, null)
    }
}
