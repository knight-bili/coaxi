package app.codeg.android.core.data

import app.codeg.android.core.common.DispatcherProvider
import app.codeg.android.core.datastore.ServerProfile
import app.codeg.android.core.datastore.ServerStore
import app.codeg.android.core.network.CodegClient
import app.codeg.android.core.network.RovaiHostClient
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.json.*
import java.security.MessageDigest
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
    private val rovaiMutex=Mutex()
    private val rovaiClients=mutableMapOf<String,Pair<String,RovaiHostClient>>()
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
        rovaiMutex.withLock {rovaiClients.remove(id)?.second?.close()}
        withContext(dispatchers.io){secretStore.remove("rovai.host-session.$id");secretStore.remove("rovai.pending.$id")}
        serverStore.delete(id)
        withContext(dispatchers.io) { secretStore.remove(id) }
    }

    suspend fun select(id: String?) = serverStore.select(id)

    /** An API client for a saved profile, or null if its token is missing. */
    suspend fun client(profile: ServerProfile): CodegClient? {
        val token = token(profile.id) ?: return null
        return factory.client(profile.baseUrl, token)
    }

    suspend fun rovaiClient(profile:ServerProfile):RovaiHostClient? {
        val token=token(profile.id)?:return null
        val signature=MessageDigest.getInstance("SHA-256").digest((profile.baseUrl+"\u0000"+token).toByteArray()).joinToString(""){"%02x".format(it)}
        return rovaiMutex.withLock {
            rovaiClients[profile.id]?.let{if(it.first==signature)return@withLock it.second;it.second.close()}
            val key="rovai.host-session.${profile.id}"
            val client=RovaiHostClient(profile.baseUrl,token,loadSession={
                withContext(dispatchers.io){secretStore.get(key)?.let{raw->runCatching{val value=Json.parseToJsonElement(raw).jsonObject;if(value["signature"]?.jsonPrimitive?.content==signature)value["credential"]?.jsonPrimitive?.content else null}.getOrNull()}}
            },saveSession={credential->withContext(dispatchers.io){if(credential==null)secretStore.remove(key)else secretStore.put(key,buildJsonObject{put("signature",signature);put("credential",credential)}.toString())}})
            rovaiClients[profile.id]=signature to client;client
        }
    }

    suspend fun loadRovaiPending(profile:ServerProfile):JsonObject?=withContext(dispatchers.io){secretStore.get("rovai.pending.${profile.id}")?.let{raw->runCatching{val value=Json.parseToJsonElement(raw).jsonObject;if(value["endpoint"]?.jsonPrimitive?.content==profile.baseUrl)value["request"] as? JsonObject else null}.getOrNull()}}
    suspend fun saveRovaiPending(profile:ServerProfile,request:JsonObject?)=withContext(dispatchers.io){val key="rovai.pending.${profile.id}";if(request==null)secretStore.remove(key)else secretStore.put(key,buildJsonObject{put("endpoint",profile.baseUrl);put("request",request)}.toString());Unit}

    /** An event stream for a saved profile, or null if its token is missing. */
    suspend fun eventStream(profile: ServerProfile): EventStream? {
        val token = token(profile.id) ?: return null
        return factory.eventStream(profile.baseUrl, token)
    }

    /** A transient client for an unsaved server (e.g. "Test Connection"). */
    fun transientClient(baseUrl: String, token: String): CodegClient =
        factory.client(baseUrl, token)
}
