package app.codeg.android.core.model

import app.codeg.android.core.model.wire.arrayOrNull
import app.codeg.android.core.model.wire.boolOrNull
import app.codeg.android.core.model.wire.longOrNull
import app.codeg.android.core.model.wire.objectOrNull
import app.codeg.android.core.model.wire.stringOrNull
import kotlinx.serialization.KSerializer
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

/** Status of a live ACP connection (Rust `ConnectionStatus`). Unknown → [ERROR]. */
enum class ConnectionStatus(val wire: String) {
    CONNECTING("connecting"),
    CONNECTED("connected"),
    PROMPTING("prompting"),
    DISCONNECTED("disconnected"),
    ERROR("error");

    companion object {
        fun fromWire(raw: String?): ConnectionStatus =
            entries.firstOrNull { it.wire == raw } ?: ERROR
    }
}

/** A block of a user's submitted prompt echoed on the connection stream
 * (Rust `UserMessageBlock`). */
sealed interface UserMessageBlock {
    data class Text(val text: String) : UserMessageBlock
    data class Image(val image: ImageData) : UserMessageBlock
    data object Unknown : UserMessageBlock

    companion object {
        fun fromWire(obj: JsonObject): UserMessageBlock =
            when (obj.stringOrNull("type")) {
                "text" -> Text(obj.stringOrNull("text").orEmpty())
                "image" -> Image(
                    ImageData(
                        data = obj.stringOrNull("data").orEmpty(),
                        mimeType = obj.stringOrNull("mime_type") ?: "image/png",
                        uri = null,
                    ),
                )
                else -> Unknown
            }
    }
}

/**
 * A backend→client ACP event (Rust `AcpEvent`, internally tagged `type`).
 * Decode-only. Decoded by hand from the flattened wire object so unknown event
 * types yield [Unknown] rather than breaking the whole stream — exactly as the
 * iOS decoder does.
 */
sealed interface AcpEvent {
    data class ContentDelta(val text: String) : AcpEvent
    data class Thinking(val text: String) : AcpEvent
    /** [meta] is the agent's opaque ACP extensibility metadata (context compaction,
     *  Grok's `x.ai/tool` identity, …). Absent for most tools/hosts. */
    data class ToolCall(
        val id: String,
        val title: String,
        val kind: String,
        val status: String,
        val content: String?,
        val rawInput: String?,
        val rawOutput: String?,
        val meta: JsonObject? = null,
    ) : AcpEvent
    /** A null [meta] means "unchanged" (the server omits the key), NOT "cleared" —
     *  see `LiveTurnBuilder`'s replace-on-update handling. */
    data class ToolCallUpdate(
        val id: String,
        val title: String?,
        val status: String?,
        val content: String?,
        val rawInput: String?,
        val rawOutput: String?,
        val append: Boolean,
        val meta: JsonObject? = null,
    ) : AcpEvent
    data class TurnComplete(val stopReason: String) : AcpEvent
    data class SessionStarted(val sessionId: String) : AcpEvent
    data class ConversationLinked(val conversationId: Int, val folderId: Int) : AcpEvent
    data class ConversationStatusChanged(val conversationId: Int, val status: ConversationStatus) : AcpEvent
    data class StatusChanged(val status: ConnectionStatus) : AcpEvent
    data class UsageUpdate(val used: Long, val size: Long) : AcpEvent
    data class UserMessage(val messageId: String, val blocks: List<UserMessageBlock>) : AcpEvent
    data class UserPromptSent(val textPreview: String) : AcpEvent
    data class Error(val message: String, val code: String?) : AcpEvent
    /** Agent asks to approve a tool call (also carries ExitPlanMode — the plan
     * rides inside [toolCall]). Resolve via `acp_respond_permission`. */
    data class PermissionRequest(
        val requestId: String,
        val toolCall: JsonElement,
        val options: List<PermissionOption>,
    ) : AcpEvent
    data class PermissionResolved(val requestId: String) : AcpEvent
    /** Agent asks one or more multiple-choice questions. Resolve via
     * `acp_answer_question`. */
    data class QuestionRequest(val questionId: String, val questions: List<QuestionSpec>) : AcpEvent
    data class QuestionResolved(val questionId: String) : AcpEvent
    /**
     * Grok's native `exit_plan_mode`: the agent finished planning and is BLOCKED on the
     * user's approval before it leaves plan mode and starts implementing. Resolve via
     * `acp_answer_plan_approval`; also carried on the session snapshot so a mid-turn
     * attach recovers it.
     */
    data class PlanApprovalRequest(
        val approvalId: String,
        val toolCallId: String,
        val planMarkdown: String,
    ) : AcpEvent
    /** A pending plan approval was answered (from any client) or canceled — clear the
     *  card. Idempotent on apply. */
    data class PlanApprovalResolved(val approvalId: String) : AcpEvent
    /** The agent's live plan / TODO list (display only; does not block the turn). */
    data class PlanUpdate(val entries: List<PlanEntry>) : AcpEvent
    data class Unknown(val type: String) : AcpEvent

