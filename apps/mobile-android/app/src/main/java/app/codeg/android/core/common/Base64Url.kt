package app.codeg.android.core.common

import java.util.Base64

/**
 * Base64URL **without padding**, matching the iOS client's `base64URLNoPad`
 * (standard base64 → `+`→`-`, `/`→`_`, strip `=`).
 *
 * Used to embed the auth token in the WebSocket subprotocol:
 * `codeg-token.<base64url-no-pad(token)>`. Implemented with `java.util.Base64`
 * (available since API 26, < our minSdk 31) so it also runs in plain-JVM unit
 * tests — `android.util.Base64` would not.
 */
object Base64Url {
    private val encoder = Base64.getUrlEncoder().withoutPadding()

    fun encode(value: String): String =
        encoder.encodeToString(value.toByteArray(Charsets.UTF_8))
}
