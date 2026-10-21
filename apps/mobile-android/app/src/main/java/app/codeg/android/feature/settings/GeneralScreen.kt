package app.codeg.android.feature.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Remove
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.codeg.android.R
import app.codeg.android.core.designsystem.theme.CodegTheme

@Composable
fun GeneralContent(viewModel: GeneralViewModel = hiltViewModel()) {
    val ui by viewModel.ui.collectAsStateWithLifecycle()
    val colors = CodegTheme.colors

    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp), verticalArrangement = Arrangement.spacedBy(20.dp)) {
        Section(stringResource(R.string.general_conversation_tools)) {
            ToggleRow(stringResource(R.string.general_live_feedback_title), stringResource(R.string.general_live_feedback_subtitle), ui.feedbackEnabled, viewModel::setFeedback)
            ToggleRow(stringResource(R.string.general_ask_questions_title), stringResource(R.string.general_ask_questions_subtitle), ui.questionEnabled, viewModel::setQuestion)
        }
        Section(stringResource(R.string.general_multi_agent_delegation)) {
            ToggleRow(stringResource(R.string.general_enable_delegation_title), stringResource(R.string.general_enable_delegation_subtitle), ui.delegationEnabled, viewModel::setDelegationEnabled)
            if (ui.delegationEnabled) {
                ListItem(
                    headlineContent = { Text(stringResource(R.string.general_max_depth_title)) },
                    supportingContent = { Text(stringResource(R.string.general_max_depth_subtitle)) },
                    trailingContent = {
                        Stepper(ui.depthLimit, onDec = { viewModel.setDepth(ui.depthLimit - 1) }, onInc = { viewModel.setDepth(ui.depthLimit + 1) })
                    },
                    colors = ListItemDefaults.colors(
                        containerColor = Color.Transparent,
                        headlineColor = colors.textPrimary,
                        supportingColor = colors.textTertiary,
                    ),
                )
            }
        }
    }
}

@Composable
private fun Section(title: String, content: @Composable () -> Unit) {
    val colors = CodegTheme.colors
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Text(title, style = MaterialTheme.typography.labelLarge, color = colors.textTertiary, modifier = Modifier.padding(start = 12.dp, bottom = 4.dp))
        Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(colors.bgElevated.copy(alpha = 0.5f))) { content() }
    }
}

@Composable
private fun ToggleRow(title: String, subtitle: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    val colors = CodegTheme.colors
    ListItem(
        headlineContent = { Text(title) },
        supportingContent = { Text(subtitle) },
        trailingContent = {
            Switch(
                checked = checked,
                onCheckedChange = onChange,
                colors = SwitchDefaults.colors(checkedTrackColor = colors.accent, checkedThumbColor = colors.onAccent),
            )
        },
        colors = ListItemDefaults.colors(
            containerColor = Color.Transparent,
            headlineColor = colors.textPrimary,
            supportingColor = colors.textTertiary,
        ),
        modifier = Modifier.clickable { onChange(!checked) },
    )
}

@Composable
private fun Stepper(value: Int, onDec: () -> Unit, onInc: () -> Unit) {
    val colors = CodegTheme.colors
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        StepButton(Icons.Rounded.Remove, onDec)
        Text("$value", fontSize = 16.sp, fontWeight = FontWeight.SemiBold, color = colors.textPrimary, modifier = Modifier.padding(horizontal = 8.dp))
        StepButton(Icons.Rounded.Add, onInc)
    }
}

@Composable
private fun StepButton(icon: androidx.compose.ui.graphics.vector.ImageVector, onClick: () -> Unit) {
    val colors = CodegTheme.colors
    FilledTonalIconButton(
        onClick = onClick,
        modifier = Modifier.size(36.dp),
        colors = IconButtonDefaults.filledTonalIconButtonColors(
            containerColor = colors.textPrimary.copy(alpha = 0.06f),
            contentColor = colors.textPrimary,
        ),
    ) {
        Icon(icon, contentDescription = null, modifier = Modifier.size(18.dp))
    }
}
