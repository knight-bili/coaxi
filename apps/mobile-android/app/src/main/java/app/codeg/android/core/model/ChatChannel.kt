package app.codeg.android.core.model

import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.PrimitiveSerialDescriptor
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonPrimitive

/**
 * The three chat-channel backends codeg supports (`channel_type` on the wire).
 * Closed + authoritative server-side. A channel's type is immutable after
 * creation (the update call has no type field), mirroring the web.
 */
@Serializable
enum class ChannelType(val wire: String) {
    @SerialName("lark") LARK("lark"),
    @SerialName("telegram") TELEGRAM("telegram"),
    @SerialName("weixin") WEIXIN("weixin");

    val displayName: String
        get() = when (this) {
            LARK -> "Lark / Feishu"
            TELEGRAM -> "Telegram"
            WEIXIN -> "WeChat"
        }

    /** Label for the keyring-stored secret, or null when the type has no manual
     *  token (weixin sets its token via the QR flow). */
    val secretLabel: String?
        get() = when (this) {
            LARK -> "App Secret"
            TELEGRAM -> "Bot Token"
            WEIXIN -> null
        }

    companion object {
        fun fromWire(raw: String): ChannelType = entries.firstOrNull { it.wire == raw } ?: TELEGRAM
    }
}

/** Live connection state (`get_chat_channel_status`). Lenient: an unknown value
 *  falls back to [DISCONNECTED] rather than failing the whole list. */
@Serializable(with = ChannelConnectionStatus.Serializer::class)
enum class ChannelConnectionStatus(val wire: String) {
    CONNECTED("connected"),
    CONNECTING("connecting"),
    DISCONNECTED("disconnected"),
    ERROR("error");

    val label: String
        get() = when (this) {
            CONNECTED -> "Connected"
            CONNECTING -> "Connecting"
            DISCONNECTED -> "Disconnected"
            ERROR -> "Error"
        }

    object Serializer : KSerializer<ChannelConnectionStatus> {
        override val descriptor: SerialDescriptor =
            PrimitiveSerialDescriptor("ChannelConnectionStatus", PrimitiveKind.STRING)
        override fun serialize(encoder: Encoder, value: ChannelConnectionStatus) = encoder.encodeString(value.wire)
        override fun deserialize(decoder: Decoder): ChannelConnectionStatus =
            entries.firstOrNull { it.wire == decoder.decodeString() } ?: DISCONNECTED
    }
}

/** A configured chat channel (`list_chat_channels`). `configJson` is a raw JSON
 *  string (per-type keys are snake_case inside it — see [ChannelConfig]). */
@Serializable
data class ChatChannelInfo(
    val id: Int,
    val name: String,
    val channelType: ChannelType,
    val enabled: Boolean = false,
    val configJson: String = "{}",
    val eventFilterJson: String? = null,
    val dailyReportEnabled: Boolean = false,
    val dailyReportTime: String? = null,
) {
    fun withEnabled(value: Boolean): ChatChannelInfo = copy(enabled = value)
}

/** One row of `get_chat_channel_status` (joined into the list by `channelId`). */
@Serializable
data class ChannelStatusInfo(
    val channelId: Int,
    val name: String = "",
    val channelType: ChannelType,
    val status: ChannelConnectionStatus = ChannelConnectionStatus.DISCONNECTED,
)

/** A delivered/received message log entry (`list_chat_channel_messages`). */
@Serializable
data class ChatChannelMessageLog(
    val id: Int,
    val channelId: Int,
    val direction: String = "",
    val messageType: String = "",
    val contentPreview: String = "",
    val status: String = "",
    val errorDetail: String? = null,
    val createdAt: String? = null,
) {
    val isInbound: Boolean get() = direction.lowercase() == "inbound"
    val failed: Boolean get() = status.lowercase() == "failed"
}

