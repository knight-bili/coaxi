package app.codeg.android.core.model

import kotlinx.serialization.Serializable

/**
 * Pi (self-extensible coding agent) config model — ported from iOS `AgentConfigPi.swift`
 * (itself from the web `PiConfigPanel`). pi has three concerns, each with its own store:
 *  - Credentials/model → pi's native ~/.pi/agent/{settings,auth,models}.json via
 *    `acp_update_pi_config` / `acp_load_pi_config`.
 *  - Runtime (bring-your-own-pi) → a default↔custom toggle writing PI_ACP_PI_COMMAND
 *    (+ optional dir overrides) into the per-agent env.
 *  - Workspace trust → the PI_ACP_TRUST_WORKSPACE env flag.
 *
 * The load/validate responses use a **camelCase** wire (unlike the usual snake_case
 * responses), so they are decoded with `CodegClient`'s camel decoder.
 */

/** Response of `acp_load_pi_config` (camelCase wire). */
@Serializable
data class PiConfigProjection(
    val defaultProvider: String? = null,
    val defaultModel: String? = null,
    val defaultThinkingLevel: String? = null,
    val authProviders: List<String> = emptyList(),
    val customProviders: List<PiCustomProvider> = emptyList(),
)

@Serializable
data class PiCustomProvider(val id: String, val baseUrl: String, val api: String)

/** Response of `acp_validate_pi_command`. Not-found is a normal result (found=false). */
@Serializable
data class PiCommandValidation(
    val found: Boolean = false,
    val resolvedPath: String? = null,
    val version: String? = null,
)

/** Which pi binary pi-acp spawns. */
enum class PiRuntimeMode { DEFAULT, CUSTOM }

object PiEnvKeys {
    const val COMMAND = "PI_ACP_PI_COMMAND"
    const val CONFIG_DIR = "PI_CODING_AGENT_DIR"
    const val SESSION_DIR = "PI_CODING_AGENT_SESSION_DIR"
    /** Absent or any value other than "0" ⇒ workspace-trust seeding enabled. */
    const val TRUST_WORKSPACE = "PI_ACP_TRUST_WORKSPACE"
}

/** Sentinel Select value that switches the credentials form to custom mode. */
const val PI_CUSTOM_PROVIDER_SENTINEL = "__custom__"

val piThinkingLevels: List<String> = listOf("off", "minimal", "low", "medium", "high", "xhigh")

/** Wire protocols pi accepts for a custom provider in `models.json`. */
val piCustomApiProtocols: List<String> =
    listOf("openai-completions", "openai-responses", "anthropic-messages", "google-generative-ai")

/**
 * Curated built-in providers (id → brand label). Mirrors the web's `PI_BUILTIN_PROVIDERS`
 * subset of pi's `env-api-keys.ts`. Labels are brand names (not localized). Special-auth
 * providers (azure/bedrock/vertex/…) are omitted — they don't fit the single-key flow.
 */
val piBuiltinProviders: List<Pair<String, String>> = listOf(
    "anthropic" to "Anthropic",
    "openai" to "OpenAI",
    "google" to "Google Gemini",
    "openrouter" to "OpenRouter",
    "vercel-ai-gateway" to "Vercel AI Gateway",
    "xai" to "xAI",
    "deepseek" to "DeepSeek",
    "groq" to "Groq",
    "cerebras" to "Cerebras",
    "mistral" to "Mistral",
    "nvidia" to "NVIDIA NIM",
    "together" to "Together AI",
    "fireworks" to "Fireworks",
    "huggingface" to "Hugging Face",
    "kimi-coding" to "Kimi For Coding",
    "moonshotai" to "Moonshot AI",
    "moonshotai-cn" to "Moonshot AI (China)",
    "zai" to "Z.AI Coding Plan (Global)",
    "zai-coding-cn" to "Z.AI Coding Plan (China)",
    "minimax" to "MiniMax",
    "minimax-cn" to "MiniMax (China)",
    "ant-ling" to "Ant Ling",
    "xiaomi" to "Xiaomi MiMo",
    "xiaomi-token-plan-cn" to "Xiaomi MiMo Token Plan (China)",
    "xiaomi-token-plan-ams" to "Xiaomi MiMo Token Plan (Amsterdam)",
    "xiaomi-token-plan-sgp" to "Xiaomi MiMo Token Plan (Singapore)",
    "opencode" to "OpenCode Zen",
    "opencode-go" to "OpenCode Go",
)

object PiConfig {
    /**
     * Build the env map to persist for pi's runtime. `custom` mode writes
     * PI_ACP_PI_COMMAND (+ optional dir overrides); `default` clears all three so
     * pi-acp falls back to the `pi` on PATH. Preserves unrelated env (incl. the
     * trust flag). Mirrors the web `buildPiRuntimeEnv`.
     */
    fun buildRuntimeEnv(
        prevEnv: Map<String, String>,
        mode: PiRuntimeMode,
        command: String,
        configDir: String,
        sessionDir: String,
    ): Map<String, String> {
        val env = LinkedHashMap(prevEnv)
        val cmd = command.trim()
        if (mode == PiRuntimeMode.CUSTOM && cmd.isNotEmpty()) {
            env[PiEnvKeys.COMMAND] = cmd
            assign(env, PiEnvKeys.CONFIG_DIR, configDir)
            assign(env, PiEnvKeys.SESSION_DIR, sessionDir)
        } else {
            env.remove(PiEnvKeys.COMMAND)
            env.remove(PiEnvKeys.CONFIG_DIR)
            env.remove(PiEnvKeys.SESSION_DIR)
        }
        return env
    }

    private fun assign(env: MutableMap<String, String>, key: String, value: String) {
        val t = value.trim()
        if (t.isEmpty()) env.remove(key) else env[key] = t
    }
}
