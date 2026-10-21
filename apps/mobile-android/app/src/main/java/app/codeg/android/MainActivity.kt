package app.codeg.android

import android.content.Context
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import app.codeg.android.app.CodegApp
import app.codeg.android.core.common.DeepLinkBus
import app.codeg.android.core.common.LocaleManager
import dagger.hilt.android.AndroidEntryPoint

/**
 * Single-activity host. The whole UI is Compose; this activity enables
 * edge-to-edge and hosts the [CodegApp] root. [attachBaseContext] applies the
 * user-chosen app language (see [LocaleManager]); changing it calls `recreate()`.
 * Incoming `codeg://` deep links are dispatched to [DeepLinkBus].
 */
@AndroidEntryPoint
class MainActivity : ComponentActivity() {
    override fun attachBaseContext(newBase: Context) {
        super.attachBaseContext(LocaleManager.wrap(newBase))
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        setContent { CodegApp() }
        DeepLinkBus.dispatch(intent?.data)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        DeepLinkBus.dispatch(intent.data)
    }
}
