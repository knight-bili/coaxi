package app.coaxi.android

import android.content.Context
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import app.coaxi.android.app.CoaxiApp
import app.coaxi.android.core.common.DeepLinkBus
import app.coaxi.android.core.common.CoaxiDeepLinkBus
import app.coaxi.android.core.common.LocaleManager
import dagger.hilt.android.AndroidEntryPoint

/**
 * Single-activity host. The whole UI is Compose; this activity enables
 * edge-to-edge and hosts the [CoaxiApp] root. [attachBaseContext] applies the
 * user-chosen app language (see [LocaleManager]); changing it calls `recreate()`.
 * Incoming `coaxi://` deep links are dispatched to [DeepLinkBus].
 */
@AndroidEntryPoint
class MainActivity : ComponentActivity() {
    override fun attachBaseContext(newBase: Context) {
        super.attachBaseContext(LocaleManager.wrap(newBase))
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        setContent { CoaxiApp() }
        DeepLinkBus.dispatch(intent?.data)
        CoaxiDeepLinkBus.dispatch(intent?.data)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        DeepLinkBus.dispatch(intent.data)
        CoaxiDeepLinkBus.dispatch(intent.data)
    }
}