    companion object {
        /**
         * Decode an [AcpEvent] from the flattened wire object. [json] is used to
         * decode the nested structured lists (options / questions / plan entries)
         * with the snake_case naming strategy; everything else is read by hand.
         */
        fun fromWire(obj: JsonObject, json: Json): AcpEvent {
            return when (val type = obj.stringOrNull("type").orEmpty()) {
                "content_delta" -> ContentDelta(obj.stringOrNull("text").orEmpty())
                "thinking" -> Thinking(obj.stringOrNull("text").orEmpty())
                "tool_call" -> ToolCall(
                    id = obj.stringOrNull("tool_call_id").orEmpty(),
                    title = obj.stringOrNull("title").orEmpty(),
                    kind = obj.stringOrNull("kind").orEmpty(),
                    status = obj.stringOrNull("status").orEmpty(),
                    content = obj.stringOrNull("content"),
                    rawInput = obj.stringOrNull("raw_input"),
                    rawOutput = obj.stringOrNull("raw_output"),
                    meta = obj.objectOrNull("meta"),
                )
                "tool_call_update" -> ToolCallUpdate(
                    id = obj.stringOrNull("tool_call_id").orEmpty(),
                    title = obj.stringOrNull("title"),
                    status = obj.stringOrNull("status"),
                    content = obj.stringOrNull("content"),
                    rawInput = obj.stringOrNull("raw_input"),
                    rawOutput = obj.stringOrNull("raw_output"),
                    append = obj.boolOrNull("raw_output_append") ?: false,
                    meta = obj.objectOrNull("meta"),
                )
                "turn_complete" -> TurnComplete(obj.stringOrNull("stop_reason") ?: "end_turn")
                "session_started" -> SessionStarted(obj.stringOrNull("session_id").orEmpty())
                "conversation_linked" -> ConversationLinked(
                    conversationId = obj.intOrZero("conversation_id"),
                    folderId = obj.intOrZero("folder_id"),
                )
                "conversation_status_changed" -> ConversationStatusChanged(
                    conversationId = obj.intOrZero("conversation_id"),
                    status = ConversationStatus.fromWire(obj.stringOrNull("status").orEmpty()),
                )
                "status_changed" -> StatusChanged(ConnectionStatus.fromWire(obj.stringOrNull("status")))
                "usage_update" -> UsageUpdate(
                    used = obj.longOrNull("used") ?: 0,
                    size = obj.longOrNull("size") ?: 0,
                )
                "user_message" -> UserMessage(
                    messageId = obj.stringOrNull("message_id").orEmpty(),
                    blocks = obj.arrayOrNull("blocks")
                        ?.mapNotNull { (it as? JsonObject)?.let(UserMessageBlock::fromWire) }
                        ?: emptyList(),
                )
                "user_prompt_sent" -> UserPromptSent(obj.stringOrNull("text_preview").orEmpty())
                "error" -> Error(
                    message = obj.stringOrNull("message") ?: "Unknown error",
                    code = obj.stringOrNull("code"),
                )
                "permission_request" -> PermissionRequest(
                    requestId = obj.stringOrNull("request_id").orEmpty(),
                    toolCall = obj["tool_call"] ?: JsonNull,
                    options = decodeList(json, obj["options"], PermissionOption.serializer()),
                )
                "permission_resolved" -> PermissionResolved(obj.stringOrNull("request_id").orEmpty())
                "question_request" -> QuestionRequest(
                    questionId = obj.stringOrNull("question_id").orEmpty(),
                    questions = decodeList(json, obj["questions"], QuestionSpec.serializer()),
                )
                "question_resolved" -> QuestionResolved(obj.stringOrNull("question_id").orEmpty())
                "plan_approval_request" -> PlanApprovalRequest(
                    approvalId = obj.stringOrNull("approval_id").orEmpty(),
                    toolCallId = obj.stringOrNull("tool_call_id").orEmpty(),
                    // An empty/missing plan still opens the approval surface (Grok's
                    // plan-mode doc allows it) — the card shows an empty-state notice.
                    planMarkdown = obj.stringOrNull("plan_markdown").orEmpty(),
                )
                "plan_approval_resolved" -> PlanApprovalResolved(obj.stringOrNull("approval_id").orEmpty())
                "plan_update" -> PlanUpdate(
                    entries = decodeList(json, obj["entries"], PlanEntry.serializer()),
                )
                else -> Unknown(type)
            }
        }

        private fun JsonObject.intOrZero(key: String): Int =
            (this[key] as? kotlinx.serialization.json.JsonPrimitive)
                ?.let { runCatching { it.content.toInt() }.getOrNull() } ?: 0

        private fun <T> decodeList(json: Json, element: JsonElement?, serializer: KSerializer<T>): List<T> {
            val arr = element as? JsonArray ?: return emptyList()
            return runCatching { json.decodeFromJsonElement(ListSerializer(serializer), arr) }
                .getOrDefault(emptyList())
        }
    }
}

