package app.codeg.android.core.datastore

import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import app.codeg.android.core.model.AgentType
import kotlinx.coroutines.flow.first
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.MapSerializer
import kotlinx.serialization.builtins.serializer
import kotlinx.serialization.json.Json
import javax.inject.Inject
import javax.inject.Singleton

/** One agent's remembered "selector" choices: its last-used mode + per-option config
 *  values. Mirrors the codeg web/iOS `SelectorPrefs`. */
@Serializable
data class SelectorPrefs(
    val modeId: String? = null,
    val configValues: Map<String, String>? = null,
)

/**
 * Local cache of agent mode/config selections, keyed by [AgentType.wire] — a port of
 * the web client's `codeg:selector-prefs` (and iOS `SelectorPrefsStore`).
 *
 * Why: the agent-options sheet's mode (plan/default/…) and config options (e.g. model)
 * otherwise reset to the server's fresh-session defaults every time. We persist the
 * user's pick per agent and re-apply it: passed to `acp_connect` as
 * `preferredModeId`/`preferredConfigValues` (the server applies them before reporting
 * state) and used to pre-select the draft options sheet.
 *
 * Keyed purely by agent (not per-server) like the web — modes/config are intrinsic to
 * the agent; a value the current server doesn't know is simply ignored on connect.
 */
@Singleton
class SelectorPrefsStore @Inject constructor(
    private val dataStore: DataStore<Preferences>,
) {
    private val json = Json { ignoreUnknownKeys = true }

    /** The remembered choices for [agent] (empty if none yet). */
    suspend fun prefs(agent: AgentType): SelectorPrefs = all()[agent.wire] ?: SelectorPrefs()

    /** Remember the agent's mode selection. */
    suspend fun saveMode(agent: AgentType, modeId: String) =
        update(agent) { it.copy(modeId = modeId) }

    /** Remember one config option's value, preserving the agent's other values. */
    suspend fun saveConfig(agent: AgentType, configId: String, valueId: String) =
        update(agent) { it.copy(configValues = (it.configValues ?: emptyMap()) + (configId to valueId)) }

    private suspend fun all(): Map<String, SelectorPrefs> = decode(dataStore.data.first()[KEY])

    private suspend fun update(agent: AgentType, mutate: (SelectorPrefs) -> SelectorPrefs) {
        dataStore.edit { prefs ->
            val map = decode(prefs[KEY]).toMutableMap()
            map[agent.wire] = mutate(map[agent.wire] ?: SelectorPrefs())
            prefs[KEY] = json.encodeToString(SERIALIZER, map)
        }
    }

    private fun decode(raw: String?): Map<String, SelectorPrefs> =
        raw?.let { runCatching { json.decodeFromString(SERIALIZER, it) }.getOrNull() } ?: emptyMap()

    private companion object {
        val KEY = stringPreferencesKey("selector_prefs")
        val SERIALIZER = MapSerializer(String.serializer(), SelectorPrefs.serializer())
    }
}
