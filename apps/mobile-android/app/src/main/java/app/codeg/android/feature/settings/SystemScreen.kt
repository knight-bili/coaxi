package app.codeg.android.feature.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.codeg.android.R
import app.codeg.android.core.designsystem.component.CodegTextField
import app.codeg.android.core.designsystem.theme.CodegTheme

@Composable
fun SystemContent(viewModel: SystemViewModel = hiltViewModel()) {
    val ui by viewModel.ui.collectAsStateWithLifecycle()
    val colors = CodegTheme.colors

    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp), verticalArrangement = Arrangement.spacedBy(20.dp)) {
        // Proxy.
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(stringResource(R.string.system_http_proxy), fontSize = 13.sp, fontWeight = FontWeight.SemiBold, color = colors.textSecondary)
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(stringResource(R.string.system_enable_proxy), fontSize = 15.sp, color = colors.textPrimary, modifier = Modifier.weight(1f))
                Switch(checked = ui.proxyEnabled, onCheckedChange = viewModel::setProxyEnabled, colors = SwitchDefaults.colors(checkedTrackColor = colors.accent, checkedThumbColor = colors.onAccent))
            }
            if (ui.proxyEnabled) {
                CodegTextField(ui.proxyUrl, viewModel::onProxyUrl, label = stringResource(R.string.system_proxy_url), placeholder = "http://host:port", mono = true)
            }
            PillButton(stringResource(R.string.common_save), loading = ui.savingProxy, onClick = viewModel::saveProxy)
        }
        // Update check.
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(stringResource(R.string.system_server_update), fontSize = 13.sp, fontWeight = FontWeight.SemiBold, color = colors.textSecondary)
            PillButton(stringResource(R.string.system_check_for_updates), loading = ui.checking, onClick = viewModel::checkUpdate)
            ui.updateResult?.let { r ->
                Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(colors.bgElevated.copy(alpha = 0.5f)).padding(14.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text(stringResource(R.string.system_current_version, r.currentVersion), fontSize = 13.sp, color = colors.textSecondary)
                    if (r.update != null) {
                        Text(stringResource(R.string.system_update_available, r.update.version), fontSize = 14.sp, fontWeight = FontWeight.SemiBold, color = colors.accent)
                        if (r.update.body.isNotEmpty()) Text(r.update.body, fontSize = 12.sp, color = colors.textTertiary)
                    } else {
                        Text(stringResource(R.string.system_up_to_date), fontSize = 13.sp, color = colors.textTertiary)
                    }
                }
            }
        }
        ui.error?.let { Text(it, fontSize = 13.sp, color = colors.danger) }
    }
}

@Composable
private fun PillButton(label: String, loading: Boolean, onClick: () -> Unit) {
    val colors = CodegTheme.colors
    Button(
        onClick = onClick,
        enabled = !loading,
        shape = RoundedCornerShape(10.dp),
        colors = ButtonDefaults.buttonColors(containerColor = colors.accent, contentColor = colors.onAccent),
    ) {
        if (loading) CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp, color = colors.onAccent)
        else Text(label, fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
    }
}
