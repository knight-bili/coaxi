package app.codeg.android.core.designsystem.theme

import androidx.compose.material3.Typography
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

/**
 * Type scale. Body/UI text uses the platform default (Roboto); code uses
 * [FontFamily.Monospace]. Sizes are tuned to read like the iOS app (17pt body,
 * 13pt code). A bundled developer mono (e.g. JetBrains Mono) can be dropped in
 * later by swapping [CodegMono].
 */
val CodegMono: FontFamily = FontFamily.Monospace

/** Shared style for inline code / commands / monospaced values (iOS `Font.mono(13)`). */
val CodeTextStyle = TextStyle(
    fontFamily = CodegMono,
    fontSize = 13.sp,
    lineHeight = 18.sp,
)

/**
 * Type scale aligned to the Material 3 reference scale (16sp body, 14sp label),
 * so text reads at native Android sizes rather than the iOS 17pt body. Titles
 * keep a SemiBold weight for brand emphasis; labels use Material line-heights.
 */
val CodegTypography = Typography(
    titleLarge = TextStyle(fontWeight = FontWeight.SemiBold, fontSize = 22.sp, lineHeight = 28.sp),
    titleMedium = TextStyle(fontWeight = FontWeight.SemiBold, fontSize = 16.sp, lineHeight = 24.sp),
    titleSmall = TextStyle(fontWeight = FontWeight.SemiBold, fontSize = 14.sp, lineHeight = 20.sp),
    bodyLarge = TextStyle(fontWeight = FontWeight.Normal, fontSize = 16.sp, lineHeight = 24.sp),
    bodyMedium = TextStyle(fontWeight = FontWeight.Normal, fontSize = 14.sp, lineHeight = 20.sp),
    bodySmall = TextStyle(fontWeight = FontWeight.Normal, fontSize = 12.sp, lineHeight = 16.sp),
    labelLarge = TextStyle(fontWeight = FontWeight.Medium, fontSize = 14.sp, lineHeight = 20.sp),
    labelMedium = TextStyle(fontWeight = FontWeight.Medium, fontSize = 12.sp, lineHeight = 16.sp),
    labelSmall = TextStyle(fontWeight = FontWeight.Medium, fontSize = 11.sp, lineHeight = 16.sp),
)
