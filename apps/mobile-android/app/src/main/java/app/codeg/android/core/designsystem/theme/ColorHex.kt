package app.codeg.android.core.designsystem.theme

import androidx.compose.ui.graphics.Color

/**
 * Parse a `#RRGGBB` / `RRGGBB` (optionally `#RRGGBBAA`) hex string — the format
 * the codeg server uses for folder colors. Returns null on anything else.
 * Faithful port of the iOS `Color(hexString:)`.
 */
fun colorFromHex(hexString: String): Color? {
    var hex = hexString.trim()
    if (hex.startsWith("#")) hex = hex.substring(1)
    if (hex.length != 6 && hex.length != 8) return null
    val value = hex.toLongOrNull(16) ?: return null
    return if (hex.length == 8) {
        Color(
            red = ((value shr 24) and 0xFF) / 255f,
            green = ((value shr 16) and 0xFF) / 255f,
            blue = ((value shr 8) and 0xFF) / 255f,
            alpha = (value and 0xFF) / 255f,
        )
    } else {
        Color(
            red = ((value shr 16) and 0xFF) / 255f,
            green = ((value shr 8) and 0xFF) / 255f,
            blue = (value and 0xFF) / 255f,
            alpha = 1f,
        )
    }
}
