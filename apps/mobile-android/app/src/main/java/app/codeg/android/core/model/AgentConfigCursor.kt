package app.codeg.android.core.model

/**
 * Cursor (`cursor-agent` CLI) settings logic, ported from the web
 * `src/components/settings/cursor-config-panel.tsx` (via iOS `CursorConfig`).
 * Kept out of the UI so the env rules — which decide whether a credential is
 * written or DELETED — are readable on their own and unit-testable.
 *
 * Unlike Kimi/Pi, Cursor has no dedicated save command: its credential/model
 * knobs are plain env vars and its permission rules are a structured patch on
 * `~/.cursor/cli-config.json`, both of which the shared draft + the detail
 * screen's single Save already persist in one `acp_update_agent_env` →
 * `acp_update_agent_config` pass.
 */
object CursorConfig {

    // region Env keys

    /** The Cursor Dashboard account key (headless / server machines). */
    const val API_KEY = "CURSOR_API_KEY"

    /**
     * Written by older builds; `cursor-agent` has NO bring-your-own-endpoint
     * support, so codeg always scrubs this rather than surfacing a field for it.
     */
    const val API_BASE_URL = "CURSOR_API_BASE_URL"

    /** The `--model` id passed at launch. */
    const val MODEL = "CURSOR_MODEL"

    /**
     * codeg-side launch knob: `"1"` inserts the CLI's root `--force` flag (Run
     * Everything) before the `acp` subcommand. The CLI reads no such env var.
     */
    const val FORCE = "CURSOR_FORCE"

    /**
     * codeg-side knob recording the chosen authentication method. The launch path
     * clears an inherited API key in `subscription` mode so the browser login is
     * used. The CLI ignores this var.
     */
    const val AUTH_MODE = "CURSOR_AUTH_MODE"

    // endregion

    /**
     * The persisted method, tolerant of legacy rows: an explicit [AUTH_MODE] wins,
     * otherwise a saved API key implies [CursorAuthMethod.CUSTOM].
     */
    fun inferMode(env: Map<String, String>): CursorAuthMethod {
        CursorAuthMethod.fromWire(env[AUTH_MODE]?.trim())?.let { return it }
        return if (env[API_KEY]?.trim().isNullOrEmpty()) {
            CursorAuthMethod.SUBSCRIPTION
        } else {
            CursorAuthMethod.CUSTOM
        }
    }

    /** The saved Run Everything knob, tolerant of hand-edited values. */
    fun isForceEnabled(env: Map<String, String>): Boolean =
        env[FORCE]?.trim()?.lowercase() in setOf("1", "true")

    /**
     * Whether the knob was ever written. A fresh agent (key absent) defaults to Run
     * Everything ON; an explicit `"0"` — the user chose "ask before running" — is
     * respected.
     */
    fun hasForceKnob(env: Map<String, String>): Boolean = env.containsKey(FORCE)

    /** [isForceEnabled] for a saved knob, else the fresh-agent default (ON). */
    fun forceOrDefault(env: Map<String, String>): Boolean =
        if (hasForceKnob(env)) isForceEnabled(env) else true

    /**
     * The copy-pasteable login command. codeg's managed `cursor-agent` lives in its
     * binary cache and is NOT on the user's PATH, so a bare `cursor-agent login`
     * fails — use the resolved absolute path, quoted when it contains whitespace.
     */
    fun loginCommand(binaryPath: String?): String {
        val path = binaryPath?.trim().orEmpty()
        if (path.isEmpty()) return "cursor-agent login"
        val program = if (path.any { it == ' ' || it == '\t' }) "\"$path\"" else path
        return "$program login"
    }

    /**
     * Bake the panel's credential/model/launch knobs into [envText], mirroring the
     * web `buildCursorEnv`. Unrelated keys are preserved ([EnvText.patch] merges)
     * and an empty value deletes its key:
     *
     * - [CursorAuthMethod.SUBSCRIPTION] — the API key is DELETED so a launch (and
     *   the probes) fall back to the Cursor account.
     * - [CursorAuthMethod.CUSTOM] — the key from the form is written.
     *
     * [API_BASE_URL] is always removed (dead weight from a legacy row), and the
     * method itself is always recorded.
     *
     * Run Everything OFF is written as an explicit `"0"`, NOT by deleting the key.
     * Deleting it (what the web/iOS panels do) makes the knob indistinguishable from
     * a fresh agent, which [forceOrDefault] reads as ON — so the panel would come
     * back claiming Run Everything is on and the next save would silently re-enable
     * auto-approval. `"0"` round-trips exactly and the launch path is unaffected: it
     * only ever adds `--force` for `"1"`/`"true"`.
     */
    fun applyEnv(
        envText: String,
        mode: CursorAuthMethod,
        apiKey: String,
        model: String,
        force: Boolean,
    ): String = EnvText.patch(
        envText,
        linkedMapOf(
            AUTH_MODE to mode.wire,
            API_BASE_URL to "",
            API_KEY to if (mode == CursorAuthMethod.CUSTOM) apiKey else "",
            MODEL to model,
            FORCE to if (force) "1" else "0",
        ),
    )
}

/**
 * Cursor's two real authentication methods. [CUSTOM] is a Cursor *account* API key
 * — NOT a third-party endpoint (the CLI has none). The wire token stays `"custom"`
 * for rows saved before the web's rename.
 */
enum class CursorAuthMethod(val wire: String) {
    SUBSCRIPTION("subscription"),
    CUSTOM("custom");

    companion object {
        fun fromWire(raw: String?): CursorAuthMethod? = entries.firstOrNull { it.wire == raw }
    }
}
