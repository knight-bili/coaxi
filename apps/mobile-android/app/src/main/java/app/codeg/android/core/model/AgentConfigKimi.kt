package app.codeg.android.core.model

import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.intOrNull

/**
 * Kimi Code (Moonshot AI) config model — ported from iOS `AgentConfigKimi.swift`
 * (itself from the web `acp-agent-settings.tsx` Kimi helpers). `kimi acp` gates
 * every session on a stored token and rejects a bare API key, so codeg manages BOTH
 * a `~/.kimi-code/config.toml` provider block AND a synthetic gate token; the panel
 * keeps exactly one source authoritative (apikey / login / raw), enforced by the
 * `acp_update_kimi_code_config` backend. The current state is projected into
 * [AcpAgentInfo.configJson] as a camelCase JSON string ([KimiManagedConfig]).
 */

/** Which credential source is authoritative (also the `mode` wire value for save). */
enum class KimiAuthMode(val wire: String) {
    APIKEY("apikey"),
    LOGIN("login"),
}

/** The six provider `type` values Kimi's config.toml `[providers]` accepts. */
enum class KimiInterfaceType(val wire: String) {
    KIMI("kimi"),
    OPENAI("openai"),
    OPENAI_RESPONSES("openai_responses"),
    ANTHROPIC("anthropic"),
    GOOGLE_GENAI("google-genai"),
    VERTEXAI("vertexai");

    companion object {
        fun fromWire(raw: String?): KimiInterfaceType? = entries.firstOrNull { it.wire == raw }
    }
}

/** Native-provider credential placement: inline `api_key` vs the env sub-table. */
enum class KimiNativeAuthType(val wire: String) {
    API_KEY("api_key"),
    ENV("env");

    companion object {
        fun fromWire(raw: String?): KimiNativeAuthType? = entries.firstOrNull { it.wire == raw }
    }
}

/** Env-mode endpoint: the two Moonshot regions or a custom OpenAI-compatible URL. */
enum class KimiEndpointRegion { INTERNATIONAL, CHINA, CUSTOM }

const val KIMI_BASE_URL_INTERNATIONAL = "https://api.moonshot.ai/v1"
const val KIMI_BASE_URL_CHINA = "https://api.moonshot.cn/v1"
/** Placeholder model id (a real Moonshot coding model) for the model input. */
const val KIMI_MODEL_PLACEHOLDER = "kimi-k2.7-code"

data class KimiInterfaceTypeMeta(
    val value: KimiInterfaceType,
    /** Product label (proper noun — intentionally not localized). */
    val label: String,
    /** Base URL pre-filled when this interface is selected ("" → SDK default). */
    val defaultBaseUrl: String,
    /** vertexai authenticates via GCP ADC, so it exposes no API key field. */
    val usesApiKey: Boolean,
)

val kimiInterfaceTypes: List<KimiInterfaceTypeMeta> = listOf(
    KimiInterfaceTypeMeta(KimiInterfaceType.KIMI, "Kimi / Moonshot", KIMI_BASE_URL_INTERNATIONAL, true),
    KimiInterfaceTypeMeta(KimiInterfaceType.OPENAI, "OpenAI (Chat Completions)", "https://api.openai.com/v1", true),
    KimiInterfaceTypeMeta(KimiInterfaceType.OPENAI_RESPONSES, "OpenAI (Responses)", "https://api.openai.com/v1", true),
    KimiInterfaceTypeMeta(KimiInterfaceType.ANTHROPIC, "Anthropic", "", true),
    KimiInterfaceTypeMeta(KimiInterfaceType.GOOGLE_GENAI, "Google Gemini", "", true),
    KimiInterfaceTypeMeta(KimiInterfaceType.VERTEXAI, "Google Vertex AI", "", false),
)

fun kimiInterfaceMeta(type: KimiInterfaceType): KimiInterfaceTypeMeta =
    kimiInterfaceTypes.firstOrNull { it.value == type } ?: kimiInterfaceTypes[0]

