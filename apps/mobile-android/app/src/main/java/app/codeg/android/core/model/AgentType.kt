package app.codeg.android.core.model

import kotlinx.serialization.KSerializer
import kotlinx.serialization.Serializable
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.PrimitiveSerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder

/**
 * The coding agents codeg can drive. Wire value is snake_case (Rust serde
 * `rename_all = "snake_case"`).
 *
 * Kept free of any Compose/UI types so the model layer stays pure Kotlin — the
 * per-agent accent colours and monogram rendering live in the design system
 * (`AgentVisuals`), keyed off this enum.
 *
 * Unknown future agent types decode to [CLAUDE_CODE] (matching iOS) so one new
 * server-side agent can't break a whole list decode.
 */
@Serializable(with = AgentType.AgentTypeSerializer::class)
enum class AgentType(val wire: String) {
    CLAUDE_CODE("claude_code"),
    CODEX("codex"),
    OPEN_CODE("open_code"),
    GEMINI("gemini"),
    OPEN_CLAW("open_claw"),
    CLINE("cline"),
    HERMES("hermes"),
    CODE_BUDDY("code_buddy"),
    KIMI_CODE("kimi_code"),
    PI("pi"),
    GROK("grok"),
    CURSOR("cursor");

    val displayName: String
        get() = when (this) {
            CLAUDE_CODE -> "Claude Code"
            CODEX -> "Codex CLI"
            OPEN_CODE -> "OpenCode"
            GEMINI -> "Gemini CLI"
            OPEN_CLAW -> "OpenClaw"
            CLINE -> "Cline"
            HERMES -> "Hermes"
            CODE_BUDDY -> "CodeBuddy"
            KIMI_CODE -> "Kimi Code"
            PI -> "Pi"
            GROK -> "Grok"
            CURSOR -> "Cursor"
        }

    /** Short label for dense badges. */
    val shortName: String
        get() = when (this) {
            CLAUDE_CODE -> "Claude"
            CODEX -> "Codex"
            OPEN_CODE -> "OpenCode"
            GEMINI -> "Gemini"
            OPEN_CLAW -> "OpenClaw"
            CLINE -> "Cline"
            HERMES -> "Hermes"
            CODE_BUDDY -> "CodeBuddy"
            KIMI_CODE -> "Kimi"
            PI -> "Pi"
            GROK -> "Grok"
            CURSOR -> "Cursor"
        }

    companion object {
        /**
         * The agent for [raw], falling back to [CLAUDE_CODE] when unrecognised so a
         * single agent-typed field always decodes (matching iOS). List decoders that
         * must not collapse several unknown agents onto one identity should use
         * [knownFromWire] and drop the nulls instead — see `CodegClient.decodeAgentList`.
         */
        fun fromWire(raw: String): AgentType = knownFromWire(raw) ?: CLAUDE_CODE

        /** The agent for [raw], or null when this build doesn't recognise the wire value. */
        fun knownFromWire(raw: String): AgentType? = entries.firstOrNull { it.wire == raw }
    }

    object AgentTypeSerializer : KSerializer<AgentType> {
        override val descriptor =
            PrimitiveSerialDescriptor("com.codeg.AgentType", PrimitiveKind.STRING)

        override fun serialize(encoder: Encoder, value: AgentType) =
            encoder.encodeString(value.wire)

        override fun deserialize(decoder: Decoder): AgentType =
            fromWire(decoder.decodeString())
    }
}
