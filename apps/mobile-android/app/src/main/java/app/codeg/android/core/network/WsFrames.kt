package app.codeg.android.core.network

import app.codeg.android.core.model.EventEnvelope
import app.codeg.android.core.model.LiveSessionSnapshot
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/**
 * Client→server attach-protocol messages (Rust `ClientMsg`, tagged by `action`,
 * snake_case keys). Built by hand as compact JSON — the request encoder does not
 * convert keys, and there are only three shapes.
 */
object WsClientMessage {
    fun attach(subscriptionId: String, connectionId: String, sinceSeq: Long?): String =
        buildJsonObject {
            put("action", "attach")
            put("subscription_id", subscriptionId)
            put("connection_id", connectionId)
            if (sinceSeq != null) put("since_seq", sinceSeq)
        }.toString()

    fun detach(subscriptionId: String): String =
        buildJsonObject {
            put("action", "detach")
            put("subscription_id", subscriptionId)
        }.toString()

    fun ping(): String = """{"action":"ping"}"""
}

/**
 * A decoded server→client frame from `/ws/events`. The socket multiplexes the
 * legacy `{channel,payload}` firehose with the attach protocol `{type,...}`;
 * [EventStream] routes both into this one type (mirroring the iOS `Frame`).
 */
sealed interface StreamFrame {
    /** Legacy `{channel:"__ready__"}` — the link is usable; safe to attach/prompt. */
    data object Ready : StreamFrame
    data class Snapshot(val snapshot: LiveSessionSnapshot) : StreamFrame
    data class Replay(val events: List<EventEnvelope>) : StreamFrame
    data class Event(val envelope: EventEnvelope) : StreamFrame
    data class Detached(val reason: String) : StreamFrame
    data object Pong : StreamFrame
    /** The socket closed (gracefully or with an error reason). */
    data class Closed(val reason: String?) : StreamFrame
}