/**
 * The flat ACP event envelope (Rust `EventEnvelope`): `{seq, connection_id,
 * type, ...payload}`. The payload is flattened, so we read `seq`/`connectionId`
 * and then the event from the same object.
 */
data class EventEnvelope(
    val seq: Long,
    val connectionId: String,
    val event: AcpEvent,
) {
    companion object {
        fun fromWire(obj: JsonObject, json: Json): EventEnvelope =
            EventEnvelope(
                seq = obj.longOrNull("seq") ?: 0,
                connectionId = obj.stringOrNull("connection_id").orEmpty(),
                event = AcpEvent.fromWire(obj, json),
            )
    }
}

/**
 * Projection of the live session snapshot delivered on attach (Rust
 * `LiveSessionSnapshot`). The scalar fields drive the send path; [liveMessage] +
 * [activeToolCalls] let `reattachIfLive` rebuild an in-flight assistant turn (and
 * [pendingPermission] / [pendingQuestion] its interactive card) when opening a
 * session whose turn is still running. All parsing is tolerant — a shape surprise
 * yields null/empty, never a thrown frame (the send path decodes this on every
 * `.snapshot`).
 */
data class LiveSessionSnapshot(
    val connectionId: String?,
    val conversationId: Int?,
    val folderId: Int?,
    val status: ConnectionStatus?,
    val externalId: String?,
    val eventSeq: Long?,
    val liveMessage: LiveMessageSnapshot?,
    val activeToolCalls: List<ToolCallStateSnapshot>,
    val pendingPermission: PendingPermissionSnapshot?,
    val pendingQuestion: PendingQuestionSnapshot?,
    val pendingPlanApproval: PendingPlanApprovalSnapshot?,
) {
    companion object {
        fun fromWire(obj: JsonObject, json: Json): LiveSessionSnapshot =
            LiveSessionSnapshot(
                connectionId = obj.stringOrNull("connection_id"),
                conversationId = obj.intOrNull("conversation_id"),
                folderId = obj.intOrNull("folder_id"),
                status = obj.stringOrNull("status")?.let(ConnectionStatus::fromWire),
                externalId = obj.stringOrNull("external_id"),
                eventSeq = obj.longOrNull("event_seq"),
                liveMessage = obj.objectOrNull("live_message")
                    ?.let { LiveMessageSnapshot.fromWire(it, json) },
                activeToolCalls = obj.arrayOrNull("active_tool_calls")
                    ?.mapNotNull { (it as? JsonObject)?.let(ToolCallStateSnapshot::fromWire) }
                    ?: emptyList(),
                pendingPermission = obj.objectOrNull("pending_permission")
                    ?.let { PendingPermissionSnapshot.fromWire(it, json) },
                pendingQuestion = obj.objectOrNull("pending_question")
                    ?.let { PendingQuestionSnapshot.fromWire(it, json) },
                pendingPlanApproval = obj.objectOrNull("pending_plan_approval")
                    ?.let(PendingPlanApprovalSnapshot::fromWire),
            )

        private fun JsonObject.intOrNull(key: String): Int? =
            (this[key] as? JsonPrimitive)
                ?.let { runCatching { it.content.toInt() }.getOrNull() }
    }
}

