package app.codeg.android.core.designsystem.theme

import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance

/**
 * User-selectable accent palettes, ported from the iOS `AccentPalette`. Each has
 * a dark-mode and light-mode fill. The default is [ORANGE], matching iOS.
 */
enum class AccentPalette(val id: String, val dark: Color, val light: Color) {
    MINT("mint", Color(0xFF66E0B3), Color(0xFF0F946B)),
    BLUE("blue", Color(0xFF63A8FF), Color(0xFF0073EB)),
    INDIGO("indigo", Color(0xFF8F99FC), Color(0xFF4A4FDB)),
    PURPLE("purple", Color(0xFFC28CFF), Color(0xFF8542D4)),
    PINK("pink", Color(0xFFFF73B5), Color(0xFFDB297D)),
    ORANGE("orange", Color(0xFFFF9E4D), Color(0xFFD96B0D)),
    TEAL("teal", Color(0xFF4DD1DB), Color(0xFF008594)),
    RED("red", Color(0xFFFF7373), Color(0xFFD13033));

    fun resolve(isDark: Boolean): Color = if (isDark) dark else light

    companion object {
        val DEFAULT = ORANGE
        val all: List<AccentPalette> = entries
        fun from(id: String?): AccentPalette = entries.firstOrNull { it.id == id } ?: DEFAULT
    }
}

/**
 * The full set of semantic colours used across the app, ported from the iOS
 * `Theme`. Provided via `LocalCodegColors`; resolved for the active mode +
 * accent by [codegColors].
 */
@Immutable
data class CodegColors(
    val isDark: Boolean,
    val bg: Color,
    val bgElevated: Color,
    val surfaceStroke: Color,
    val hairline: Color,
    val codeSurface: Color,
    val rail: Color,
    val textPrimary: Color,
    val textSecondary: Color,
    val textTertiary: Color,
    val danger: Color,
    val accent: Color,
    val onAccent: Color,
    val accentDim: Color,
)

/** Build the colour set for the given mode + accent (iOS `Theme.swift` values). */
fun codegColors(isDark: Boolean, accent: AccentPalette): CodegColors {
    val accentColor = accent.resolve(isDark)
    return if (isDark) {
        CodegColors(
            isDark = true,
            bg = Color(0xFF0A0B0D),
            bgElevated = Color(0xFF141619),
            surfaceStroke = Color.White.copy(alpha = 0.09f),
            hairline = Color.White.copy(alpha = 0.055f),
            codeSurface = Color.Black.copy(alpha = 0.30f),
            rail = Color.White.copy(alpha = 0.16f),
            textPrimary = Color(0xFFF7F7F7),
            textSecondary = Color(0xFFA3A3A3),
            textTertiary = Color(0xFF707070),
            danger = Color(0xFFF57575),
            accent = accentColor,
            onAccent = onAccentColor(accentColor),
            accentDim = accentColor.copy(alpha = 0.16f),
        )
    } else {
        CodegColors(
            isDark = false,
            bg = Color(0xFFF2F4F6),
            bgElevated = Color(0xFFFFFFFF),
            surfaceStroke = Color.Black.copy(alpha = 0.10f),
            hairline = Color.Black.copy(alpha = 0.07f),
            codeSurface = Color.Black.copy(alpha = 0.045f),
            rail = Color.Black.copy(alpha = 0.20f),
            textPrimary = Color(0xFF1C1C1C),
            textSecondary = Color(0xFF616161),
            textTertiary = Color(0xFF8C8C8C),
            danger = Color(0xFFCC2E2E),
            accent = accentColor,
            onAccent = onAccentColor(accentColor),
            accentDim = accentColor.copy(alpha = 0.16f),
        )
    }
}

/** Black or white text/icon on a filled accent, chosen by luminance (iOS `onColor`). */
private fun onAccentColor(accent: Color): Color =
    if (accent.luminance() > 0.6f) Color(0xFF0F0F0F) else Color.White

/** The cool indigo glow used in the app background (iOS `CodegBackground`). */
val CodegGlowCool = Color(0xFF4D6BF2)