/**
 * Region implied by an env-mode base URL: `.cn` → china, `.ai` or empty →
 * international, any other non-empty endpoint → custom.
 */
fun kimiEndpointRegionFromBaseUrl(baseUrl: String): KimiEndpointRegion {
    val raw = baseUrl.trim().lowercase()
    return when {
        raw.isEmpty() -> KimiEndpointRegion.INTERNATIONAL
        raw.contains("moonshot.cn") -> KimiEndpointRegion.CHINA
        raw.contains("moonshot.ai") -> KimiEndpointRegion.INTERNATIONAL
        else -> KimiEndpointRegion.CUSTOM
    }
}

fun kimiBaseUrlForRegion(region: KimiEndpointRegion, customUrl: String): String = when (region) {
    KimiEndpointRegion.CHINA -> KIMI_BASE_URL_CHINA
    KimiEndpointRegion.CUSTOM -> customUrl.trim()
    KimiEndpointRegion.INTERNATIONAL -> KIMI_BASE_URL_INTERNATIONAL
}

/**
 * Mirror of the backend `load_kimi_code_config_json` projection (camelCase keys),
 * parsed from [AcpAgentInfo.configJson]. Deliberately NOT `apiKey`/`model`/`env` so
 * the projected block never leaks back into the runtime env.
 */
data class KimiManagedConfig(
    val interfaceType: KimiInterfaceType? = null,
    val baseUrl: String? = null,
    val key: String? = null,
    val authType: KimiNativeAuthType? = null,
    val modelId: String? = null,
    val maxContextSize: Int? = null,
    val vertexProject: String? = null,
    val vertexLocation: String? = null,
    val hasManagedBlock: Boolean? = null,
    /** Whether `kimi acp`'s session gate is satisfied (a token file is present). */
    val credentialPresent: Boolean? = null,
    /** Whether that gate token is codeg's synthetic one (vs a real OAuth login). */
    val credentialSynthetic: Boolean? = null,
    val rawConfigToml: String? = null,
) {
    companion object {
        /**
         * Parse the config_json string. Missing/unparseable → an empty config (the
         * panel treats that as "not configured yet"). Unknown enum values fall back to
         * null rather than failing the whole parse.
         */
        fun parse(configJson: String?): KimiManagedConfig {
            val s = configJson?.trim()
            if (s.isNullOrEmpty()) return KimiManagedConfig()
            val obj = JsonConfig.parse(s).config
            if (obj.isEmpty()) return KimiManagedConfig()
            fun str(k: String): String? = obj[k]?.asStringOrNull()
            fun bool(k: String): Boolean? = (obj[k] as? JsonPrimitive)?.booleanOrNull
            fun int(k: String): Int? = (obj[k] as? JsonPrimitive)?.intOrNull
            return KimiManagedConfig(
                interfaceType = KimiInterfaceType.fromWire(str("interfaceType")),
                baseUrl = str("baseUrl"),
                key = str("key"),
                authType = KimiNativeAuthType.fromWire(str("authType")),
                modelId = str("modelId"),
                maxContextSize = int("maxContextSize"),
                vertexProject = str("vertexProject"),
                vertexLocation = str("vertexLocation"),
                hasManagedBlock = bool("hasManagedBlock"),
                credentialPresent = bool("credentialPresent"),
                credentialSynthetic = bool("credentialSynthetic"),
                rawConfigToml = str("rawConfigToml"),
            )
        }
    }
}

/**
 * Initial panel mode: the codeg-managed API-key block wins; otherwise a real
 * (non-synthetic) OAuth login shows login; else default to the API-key form.
 */
fun kimiInitialMode(config: KimiManagedConfig): KimiAuthMode = when {
    config.hasManagedBlock == true -> KimiAuthMode.APIKEY
    config.credentialPresent == true && config.credentialSynthetic != true -> KimiAuthMode.LOGIN
    else -> KimiAuthMode.APIKEY
}
