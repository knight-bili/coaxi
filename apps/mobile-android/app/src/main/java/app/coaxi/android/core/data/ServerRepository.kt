package app.coaxi.android.core.data

import app.coaxi.android.core.common.DispatcherProvider
import app.coaxi.android.core.datastore.ServerProfile
import app.coaxi.android.core.datastore.ServerStore
import app.coaxi.android.core.network.CoaxiClient
import app.coaxi.android.core.network.CoaxiHostClient
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.json.*
import java.security.MessageDigest
import app.coaxi.android.core.network.CoaxiClientFactory
import app.coaxi.android.core.network.EventStream
import app.coaxi.android.core.security.SecretStore
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
 * and building API clients for them (via [CoaxiClientFactory]). Keeps the
 * token-handling and client construction in one place so ViewModels stay thin.
 */
@Singleton
class ServerRepository @Inject constructor(
    private val serverStore: ServerStore,
    private val secretStore: SecretStore,
    private val factory: CoaxiClientFactory,
    private val dispatchers: DispatcherProvider,
) {
    private val coaxiMutex=Mutex()
    private val coaxiClients=mutableMapOf<String,Pair<String,CoaxiHostClient>>()
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
        coaxiMutex.withLock {coaxiClients.remove(id)?.second?.close()}
        withContext(dispatchers.io){secretStore.remove("coaxi.host-session.$id");secretStore.remove("coaxi.pending.$id")}
        serverStore.delete(id)
        withContext(dispatchers.io) { secretStore.remove(id) }
    }

    suspend fun select(id: String?) = serverStore.select(id)

    /** An API client for a saved profile, or null if its token is missing. */
    suspend fun client(profile: ServerProfile): CoaxiClient? {
        val token = token(profile.id) ?: return null
        return factory.client(profile.baseUrl, token)
    }

    suspend fun coaxiClient(profile:ServerProfile):CoaxiHostClient? {
        val token=token(profile.id)?:return null
        val signature=MessageDigest.getInstance("SHA-256").digest((profile.baseUrl+"\u0000"+token).toByteArray()).joinToString(""){"%02x".format(it)}
        return coaxiMutex.withLock {
            coaxiClients[profile.id]?.let{if(it.first==signature)return@withLock it.second;it.second.close()}
            val key="coaxi.host-session.${profile.id}"
            val client=CoaxiHostClient(profile.baseUrl,token,loadSession={
                withContext(dispatchers.io){secretStore.get(key)?.let{raw->runCatching{val value=Json.parseToJsonElement(raw).jsonObject;if(value["signature"]?.jsonPrimitive?.content==signature)value["credential"]?.jsonPrimitive?.content else null}.getOrNull()}}
            },saveSession={credential->withContext(dispatchers.io){if(credential==null)secretStore.remove(key)else secretStore.put(key,buildJsonObject{put("signature",signature);put("credential",credential)}.toString())}})
            coaxiClients[profile.id]=signature to client;client
        }
    }

    private suspend fun coaxiBodyKey(profile:ServerProfile,owner:String):String {
        val credential=token(profile.id)?:throw IllegalStateException("缺少服务器凭据")
        val digest=MessageDigest.getInstance("SHA-256").digest((profile.baseUrl+"\u0000"+credential+"\u0000"+owner).toByteArray(Charsets.UTF_8)).joinToString(""){"%02x".format(it)}
        return "coaxi.body.${profile.id}.$digest"
    }
    suspend fun readCoaxiBody(profile:ServerProfile,owner:String):String=withContext(dispatchers.io){secretStore.get(coaxiBodyKey(profile,owner))?:""}
    suspend fun saveCoaxiBody(profile:ServerProfile,owner:String,body:String)=withContext(dispatchers.io){require(body.toByteArray(Charsets.UTF_8).size<=1024*1024){"草稿超过1MB，请拆分后发送。"};val key=coaxiBodyKey(profile,owner);if(body.isEmpty())secretStore.remove(key)else secretStore.put(key,body);Unit}

    suspend fun loadCoaxiPending(profile:ServerProfile):JsonObject?=withContext(dispatchers.io){secretStore.get("coaxi.pending.${profile.id}")?.let{raw->runCatching{val value=Json.parseToJsonElement(raw).jsonObject;if(value["endpoint"]?.jsonPrimitive?.content==profile.baseUrl)value["request"] as? JsonObject else null}.getOrNull()}}
    suspend fun saveCoaxiPending(profile:ServerProfile,request:JsonObject?)=withContext(dispatchers.io){val key="coaxi.pending.${profile.id}";if(request==null)secretStore.remove(key)else secretStore.put(key,buildJsonObject{put("endpoint",profile.baseUrl);put("request",request)}.toString());Unit}

    /** An event stream for a saved profile, or null if its token is missing. */
    suspend fun eventStream(profile: ServerProfile): EventStream? {
        val token = token(profile.id) ?: return null
        return factory.eventStream(profile.baseUrl, token)
    }

    /** A transient client for an unsaved server (e.g. "Test Connection"). */
    fun transientClient(baseUrl: String, token: String): CoaxiClient =
        factory.client(baseUrl, token)
}
