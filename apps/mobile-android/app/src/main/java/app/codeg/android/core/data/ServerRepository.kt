package app.codeg.android.core.data

import app.codeg.android.core.common.DispatcherProvider
import app.codeg.android.core.datastore.ServerProfile
import app.codeg.android.core.datastore.ServerStore
import app.codeg.android.core.network.CodegClient
import app.codeg.android.core.network.CodegClientFactory
import app.codeg.android.core.network.EventStream
import app.codeg.android.core.security.SecretStore
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.withContext
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Single entry point the UI layer uses for everything server-scoped: the saved
 * profiles + selection (from [ServerStore]), their tokens (from [SecretStore]),
 * and building API clients for them (via [CodegClientFactory]). Keeps the
 * token-handling and client construction in one place so ViewModels stay thin.
 */
@Singleton
class ServerRepository @Inject constructor(
    private val serverStore: ServerStore,
    private val secretStore: SecretStore,
    private val factory: CodegClientFactory,
    private val dispatchers: DispatcherProvider,
) {
    val profiles: Flow<List<ServerProfile>> = serverStore.profiles
    val selectedId: Flow<String?> = serverStore.selectedId

    /** Broadcast that a conversation was renamed/pinned/status-changed/deleted, so
     *  list-type screens can refetch. The faithful equivalent of iOS's
     *  `NotificationCenter` `conversationsDidChange`. */
    private val _conversationsChanged = MutableSharedFlow<Unit>(extraBufferCapacity = 1)
    val conversationsChanged: SharedFlow<Unit> = _conversationsChanged.asSharedFlow()

    fun notifyConversationsChanged() { _conversationsChanged.tryEmit(Unit) }

    /** Broadcast that the folder set changed (e.g. a worktree was registered/opened
     *  during a branch switch), so project lists can refetch. iOS's `foldersDidChange`. */
    private val _foldersChanged = MutableSharedFlow<Unit>(extraBufferCapacity = 1)
    val foldersChanged: SharedFlow<Unit> = _foldersChanged.asSharedFlow()

    fun notifyFoldersChanged() { _foldersChanged.tryEmit(Unit) }

    val selectedProfile: Flow<ServerProfile?> =
        combine(profiles, selectedId) { list, id -> list.firstOrNull { it.id == id } }

    /** The token for a profile, read off the IO dispatcher (Keystore decrypt). */
    suspend fun token(id: String): String? =
        withContext(dispatchers.io) { secretStore.get(id) }

    /** Save (insert/replace) a profile and its token. */
    suspend fun save(profile: ServerProfile, token: String) {
        withContext(dispatchers.io) { secretStore.put(profile.id, token) }
        serverStore.upsert(profile)
    }

    suspend fun delete(id: String) {
        serverStore.delete(id)
        withContext(dispatchers.io) { secretStore.remove(id) }
    }

    suspend fun select(id: String?) = serverStore.select(id)

    /** An API client for a saved profile, or null if its token is missing. */
    suspend fun client(profile: ServerProfile): CodegClient? {
        val token = token(profile.id) ?: return null
        return factory.client(profile.baseUrl, token)
    }

    /** An event stream for a saved profile, or null if its token is missing. */
    suspend fun eventStream(profile: ServerProfile): EventStream? {
        val token = token(profile.id) ?: return null
        return factory.eventStream(profile.baseUrl, token)
    }

    /** A transient client for an unsaved server (e.g. "Test Connection"). */
    fun transientClient(baseUrl: String, token: String): CodegClient =
        factory.client(baseUrl, token)
}
