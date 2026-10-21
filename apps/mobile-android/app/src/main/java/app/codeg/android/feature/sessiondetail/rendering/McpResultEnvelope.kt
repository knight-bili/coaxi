package app.codeg.android.feature.sessiondetail.rendering

import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

/**
 * Peels the host envelopes that wrap an MCP `CallToolResult` on its way to a tool
 * card. Port of the web `lib/mcp-result-envelope.ts`.
 *
 * codex-acp forwards EVERY MCP tool call's outcome to the ACP wire as
 * `rawOutput = { result: <CallToolResult> | null, error: <string> | null }`, and
 * codex's own rollout tags the same result under a serde `{ Ok: … }` variant. Neither
 * layer is part of the result the tool actually returned, so a card reading that
 * result has to strip them first — otherwise the whole envelope falls through as
 * opaque text and the card renders raw JSON.
 *
 * The peel is deliberately narrow: a tool result is only ever a *child agent's*
 * arbitrary payload away from being mangled, so both the destination and the failure
 * case are positively identified rather than matched on key names alone. A payload
 * that merely happens to own a `result` or `error` key is left exactly as it was.
 */
object McpResultEnvelope {

    /** Keys a host uses to nest the actual `CallToolResult`. `result` is codex-acp's
     *  live-wire envelope; `Ok`/`ok` the serde-tagged `Result` variant codex writes
     *  into its rollout. */
    private val wrapperKeys = listOf("result", "Ok", "ok")

    /** One host layer plus a serde tag is the deepest shape seen. */
    private const val MAX_DEPTH = 3

    /**
     * @property obj The `CallToolResult` reached by peeling, or the input unchanged
     *   when no host envelope was positively identified.
     * @property hostError codex-acp's `rawOutput.error`, read ONLY from an envelope
     *   carrying a `result` key with nothing in it — i.e. the MCP call failed outright
     *   and that string is all there is to show.
     */
    data class Peeled(val obj: JsonObject, val hostError: String?)

    /**
     * Strip host `{ result, error }` / `{ Ok }` layers from [obj].
     *
     * [isResolvable] is the caller's own "I can already read this shape" predicate.
     * Peeling stops as soon as it holds, so a `CallToolResult` that itself owns a
     * `result` key is never unwrapped out from under the caller.
     */
    fun peel(obj: JsonObject, isResolvable: (JsonObject) -> Boolean): Peeled {
        var current = obj
        var depth = 0
        while (depth < MAX_DEPTH && !isResolvable(current)) {
            val next = wrapperKeys
                .firstNotNullOfOrNull { key -> (current[key] as? JsonObject)?.takeIf(::isCallToolResult) }
            // Nothing peelable left: this is either the payload itself or a host
            // envelope whose call failed before producing a result.
                ?: return Peeled(current, hostFailureError(current))
            current = next
            depth++
        }
        return Peeled(current, null)
    }

    /**
     * Whether [value] is an MCP `CallToolResult` — the only thing worth peeling TO.
     * Requiring this of the destination (not just the wrapper key's presence) is what
     * keeps a child's own `{result: {...}}` payload from being unwrapped and then
     * misread.
     */
    fun isCallToolResult(value: JsonObject): Boolean =
        value["content"] is JsonArray || value["structuredContent"] is JsonObject

    /**
     * The error string of a host FAILURE envelope — `{result: null, error: "…"}`.
     * codex-acp always emits both keys, so requiring a present-but-empty `result`
     * alongside the string is what separates a failed MCP call from a child payload
     * that merely has an `error` field of its own.
     */
    private fun hostFailureError(obj: JsonObject): String? {
        if (!obj.containsKey("result")) return null
        if (obj["result"].let { it != null && it != JsonNull }) return null
        val err = (obj["error"] as? JsonPrimitive)?.takeIf { it.isString }?.content
        return err?.takeIf { it.isNotBlank() }
    }
}
