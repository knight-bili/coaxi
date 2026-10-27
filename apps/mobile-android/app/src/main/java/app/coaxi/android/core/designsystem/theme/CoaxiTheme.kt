package app.coaxi.android.core.designsystem.theme

import android.app.Activity
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat
import app.coaxi.android.core.datastore.ThemeMode

val LocalCoaxiColors = staticCompositionLocalOf<CoaxiColors> {
    error("CoaxiColors not provided — wrap content in CoaxiTheme { }")
}
val LocalCoaxiDimens = staticCompositionLocalOf { CoaxiDimens() }

/**
 * Root theme. Resolves the active appearance (light/dark/system) + accent into a
 * [CoaxiColors] set (exposed via `LocalCoaxiColors` / the [CoaxiTheme] accessor)
 * and a Material3 [MaterialTheme] so stock M3 components inherit the same palette.
 */
@Composable
fun CoaxiTheme(
    themeMode: ThemeMode = ThemeMode.SYSTEM,
    accentId: String = AccentPalette.DEFAULT.id,
    content: @Composable () -> Unit,
) {
    val isDark = when (themeMode) {
        ThemeMode.DARK -> true
        ThemeMode.LIGHT -> false
        ThemeMode.SYSTEM -> isSystemInDarkTheme()
    }
    val palette = AccentPalette.from(accentId)
    val colors = remember(isDark, palette) { coaxiColors(isDark, palette) }
    val material = remember(colors) { colors.toMaterialScheme() }

    // Drive the system bar icon contrast from the *resolved* appearance rather than the
    // system's dark-mode flag: a user who forces Light while the OS is Dark (or vice
    // versa) must still get legible status/navigation icons over our backdrop.
    val view = LocalView.current
    if (!view.isInEditMode) {
        SideEffect {
            val window = (view.context as? Activity)?.window ?: return@SideEffect
            WindowCompat.getInsetsController(window, view).apply {
                isAppearanceLightStatusBars = !isDark
                isAppearanceLightNavigationBars = !isDark
            }
        }
    }

    CompositionLocalProvider(
        LocalCoaxiColors provides colors,
        LocalCoaxiDimens provides CoaxiDimens(),
    ) {
        MaterialTheme(
            colorScheme = material,
            typography = CoaxiTypography,
            shapes = CoaxiShapes,
            content = content,
        )
    }
}

/** Accessor for design-system tokens, used like `CoaxiTheme.colors.accent`. */
object CoaxiTheme {
    val colors: CoaxiColors
        @Composable @ReadOnlyComposable get() = LocalCoaxiColors.current
    val dimens: CoaxiDimens
        @Composable @ReadOnlyComposable get() = LocalCoaxiDimens.current
}

private fun CoaxiColors.toMaterialScheme() =
    if (isDark) {
        darkColorScheme(
            primary = accent,
            onPrimary = onAccent,
            primaryContainer = accentDim,
            onPrimaryContainer = textPrimary,
            background = bg,
            onBackground = textPrimary,
            surface = bgElevated,
            onSurface = textPrimary,
            surfaceVariant = codeSurface,
            onSurfaceVariant = textSecondary,
            outline = textTertiary,
            outlineVariant = surfaceStroke,
            error = danger,
            onError = Color.White,
        )
    } else {
        lightColorScheme(
            primary = accent,
            onPrimary = onAccent,
            primaryContainer = accentDim,
            onPrimaryContainer = textPrimary,
            background = bg,
            onBackground = textPrimary,
            surface = bgElevated,
            onSurface = textPrimary,
            surfaceVariant = codeSurface,
            onSurfaceVariant = textSecondary,
            outline = textTertiary,
            outlineVariant = surfaceStroke,
            error = danger,
            onError = Color.White,
        )
    }
