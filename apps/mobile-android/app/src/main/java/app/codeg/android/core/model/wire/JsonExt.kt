package app.codeg.android.core.model.wire

import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.longOrNull

/**
 * Small null-tolerant accessors over [JsonObject] used by the hand-written
 * decoders (events, content blocks, snapshots). These mirror the iOS pattern of
 * `decodeIfPresent(... ) ?? default` — a missing key, a `null`, or a
 * wrong-typed value all yield `null` rather than throwing, so one malformed
 * field never breaks the whole frame decode.
 *
 * NOTE: these operate on already-parsed JSON, so the snake_case wire keys are
 * intact (no naming-strategy conversion). Always pass the raw wire key
 * (e.g. `"tool_call_id"`).
 */

internal fun JsonObject.stringOrNull(key: String): String? {
    val p = this[key] as? JsonPrimitive ?: return null
    return if (p.isString) p.content else null
}

/** Non-empty trimmed string, or `null`. */
internal fun JsonObject.nonEmptyString(key: String): String? =
    stringOrNull(key)?.takeIf { it.isNotBlank() }

internal fun JsonObject.intOrNull(key: String): Int? =
    (this[key] as? JsonPrimitive)?.intOrNull

internal fun JsonObject.longOrNull(key: String): Long? =
    (this[key] as? JsonPrimitive)?.longOrNull

internal fun JsonObject.boolOrNull(key: String): Boolean? =
    (this[key] as? JsonPrimitive)?.booleanOrNull

internal fun JsonObject.objectOrNull(key: String): JsonObject? =
    this[key] as? JsonObject

internal fun JsonObject.arrayOrNull(key: String): JsonArray? =
    this[key] as? JsonArray

internal fun JsonElement.asObjectOrNull(): JsonObject? = this as? JsonObject
