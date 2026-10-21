package app.codeg.android.core.common

import android.content.Context
import android.content.res.Configuration
import java.util.Locale

/**
 * The app's display language. [SYSTEM] follows the device; the other cases
 * override it in-app via an attach-time configuration wrap (see [LocaleManager]).
 *
 * This is the **app UI** language and is purely device-local — unrelated to the
 * server-side reply language (`ChatLanguageCatalog`). Mirrors iOS `AppLanguage`.
 */
enum class AppLanguage(val storageKey: String, val title: String, val locale: Locale?) {
    SYSTEM("system", "System", null),
    ENGLISH("english", "English", Locale.ENGLISH),
    CHINESE("chinese", "中文", Locale.SIMPLIFIED_CHINESE);

    companion object {
        fun fromKey(key: String?): AppLanguage = entries.firstOrNull { it.storageKey == key } ?: SYSTEM
    }
}

/**
 * Reads/writes the chosen app language to a dedicated synchronous
 * `SharedPreferences` (so it can be read in `attachBaseContext`, before DataStore
 * is available) and wraps a base `Context` with the matching locale.
 */
object LocaleManager {
    private const val PREFS = "codeg.locale"
    private const val KEY = "app_language"

    fun current(context: Context): AppLanguage {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        return AppLanguage.fromKey(prefs.getString(KEY, null))
    }

    fun set(context: Context, language: AppLanguage) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit()
            .putString(KEY, language.storageKey).apply()
    }

    /** Apply the saved language override to [base], or return it unchanged for [AppLanguage.SYSTEM]. */
    fun wrap(base: Context): Context {
        val locale = current(base).locale ?: return base
        Locale.setDefault(locale)
        val config = Configuration(base.resources.configuration)
        config.setLocale(locale)
        return base.createConfigurationContext(config)
    }
}
