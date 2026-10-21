package app.codeg.android.core.datastore

import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.serializer
import kotlinx.serialization.json.Json
import javax.inject.Inject
import javax.inject.Singleton

/** Persists the user's recent search terms (most-recent first, capped), mirroring iOS. */
@Singleton
class RecentSearchesStore @Inject constructor(
    private val dataStore: DataStore<Preferences>,
) {
    private val json = Json { ignoreUnknownKeys = true }

    val searches: Flow<List<String>> = dataStore.data.map { decode(it[KEY]) }

    suspend fun add(term: String) {
        val trimmed = term.trim()
        if (trimmed.isEmpty()) return
        dataStore.edit { prefs ->
            val list = decode(prefs[KEY]).toMutableList()
            list.removeAll { it.equals(trimmed, ignoreCase = true) }
            list.add(0, trimmed)
            prefs[KEY] = json.encodeToString(ListSerializer(String.serializer()), list.take(CAP))
        }
    }

    suspend fun clear() {
        dataStore.edit { it.remove(KEY) }
    }

    private fun decode(raw: String?): List<String> =
        raw?.let { runCatching { json.decodeFromString(ListSerializer(String.serializer()), it) }.getOrNull() } ?: emptyList()

    private companion object {
        val KEY = stringPreferencesKey("recent_searches")
        const val CAP = 10
    }
}
