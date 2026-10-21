package app.codeg.android.core.security

/**
 * Stores per-server secrets (auth tokens) at rest. The Android analogue of the
 * iOS Keychain-backed token store. Keyed by an opaque id (the server profile id).
 */
interface SecretStore {
    fun put(key: String, value: String)
    fun get(key: String): String?
    fun remove(key: String)
}
