package app.codeg.android.core.model

import kotlinx.serialization.KSerializer
import kotlinx.serialization.Serializable
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.PrimitiveSerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder

/**
 * Lifecycle status of a conversation row. Free-form string on the wire; modeled
 * as an enum with an [OTHER] escape hatch for forward compatibility. The
 * label/tint presentation lives in the design system (status → colour), keeping
 * this model Compose-free.
 */
@Serializable(with = ConversationStatus.ConversationStatusSerializer::class)
enum class ConversationStatus(val wire: String) {
    IN_PROGRESS("in_progress"),
    PENDING_REVIEW("pending_review"),
    COMPLETED("completed"),
    CANCELLED("cancelled"),
    OTHER("");

    val isLive: Boolean get() = this == IN_PROGRESS

    companion object {
        fun fromWire(raw: String): ConversationStatus =
            entries.firstOrNull { it.wire == raw } ?: OTHER

        /**
         * The statuses a user can assign from the actions menu (excludes the
         * [OTHER] decode escape hatch), in the order the web client lists them.
         */
        val selectable: List<ConversationStatus> =
            listOf(IN_PROGRESS, PENDING_REVIEW, COMPLETED, CANCELLED)
    }

    object ConversationStatusSerializer : KSerializer<ConversationStatus> {
        override val descriptor =
            PrimitiveSerialDescriptor("com.codeg.ConversationStatus", PrimitiveKind.STRING)

        override fun serialize(encoder: Encoder, value: ConversationStatus) =
            encoder.encodeString(value.wire)

        override fun deserialize(decoder: Decoder): ConversationStatus =
            fromWire(decoder.decodeString())
    }
}