/** A global outbound webhook (`get/set_chat_event_webhooks`). */
@Serializable
data class WebhookConfig(val url: String, val enabled: Boolean)

/** Weixin login QR (`weixin_get_qrcode`). `qrcodeImgContent` is a base64 PNG
 *  (optionally a `data:` URI). */
@Serializable
data class WeixinQrcode(val qrcodeId: String, val qrcodeImgContent: String)

/** `weixin_check_qrcode` result — only the status is ever returned. */
@Serializable
data class WeixinQrStatus(val status: String = "")

/** Typed view over a channel's `config_json`: telegram `{chat_id}`, lark
 *  `{app_id, chat_id}`, weixin `{base_url}`. Keys are snake_case to match the server. */
data class ChannelConfig(
    var chatId: String = "",
    var appId: String = "",
    var baseUrl: String = "",
) {
    /** Build the `config_json` string for [type] with snake_case keys. */
    fun toJson(type: ChannelType): String {
        val obj = when (type) {
            ChannelType.TELEGRAM -> mapOf("chat_id" to chatId.trim())
            ChannelType.LARK -> mapOf("app_id" to appId.trim(), "chat_id" to chatId.trim())
            ChannelType.WEIXIN -> mapOf("base_url" to baseUrl.trim().ifEmpty { WEIXIN_DEFAULT_BASE_URL })
        }
        return Json.encodeToString(JsonObject.serializer(), JsonObject(obj.toSortedMap().mapValues { JsonPrimitive(it.value) }))
    }

    /** A short human-readable summary for the detail screen. */
    fun summary(type: ChannelType): String = when (type) {
        ChannelType.TELEGRAM -> if (chatId.isEmpty()) "No chat ID" else "Chat $chatId"
        ChannelType.LARK -> listOf(appId, chatId).filter { it.isNotEmpty() }.joinToString(" · ")
        ChannelType.WEIXIN -> baseUrl.ifEmpty { WEIXIN_DEFAULT_BASE_URL }
    }

    companion object {
        const val WEIXIN_DEFAULT_BASE_URL = "https://ilinkai.weixin.qq.com"

        private val lenient = Json { ignoreUnknownKeys = true; isLenient = true }

        fun parse(json: String?): ChannelConfig {
            val obj = json?.takeIf { it.isNotBlank() }?.let {
                runCatching { lenient.parseToJsonElement(it) as? JsonObject }.getOrNull()
            } ?: return ChannelConfig()
            fun str(key: String) = (obj[key] as? JsonPrimitive)?.contentOrNull?.trim().orEmpty()
            return ChannelConfig(chatId = str("chat_id"), appId = str("app_id"), baseUrl = str("base_url"))
        }
    }
}

/** Chat-event types that can be forwarded (`get/set_chat_event_filter`). A `null`
 *  stored filter means "all on except `user_prompt_sent`". */
object ChatEventCatalog {
    data class Item(val id: String, val label: String, val defaultOn: Boolean, val note: String? = null)

    val all: List<Item> = listOf(
        Item("turn_complete", "Turn complete", true),
        Item("error", "Errors", true),
        Item("permission_request", "Permission requests", true),
        Item("question_request", "Questions", true),
        Item("user_prompt_sent", "User prompts", false, "Exports your prompt text"),
    )

    val defaultEnabled: Set<String> get() = all.filter { it.defaultOn }.map { it.id }.toSet()
}

/** Languages the channel bot can reply in (`get/set_chat_message_language`). */
object ChatLanguageCatalog {
    val options: List<Pair<String, String>> = listOf(
        "en" to "English", "zh-cn" to "简体中文", "zh-tw" to "繁體中文",
        "ja" to "日本語", "ko" to "한국어", "es" to "Español",
        "de" to "Deutsch", "fr" to "Français", "pt" to "Português", "ar" to "العربية",
    )

    fun label(code: String): String = options.firstOrNull { it.first == code.lowercase() }?.second ?: code
}
