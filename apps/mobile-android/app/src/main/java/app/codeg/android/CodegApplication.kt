package app.codeg.android

import android.app.Application
import dagger.hilt.android.HiltAndroidApp

/**
 * Application entry point. [@HiltAndroidApp][HiltAndroidApp] triggers Hilt's
 * code generation and creates the application-level dependency container.
 */
@HiltAndroidApp
class CodegApplication : Application()
