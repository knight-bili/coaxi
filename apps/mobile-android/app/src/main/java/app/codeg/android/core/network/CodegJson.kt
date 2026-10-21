package app.codeg.android.core.network

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonNamingStrategy

/**
 * Shared JSON coders for codeg's split-casing convention, mirroring the iOS
 * `CodegJSON`:
 *
 * - **[request]** — requests are camelCase. No naming strategy (Kotlin property
 *   names are already camelCase). `encodeDefaults = true` so non-null defaults
 *   like `declined = false` are sent; `explicitNulls = false` omits null fields
 *   (matching Swift `encodeIfPresent`).
 * - **[response]** — responses are snake_case → camelCase via
 *   [JsonNamingStrategy.SnakeCase]. Lenient + `ignoreUnknownKeys` so a newer
 *   server with extra fields never breaks a decode; `coerceInputValues` maps a
 *   stray `null` onto a non-null default.
 *
 * The hand-written decoders (`AcpEvent.fromWire`, `ContentBlock`, snapshots)
 * read raw snake_case keys directly off the parsed tree — the naming strategy
 * only affects `@Serializable` property matching, not `decodeJsonElement`.
 */
object CodegJson {
    val request: Json = Json {
        encodeDefaults = true
        explicitNulls = false
        // Default discriminator is already "type"; stated for the PromptInputBlock
        // sealed hierarchy which the server tags with `type`.
        classDiscriminator = "type"
    }

    @OptIn(ExperimentalSerializationApi::class)
    val response: Json = Json {
        namingStrategy = JsonNamingStrategy.SnakeCase
        ignoreUnknownKeys = true
        isLenient = true
        explicitNulls = false
        coerceInputValues = true
    }
}
