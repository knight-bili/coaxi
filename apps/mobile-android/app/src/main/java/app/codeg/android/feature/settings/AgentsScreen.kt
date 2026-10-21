package app.codeg.android.feature.settings

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material3.Icon
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.codeg.android.R
import app.codeg.android.core.designsystem.component.AgentAvatar
import app.codeg.android.core.designsystem.component.EmptyState
import app.codeg.android.core.designsystem.component.LoadingView
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.model.AcpAgentInfo
import app.codeg.android.core.model.AgentType

@Composable
fun AgentsContent(onOpen: (AgentType) -> Unit, viewModel: AgentsViewModel = hiltViewModel()) {
    val ui by viewModel.ui.collectAsStateWithLifecycle()

    Box(Modifier.fillMaxSize()) {
        when {
            ui.loading && ui.agents.isEmpty() -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingView(stringResource(R.string.common_loading)) }
            ui.agents.isEmpty() -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { EmptyState(Icons.Rounded.Person, stringResource(R.string.agents_empty_title), ui.error ?: stringResource(R.string.agents_empty_subtitle)) }
            else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 12.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                items(ui.agents, key = { it.agentType.wire }) { agent ->
                    AgentRow(agent, onToggle = { viewModel.setEnabled(agent, it) }, onOpen = { onOpen(agent.agentType) })
                }
            }
        }
    }
}

@Composable
private fun AgentRow(agent: AcpAgentInfo, onToggle: (Boolean) -> Unit, onOpen: () -> Unit) {
    val colors = CodegTheme.colors
    Row(
        Modifier.fillMaxWidth().clickable(onClick = onOpen).padding(horizontal = 6.dp, vertical = 8.dp).alpha(if (agent.available) 1f else 0.5f),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        AgentAvatar(agent.agentType, size = 38.dp)
        Column(Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(agent.name.ifEmpty { agent.agentType.displayName }, fontSize = 15.sp, color = colors.textPrimary, maxLines = 1, overflow = TextOverflow.Ellipsis)
                agent.installedVersion?.let { Text("v$it", fontSize = 11.sp, color = colors.textTertiary) }
            }
            if (agent.description.isNotEmpty()) Text(agent.description, fontSize = 12.sp, color = colors.textTertiary, maxLines = 2, overflow = TextOverflow.Ellipsis)
        }
        Switch(
            checked = agent.enabled,
            onCheckedChange = onToggle,
            enabled = agent.available,
            colors = SwitchDefaults.colors(checkedTrackColor = colors.accent, checkedThumbColor = colors.onAccent),
        )
        Icon(
            Icons.AutoMirrored.Rounded.KeyboardArrowRight,
            contentDescription = null,
            tint = colors.textTertiary,
            modifier = Modifier.size(18.dp),
        )
    }
}