/** The in-flight assistant message carried by a snapshot (Rust `LiveMessage`). */
data class LiveMessageSnapshot(val content: List<LiveContentBlockSnapshot>) {
    companion object {
        fun fromWire(obj: JsonObject, json: Json): LiveMessageSnapshot =
            LiveMessageSnapshot(
                content = obj.arrayOrNull("content")
                    ?.mapNotNull { (it as? JsonObject)?.let { o -> LiveContentBlockSnapshot.fromWire(o, json) } }
                    ?: emptyList(),
            )
    }
}

/** One ordered block of the in-flight message (Rust `LiveContentBlock`,
 *  `kind`-tagged). [ToolCallRef] points into [LiveSessionSnapshot.activeToolCalls]. */
sealed interface LiveContentBlockSnapshot {
    data class Text(val text: String) : LiveContentBlockSnapshot
    data class Thinking(val text: String) : LiveContentBlockSnapshot
    data class ToolCallRef(val toolCallId: String) : LiveContentBlockSnapshot
    data class Plan(val entries: List<PlanEntry>) : LiveContentBlockSnapshot
    data object Unknown : LiveContentBlockSnapshot

    companion object {
        fun fromWire(obj: JsonObject, json: Json): LiveContentBlockSnapshot =
            when (obj.stringOrNull("kind")) {
                "text" -> Text(obj.stringOrNull("text").orEmpty())
                "thinking" -> Thinking(obj.stringOrNull("text").orEmpty())
                "tool_call_ref" -> ToolCallRef(obj.stringOrNull("tool_call_id").orEmpty())
                "plan" -> Plan(
                    (obj["entries"] as? JsonArray)?.let {
                        runCatching { json.decodeFromJsonElement(ListSerializer(PlanEntry.serializer()), it) }
                            .getOrDefault(emptyList())
                    } ?: emptyList(),
                )
                else -> Unknown
            }
    }
}

/** One active tool call carried by a snapshot (Rust `ToolCallState`). `kind` /
 *  `status` are bare strings; `input` / `output` are freeform JSON (`output` is
 *  `{kind,...}`-tagged — text/error/json). */
