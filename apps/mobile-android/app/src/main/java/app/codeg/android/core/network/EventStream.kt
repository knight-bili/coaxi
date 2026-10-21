package app.codeg.android.core.network

import app.codeg.android.core.common.Base64Url
import app.codeg.android.core.model.EventEnvelope
import app.codeg.android.core.model.LiveSessionSnapshot
import io.ktor.client.HttpClient
import io.ktor.client.plugins.websocket.webSocket
import io.ktor.client.request.header
import io.ktor.http.HttpHeaders
import io.ktor.websocket.Frame
import io.ktor.websocket.readText
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.channelFlow
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull

/**
 * A live WebSocket connection to `/ws/events`. Faithful port of the iOS
 * `EventStream`.
 *
 * Usage: collect [frames] — connecting on first collect — then drive
 * subscriptions with [attach] / [detach]. The flow emits [StreamFrame.Ready]
 * once the server's `__ready__` arrives (safe to attach/prompt after that), then
 * snapshot / replay / event frames, and finally [StreamFrame.Closed] when the
 * socket ends. Reconnection is the caller's job: create a fresh [EventStream]
 * (mirrors the iOS Phase-1 strategy).
 *
 * Auth rides in the WebSocket subprotocol — browsers can't set headers, so codeg
 * accepts `codeg-token.<base64url-no-pad(token)>` as a subprotocol alongside
 * `codeg-events`. Keepalive pings are handled by Ktor's WebSockets plugin
 * (`pingIntervalMillis`), so a quiet-but-healthy turn is never torn down.
 */
class EventStream(
    rawBaseUrl: String,
    token: String,
    private val http: HttpClient,
) {
    private val url = websocketUrl(rawBaseUrl)
    private val subprotocols = "codeg-events, codeg-token.${Base64Url.encode(token)}"

    /** Outgoing attach-protocol messages, drained into the socket once connected. */
    private val outbox = Channel<String>(Channel.UNLIMITED)

    fun attach(subscriptionId: String, connectionId: String, sinceSeq: Long? = null) {
        outbox.trySend(WsClientMessage.attach(subscriptionId, connectionId, sinceSeq))
    }

    fun detach(subscriptionId: String) {
        outbox.trySend(WsClientMessage.detach(subscriptionId))
    }

    fun ping() {
        outbox.trySend(WsClientMessage.ping())
    }

    /**
     * Connect and stream frames. Cold: connecting happens on collect, and the
     * socket is closed when the collector is cancelled. Always terminates with a
     * [StreamFrame.Closed].
     */
    fun frames(): Flow<StreamFrame> = channelFlow {
        val producer = this
        try {
            http.webSocket(
                urlString = url,
                request = { header(HttpHeaders.SecWebSocketProtocol, subprotocols) },
            ) {
                // Drain the outbox into the socket for the life of the session.
                val pump = launch {
                    for (message in outbox) outgoing.send(Frame.Text(message))
                }
                try {
                    for (frame in incoming) {
                        if (frame is Frame.Text) {
                            parseFrame(frame.readText(), CodegJson.response)?.let { producer.send(it) }
                        }
                    }
                } finally {
                    pump.cancel()
                }
            }
            producer.send(StreamFrame.Closed(null))
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            producer.send(StreamFrame.Closed(e.message))
        }
    }

    companion object {
        /**
         * Route one raw text frame into a [StreamFrame]. The socket multiplexes
         * the legacy firehose (`{channel,payload}`) with the attach protocol
         * (`{type,...}`); we disambiguate on which key is present. Unknown or
         * unparseable frames yield `null` (dropped), never an exception — exposed
         * as a pure function so the routing is unit-testable without a socket.
         */
        fun parseFrame(text: String, json: Json): StreamFrame? {
            val root = runCatching { json.parseToJsonElement(text) }.getOrNull() as? JsonObject
                ?: return null

            // Legacy global side-channel frames.
            (root["channel"] as? JsonPrimitive)?.let { ch ->
                return if (ch.contentOrNull == "__ready__") StreamFrame.Ready else null
            }

            // Attach-protocol frames.
            val type = (root["type"] as? JsonPrimitive)?.contentOrNull ?: return null
            return when (type) {
                "snapshot" -> (root["snapshot"] as? JsonObject)?.let {
                    StreamFrame.Snapshot(LiveSessionSnapshot.fromWire(it, json))
                }
                "replay" -> {
                    val events = (root["events"] as? JsonArray)?.mapNotNull { e ->
                        (e as? JsonObject)?.let { EventEnvelope.fromWire(it, json) }
                    }.orEmpty()
                    StreamFrame.Replay(events)
                }
                "event" -> (root["envelope"] as? JsonObject)?.let {
                    StreamFrame.Event(EventEnvelope.fromWire(it, json))
                }
                "detached" -> StreamFrame.Detached((root["reason"] as? JsonPrimitive)?.contentOrNull ?: "")
                "pong" -> StreamFrame.Pong
                else -> null
            }
        }

        /** Derive the `ws(s)://host/ws/events` URL from an `http(s)` base URL. */
        fun websocketUrl(rawBaseUrl: String): String {
            val base = CodegClient.normalizeBaseUrl(rawBaseUrl)
            val scheme = if (base.startsWith("https", ignoreCase = true)) "wss" else "ws"
            val authority = base.substringAfter("://", base)
            return "$scheme://$authority/ws/events"
        }
    }
}
