package app.codeg.android.core.network

import kotlinx.serialization.Serializable

/**
 * Typed failure surface for the HTTP client, mirroring the iOS `APIError`.
 * Thrown by [CodegClient]; the UI layer maps these to user-facing messages.
 */
sealed class ApiError(message: String) : Exception(message) {
    /** A network/transport failure (no HTTP response, DNS, TLS, timeout). */
    data class Transport(val detail: String) : ApiError(detail)

    /** HTTP 401 — the token is missing or rejected. */
    data object Unauthorized : ApiError("Invalid or missing token")

    /** HTTP 409 / `turn_in_progress` — a turn is already running on the connection. */
    data object TurnInProgress : ApiError("A turn is already in progress")

    /** Any other non-2xx, with the server's `{code,message}` envelope if present. */
    data class Server(
        val status: Int,
        val code: String?,
        val serverMessage: String,
    ) : ApiError(serverMessage)

    /** The response body could not be decoded into the expected shape. */
    data class Decoding(val detail: String) : ApiError(detail)
}

/** Error envelope returned by the server on non-2xx (`{code, message, ...}`). */
@Serializable
data class ServerError(
    val code: String? = null,
    val message: String? = null,
)

/**
 * A reused/looked-up ACP connection the server no longer knows about — the caller
 * should drop the cached id and reconnect. Mirrors iOS `APIError.isStaleConnection`
 * (HTTP 404, or the `connection_not_found` / `unknown_connection` server codes).
 */
val ApiError.isStaleConnection: Boolean
    get() = this is ApiError.Server &&
        (status == 404 || code == "connection_not_found" || code == "unknown_connection")

/**
 * A **git remote** authentication failure (push/pull/fetch), signalling the
 * credential-retry flow to prompt for a token / username+password and retry.
 * Mirrors iOS `APIError.isAuthFailure`: the `authentication_failed` server code, or
 * a message matching git's known auth phrases. NOT [ApiError.Unauthorized] — that
 * is the codeg server token, never a git remote.
 */
val ApiError.isAuthFailure: Boolean
    get() {
        if (this !is ApiError.Server) return false
        if (code == "authentication_failed") return true
        val lower = serverMessage.lowercase()
        return lower.contains("authentication failed") ||
            lower.contains("could not read username") ||
            lower.contains("could not read password") ||
            lower.contains("logon failed")
    }