data class ToolCallStateSnapshot(
    val id: String,
    val kind: String,
    val label: String,
    val status: String,
    val input: JsonElement?,
    val output: JsonElement?,
    val content: String?,
    /** The agent's opaque ACP metadata, as on the live event path. */
    val meta: JsonObject? = null,
) {
    /** The agent's argument JSON as a preview string (matches the event path's
     *  `raw_input`, which already arrives as a string). */
    val inputPreview: String?
        get() = when (val i = input) {
            null, JsonNull -> null
            is JsonPrimitive -> if (i.isString) i.content else i.toString()
            else -> i.toString()
        }

    /** Flatten the `{kind,...}`-tagged output into plain text for the live card. */
    val outputText: String
        get() {
            val o = output as? JsonObject
                ?: return (output as? JsonPrimitive)?.let { if (it.isString) it.content else it.toString() }.orEmpty()
            return when (o.stringOrNull("kind")) {
                "text" -> o.stringOrNull("content").orEmpty()
                "error" -> o.stringOrNull("message").orEmpty()
                "json" -> o["value"]?.toString().orEmpty()
                else -> ""
            }
        }

    /** The snapshot `status` normalized to the lowercase form the live tool card
     *  interprets — snapshot statuses may arrive PascalCase (iOS `normalizedToolStatus`). */
    val normalizedStatus: String
        get() = when (status.lowercase()) {
            "inprogress", "in_progress", "in-progress", "running" -> "in_progress"
            "completed", "done", "success" -> "completed"
            "failed", "error" -> "failed"
            "pending" -> "pending"
            else -> status.lowercase()
        }

    companion object {
        fun fromWire(obj: JsonObject): ToolCallStateSnapshot =
            ToolCallStateSnapshot(
                id = obj.stringOrNull("id").orEmpty(),
                kind = obj.stringOrNull("kind").orEmpty(),
                label = obj.stringOrNull("label").orEmpty(),
                status = obj.stringOrNull("status").orEmpty(),
                input = obj["input"],
                output = obj["output"],
                content = obj.stringOrNull("content"),
                meta = obj.objectOrNull("meta"),
            )
    }
}

/** Pending permission carried by a snapshot (Rust `PendingPermissionState`). */
data class PendingPermissionSnapshot(
    val requestId: String,
    val toolCall: JsonElement,
    val options: List<PermissionOption>,
) {
    companion object {
        fun fromWire(obj: JsonObject, json: Json): PendingPermissionSnapshot =
            PendingPermissionSnapshot(
                requestId = obj.stringOrNull("request_id").orEmpty(),
                toolCall = obj["tool_call"] ?: JsonNull,
                options = (obj["options"] as? JsonArray)?.let {
                    runCatching {
                        json.decodeFromJsonElement(ListSerializer(PermissionOption.serializer()), it)
                    }.getOrDefault(emptyList())
                } ?: emptyList(),
            )
    }
}

/**
 * Pending Grok plan approval carried by a snapshot (Rust `PendingPlanApprovalState`),
 * so a client attaching mid-turn recovers the blocked `exit_plan_mode` card instead of
 * watching the turn spin.
 */
data class PendingPlanApprovalSnapshot(
    val approvalId: String,
    val toolCallId: String,
    val planMarkdown: String,
) {
    companion object {
        fun fromWire(obj: JsonObject): PendingPlanApprovalSnapshot =
            PendingPlanApprovalSnapshot(
                approvalId = obj.stringOrNull("approval_id").orEmpty(),
                toolCallId = obj.stringOrNull("tool_call_id").orEmpty(),
                planMarkdown = obj.stringOrNull("plan_markdown").orEmpty(),
            )
    }
}

/** Pending question carried by a snapshot (Rust `PendingQuestionState`). */
data class PendingQuestionSnapshot(
    val questionId: String,
    val questions: List<QuestionSpec>,
) {
    companion object {
        fun fromWire(obj: JsonObject, json: Json): PendingQuestionSnapshot =
            PendingQuestionSnapshot(
                questionId = obj.stringOrNull("question_id").orEmpty(),
                questions = (obj["questions"] as? JsonArray)?.let {
                    runCatching {
                        json.decodeFromJsonElement(ListSerializer(QuestionSpec.serializer()), it)
                    }.getOrDefault(emptyList())
                } ?: emptyList(),
            )
    }
}
