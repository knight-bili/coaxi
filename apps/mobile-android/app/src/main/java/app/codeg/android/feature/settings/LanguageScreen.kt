package app.codeg.android.feature.settings

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.PhoneAndroid
import androidx.compose.material.icons.rounded.Translate
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.RadioButton
import androidx.compose.material3.RadioButtonDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.codeg.android.R
import app.codeg.android.core.common.AppLanguage
import app.codeg.android.core.common.LocaleManager
import app.codeg.android.core.designsystem.theme.CodegTheme

/**
 * App display language (System / English / 中文). Purely device-local; persisted
 * by [LocaleManager] and applied by recreating the activity (which re-runs
 * `attachBaseContext`). Mirrors iOS `LanguageSettingsView`.
 */
@Composable
fun LanguageContent() {
    val context = LocalContext.current
    val colors = CodegTheme.colors
    var selected by remember { mutableStateOf(LocaleManager.current(context)) }

    Column(
        Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(
            stringResource(R.string.language_interface_description),
            fontSize = 13.sp, color = colors.textTertiary, modifier = Modifier.padding(bottom = 8.dp, start = 4.dp),
        )
        Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(colors.bgElevated.copy(alpha = 0.5f))) {
            AppLanguage.entries.forEachIndexed { index, lang ->
                val isSelected = lang == selected
                ListItem(
                    headlineContent = { Text(lang.title) },
                    leadingContent = {
                        Icon(
                            if (lang == AppLanguage.SYSTEM) Icons.Rounded.PhoneAndroid else Icons.Rounded.Translate,
                            contentDescription = null, tint = colors.textSecondary,
                        )
                    },
                    trailingContent = {
                        RadioButton(
                            selected = isSelected,
                            onClick = null,
                            colors = RadioButtonDefaults.colors(selectedColor = colors.accent, unselectedColor = colors.textTertiary),
                        )
                    },
                    colors = ListItemDefaults.colors(containerColor = Color.Transparent, headlineColor = colors.textPrimary),
                    modifier = Modifier.clickable {
                        if (lang != selected) {
                            LocaleManager.set(context, lang)
                            selected = lang
                            context.findActivity()?.recreate()
                        }
                    },
                )
                if (index < AppLanguage.entries.lastIndex) {
                    HorizontalDivider(color = colors.hairline, modifier = Modifier.padding(start = 52.dp))
                }
            }
        }
    }
}

/** Walk the ContextWrapper chain to find the host Activity (for `recreate()`). */
private tailrec fun Context.findActivity(): Activity? = when (this) {
    is Activity -> this
    is ContextWrapper -> baseContext.findActivity()
    else -> null
}
