import Foundation

// Pi (self-extensible coding agent) config model — ported from the web
// `PiConfigPanel`. pi has three concerns, each with its own store:
//  • Credentials/model → pi's native ~/.pi/agent/{settings,auth,models}.json via
//    `acp_update_pi_config` / `acp_load_pi_config`.
//  • Runtime (bring-your-own-pi) → a default↔custom toggle writing PI_ACP_PI_COMMAND
//    (+ optional dir overrides) into the per-agent env.
//  • Workspace trust → the PI_ACP_TRUST_WORKSPACE env flag.

/// Response of `acp_load_pi_config` (camelCase wire; the shared `.convertFromSnakeCase`
/// decoder leaves underscore-free keys untouched, so natural props decode directly).
struct PiConfigProjection: Decodable, Sendable {
    var defaultProvider: String?
    var defaultModel: String?
    var defaultThinkingLevel: String?
    var authProviders: [String]
    var customProviders: [PiCustomProvider]
}

struct PiCustomProvider: Decodable, Sendable, Hashable, Identifiable {
    let id: String
    let baseUrl: String
    let api: String
}

/// Response of `acp_validate_pi_command`. Not-found is a normal result (found=false).
struct PiCommandValidation: Decodable, Sendable, Equatable {
    let found: Bool
    let resolvedPath: String?
    let version: String?
}

/// Which pi binary pi-acp spawns.
enum PiRuntimeMode: String, CaseIterable, Sendable, Hashable {
    case `default`
    case custom
}

enum PiEnvKeys {
    static let command = "PI_ACP_PI_COMMAND"
    static let configDir = "PI_CODING_AGENT_DIR"
    static let sessionDir = "PI_CODING_AGENT_SESSION_DIR"
    /// Absent or any value other than "0" ⇒ workspace-trust seeding enabled.
    static let trustWorkspace = "PI_ACP_TRUST_WORKSPACE"
}

/// Sentinel Select value that switches the credentials form to custom mode.
let piCustomProviderSentinel = "__custom__"

let piThinkingLevels = ["off", "minimal", "low", "medium", "high", "xhigh"]

/// Wire protocols pi accepts for a custom provider in `models.json`.
let piCustomApiProtocols = ["openai-completions", "openai-responses", "anthropic-messages", "google-generative-ai"]

/// Curated built-in providers (id → brand label). Mirrors the web's `PI_BUILTIN_PROVIDERS`
/// subset of pi's `env-api-keys.ts`. Labels are brand names (not localized). Special-auth
/// providers (azure/bedrock/vertex/…) are omitted — they don't fit the single-key flow.
let piBuiltinProviders: [(id: String, label: String)] = [
    ("anthropic", "Anthropic"),
    ("openai", "OpenAI"),
    ("google", "Google Gemini"),
    ("openrouter", "OpenRouter"),
    ("vercel-ai-gateway", "Vercel AI Gateway"),
    ("xai", "xAI"),
    ("deepseek", "DeepSeek"),
    ("groq", "Groq"),
    ("cerebras", "Cerebras"),
    ("mistral", "Mistral"),
    ("nvidia", "NVIDIA NIM"),
    ("together", "Together AI"),
    ("fireworks", "Fireworks"),
    ("huggingface", "Hugging Face"),
    ("kimi-coding", "Kimi For Coding"),
    ("moonshotai", "Moonshot AI"),
    ("moonshotai-cn", "Moonshot AI (China)"),
    ("zai", "Z.AI Coding Plan (Global)"),
    ("zai-coding-cn", "Z.AI Coding Plan (China)"),
    ("minimax", "MiniMax"),
    ("minimax-cn", "MiniMax (China)"),
    ("ant-ling", "Ant Ling"),
    ("xiaomi", "Xiaomi MiMo"),
    ("xiaomi-token-plan-cn", "Xiaomi MiMo Token Plan (China)"),
    ("xiaomi-token-plan-ams", "Xiaomi MiMo Token Plan (Amsterdam)"),
    ("xiaomi-token-plan-sgp", "Xiaomi MiMo Token Plan (Singapore)"),
    ("opencode", "OpenCode Zen"),
    ("opencode-go", "OpenCode Go"),
]

enum PiConfig {
    /// Build the env map to persist for pi's runtime. `custom` mode writes
    /// PI_ACP_PI_COMMAND (+ optional dir overrides); `default` clears all three so
    /// pi-acp falls back to the `pi` on PATH. Preserves unrelated env (incl. the
    /// trust flag). Mirrors the web `buildPiRuntimeEnv`.
    static func buildRuntimeEnv(_ prevEnv: [String: String], mode: PiRuntimeMode,
                                command: String, configDir: String, sessionDir: String) -> [String: String] {
        var env = prevEnv
        let cmd = command.trimmingCharacters(in: .whitespaces)
        if mode == .custom && !cmd.isEmpty {
            env[PiEnvKeys.command] = cmd
            assign(&env, PiEnvKeys.configDir, configDir)
            assign(&env, PiEnvKeys.sessionDir, sessionDir)
        } else {
            env.removeValue(forKey: PiEnvKeys.command)
            env.removeValue(forKey: PiEnvKeys.configDir)
            env.removeValue(forKey: PiEnvKeys.sessionDir)
        }
        return env
    }

    private static func assign(_ env: inout [String: String], _ key: String, _ value: String) {
        let t = value.trimmingCharacters(in: .whitespaces)
        if t.isEmpty { env.removeValue(forKey: key) } else { env[key] = t }
    }
}
