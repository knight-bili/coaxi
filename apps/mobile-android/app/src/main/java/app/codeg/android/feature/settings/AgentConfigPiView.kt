package app.codeg.android.feature.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.codeg.android.R
import app.codeg.android.core.designsystem.component.CodegSegmented
import app.codeg.android.core.designsystem.component.CodegTextField
import app.codeg.android.core.designsystem.component.PrimaryButton
import app.codeg.android.core.designsystem.component.SecretField
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.model.AcpAgentInfo
import app.codeg.android.core.model.PI_CUSTOM_PROVIDER_SENTINEL
import app.codeg.android.core.model.PiCommandValidation
import app.codeg.android.core.model.PiConfig
import app.codeg.android.core.model.PiCustomProvider
import app.codeg.android.core.model.PiEnvKeys
import app.codeg.android.core.model.PiRuntimeMode
import app.codeg.android.core.model.UpdatePiConfigBody
import app.codeg.android.core.model.piBuiltinProviders
import app.codeg.android.core.model.piCustomApiProtocols
import app.codeg.android.core.model.piThinkingLevels
import app.codeg.android.core.network.displayMessage
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

private val SuccessGreen = Color(0xFF4CBC75)

private data class PiBanner(val text: String, val isError: Boolean)

/**
 * Self-contained Pi settings panel (port of iOS `PiConfigSection`). Pi has three
 * independent stores — runtime (which pi binary pi-acp spawns), credentials (pi's
 * native settings/auth/models json), and workspace trust — each with its own save,
 * so [AgentDetailContent] hides the host "Save" and this owns the three cards.
 *
 * [agent] is passed fresh each recomposition; the runtime/trust seeds come from its
 * env, the credentials from `acp_load_pi_config`.
 */
@Composable
fun AgentConfigPiView(agent: AcpAgentInfo, viewModel: AgentsViewModel) {
    val colors = CodegTheme.colors
    val scope = rememberCoroutineScope()

    val seedEnv = remember { agent.env }
    // Runtime
    val seedCmd = remember { seedEnv[PiEnvKeys.COMMAND] ?: "" }
    var mode by remember { mutableStateOf(if (seedCmd.trim().isEmpty()) PiRuntimeMode.DEFAULT else PiRuntimeMode.CUSTOM) }
    var command by remember { mutableStateOf(seedCmd) }
    var configDir by remember { mutableStateOf(seedEnv[PiEnvKeys.CONFIG_DIR] ?: "") }
    var sessionDir by remember { mutableStateOf(seedEnv[PiEnvKeys.SESSION_DIR] ?: "") }
    var validation by remember { mutableStateOf<PiCommandValidation?>(null) }
    var validating by remember { mutableStateOf(false) }
    var piStatus by remember { mutableStateOf<PiCommandValidation?>(null) }
    var checkingPi by remember { mutableStateOf(true) }
    var piOp by remember { mutableStateOf<String?>(null) } // "install" / "uninstall" / null
    var savingRuntime by remember { mutableStateOf(false) }

    // Credentials
    var selectedProvider by remember { mutableStateOf("") }
    var customId by remember { mutableStateOf("") }
    var customBaseUrl by remember { mutableStateOf("") }
    var customApi by remember { mutableStateOf(piCustomApiProtocols[0]) }
    var customProviders by remember { mutableStateOf<List<PiCustomProvider>>(emptyList()) }
    var modelText by remember { mutableStateOf("") }
    var thinkingLevel by remember { mutableStateOf("") }
    var apiKey by remember { mutableStateOf("") }
    var authProviders by remember { mutableStateOf<List<String>>(emptyList()) }
    var savingCreds by remember { mutableStateOf(false) }
    var loadingCreds by remember { mutableStateOf(true) }

    // Trust
    var trustWorkspace by remember { mutableStateOf((seedEnv[PiEnvKeys.TRUST_WORKSPACE] ?: "1") != "0") }
    var savingTrust by remember { mutableStateOf(false) }

    var banner by remember { mutableStateOf<PiBanner?>(null) }

    val isCustomProvider = selectedProvider == PI_CUSTOM_PROVIDER_SENTINEL
    val effectiveProvider = (if (isCustomProvider) customId else selectedProvider).trim()
    val providerHasKey = effectiveProvider.isNotEmpty() && authProviders.contains(effectiveProvider)
    val customIncomplete = mode == PiRuntimeMode.CUSTOM && command.trim().isEmpty()
    val credsIncomplete = effectiveProvider.isEmpty() || modelText.trim().isEmpty() ||
        (isCustomProvider && customBaseUrl.trim().isEmpty())
    // Built-ins, plus a loaded provider not in the curated list (never drop a pre-existing default).
    val providerOptions = if (
        selectedProvider.isNotEmpty() && selectedProvider != PI_CUSTOM_PROVIDER_SENTINEL &&
        piBuiltinProviders.none { it.first == selectedProvider }
    ) {
        piBuiltinProviders + (selectedProvider to selectedProvider)
    } else {
        piBuiltinProviders
    }

    // Strings captured for coroutine use.
    val runtimeSaved = stringResource(R.string.agents_pi_runtime_saved)
    val configSaved = stringResource(R.string.agents_pi_config_saved)
    val providerModelRequired = stringResource(R.string.agents_pi_provider_model_required)
    val baseUrlRequired = stringResource(R.string.agents_pi_base_url_required)
    val installedMsg = stringResource(R.string.agents_pi_installed_msg)
    val uninstalledMsg = stringResource(R.string.agents_pi_uninstalled_msg)
    val installFailed = stringResource(R.string.agents_pi_install_failed)
    val uninstallFailed = stringResource(R.string.agents_pi_uninstall_failed)
    val trustFailed = stringResource(R.string.agents_pi_trust_failed)

    LaunchedEffect(Unit) {
        loadingCreds = true
        try {
            val cfg = viewModel.loadPiConfig()
            modelText = cfg.defaultModel ?: ""
            thinkingLevel = cfg.defaultThinkingLevel ?: ""
            authProviders = cfg.authProviders
            customProviders = cfg.customProviders
            val dp = cfg.defaultProvider ?: ""
            val matched = cfg.customProviders.firstOrNull { it.id == dp }
            if (matched != null) {
                selectedProvider = PI_CUSTOM_PROVIDER_SENTINEL
                customId = matched.id
                customBaseUrl = matched.baseUrl
                customApi = matched.api.ifEmpty { piCustomApiProtocols[0] }
            } else {
                selectedProvider = dp
            }
        } catch (_: Exception) {
        }
        loadingCreds = false
    }
    LaunchedEffect(Unit) {
        checkingPi = true
        piStatus = try { viewModel.validatePiCommand("pi") } catch (_: Exception) { PiCommandValidation() }
        checkingPi = false
    }
    LaunchedEffect(banner) { if (banner != null) { delay(4000); banner = null } }

    fun detectPi() {
        scope.launch {
            checkingPi = true
            piStatus = try { viewModel.validatePiCommand("pi") } catch (_: Exception) { PiCommandValidation() }
            checkingPi = false
        }
    }

    fun handleValidate() {
        val cmd = command.trim()
        if (cmd.isEmpty()) return
        scope.launch {
            validating = true
            validation = null
            validation = try { viewModel.validatePiCommand(cmd) } catch (_: Exception) { PiCommandValidation() }
            validating = false
        }
    }

    fun handleSaveRuntime() {
        val env = PiConfig.buildRuntimeEnv(agent.env, mode, command, configDir, sessionDir)
        savingRuntime = true
        scope.launch {
            try {
                viewModel.updatePiEnv(agent, env)
                banner = PiBanner(runtimeSaved, false)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                banner = PiBanner(e.displayMessage(), true)
            }
            savingRuntime = false
        }
    }

    fun handleSaveCreds() {
        val trimmedModel = modelText.trim()
        if (effectiveProvider.isEmpty() || trimmedModel.isEmpty()) {
            banner = PiBanner(providerModelRequired, true); return
        }
        val trimmedBase = customBaseUrl.trim()
        if (isCustomProvider && trimmedBase.isEmpty()) {
            banner = PiBanner(baseUrlRequired, true); return
        }
        val key = apiKey.trim()
        val body = UpdatePiConfigBody(
            provider = effectiveProvider,
            model = trimmedModel,
            thinkingLevel = thinkingLevel.ifEmpty { null },
            apiKey = key.ifEmpty { null },
            customBaseUrl = if (isCustomProvider) trimmedBase else null,
            customApi = if (isCustomProvider) customApi else null,
        )
        savingCreds = true
        scope.launch {
            try {
                viewModel.savePiConfig(body)
                if (key.isNotEmpty()) {
                    apiKey = ""
                    if (!authProviders.contains(effectiveProvider)) authProviders = (authProviders + effectiveProvider).sorted()
                }
                if (isCustomProvider) {
                    customProviders = (customProviders.filterNot { it.id == effectiveProvider } +
                        PiCustomProvider(effectiveProvider, trimmedBase, customApi)).sortedBy { it.id }
                }
                banner = PiBanner(configSaved, false)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                banner = PiBanner(e.displayMessage(), true)
            }
            savingCreds = false
        }
    }

    fun handlePiOp(install: Boolean) {
        piOp = if (install) "install" else "uninstall"
        scope.launch {
            try {
                if (install) viewModel.installPiBinary() else viewModel.uninstallPiBinary()
                banner = PiBanner(if (install) installedMsg else uninstalledMsg, false)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                banner = PiBanner(if (install) installFailed else uninstallFailed, true)
            }
            piOp = null
            detectPi()
        }
    }

    fun toggleTrust(next: Boolean) {
        trustWorkspace = next
        val env = LinkedHashMap(agent.env)
        if (next) env.remove(PiEnvKeys.TRUST_WORKSPACE) else env[PiEnvKeys.TRUST_WORKSPACE] = "0"
        savingTrust = true
        scope.launch {
            try {
                viewModel.updatePiEnv(agent, env)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                trustWorkspace = !next
                banner = PiBanner(trustFailed, true)
            }
            savingTrust = false
        }
    }

    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
        // ---- Runtime card ----
        FormSection(stringResource(R.string.agents_pi_runtime_section), footer = stringResource(R.string.agents_pi_runtime_footer)) {
            CodegSegmented(
                options = listOf(stringResource(R.string.agents_pi_mode_default), stringResource(R.string.agents_pi_mode_custom)),
                selectedIndex = if (mode == PiRuntimeMode.DEFAULT) 0 else 1,
                onSelect = { mode = if (it == 0) PiRuntimeMode.DEFAULT else PiRuntimeMode.CUSTOM },
            )
            Caption(stringResource(if (mode == PiRuntimeMode.DEFAULT) R.string.agents_pi_mode_default_caption else R.string.agents_pi_mode_custom_caption))
            HorizontalDivider(color = colors.hairline)

            if (mode == PiRuntimeMode.DEFAULT) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        val status = piStatus
                        when {
                            checkingPi -> Text(stringResource(R.string.agents_pi_checking), fontSize = 14.sp, color = colors.textSecondary)
                            status?.found == true -> {
                                Text(
                                    status.version?.let { stringResource(R.string.agents_pi_installed_version, it) } ?: stringResource(R.string.agents_pi_installed),
                                    fontSize = 14.sp, color = colors.textPrimary,
                                )
                                status.resolvedPath?.takeIf { it.isNotEmpty() }?.let {
                                    Text(it, fontFamily = FontFamily.Monospace, fontSize = 11.sp, color = colors.textTertiary, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                }
                            }
                            else -> Text(stringResource(R.string.agents_pi_not_installed), fontSize = 14.sp, color = colors.textSecondary)
                        }
                    }
                    IconButton(onClick = { detectPi() }, enabled = !checkingPi && piOp == null) {
                        Icon(Icons.Rounded.Refresh, contentDescription = stringResource(R.string.agents_pi_recheck), tint = colors.textSecondary)
                    }
                    if (!checkingPi) {
                        if (piStatus?.found == true) {
                            PiActionButton(stringResource(R.string.agents_pi_uninstall), busy = piOp == "uninstall", enabled = piOp == null) { handlePiOp(install = false) }
                        } else {
                            PiActionButton(stringResource(R.string.agents_pi_install), busy = piOp == "install", enabled = piOp == null) { handlePiOp(install = true) }
                        }
                    }
                }
            } else {
                CodegTextField(command, { command = it; validation = null }, stringResource(R.string.agents_pi_command_label), placeholder = "/path/to/pi · pi · ./pi-test.sh", mono = true)
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    PiActionButton(stringResource(R.string.agents_pi_validate), busy = validating, enabled = !validating && command.trim().isNotEmpty()) { handleValidate() }
                    validation?.let { v ->
                        val tint = if (v.found) colors.accent else colors.danger
                        Icon(if (v.found) Icons.Rounded.CheckCircle else Icons.Rounded.ErrorOutline, contentDescription = null, tint = tint, modifier = Modifier.size(16.dp))
                        Text(
                            if (v.found) listOfNotNull(v.resolvedPath, v.version?.let { "($it)" }).joinToString(" ") else stringResource(R.string.agents_pi_command_not_found),
                            fontSize = 11.sp, color = if (v.found) colors.textSecondary else colors.danger, maxLines = 2, overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
                Caption(stringResource(R.string.agents_pi_command_caption))
                ExpandableGroup(stringResource(R.string.agents_pi_advanced)) {
                    CodegTextField(configDir, { configDir = it }, stringResource(R.string.agents_pi_config_dir), placeholder = "~/.pi/agent", mono = true)
                    if (configDir.trim().isNotEmpty()) {
                        Text(stringResource(R.string.agents_pi_config_dir_warning), fontSize = 11.sp, color = Color(0xFFF0A030))
                    }
                    CodegTextField(sessionDir, { sessionDir = it }, stringResource(R.string.agents_pi_session_dir), placeholder = "~/.pi/agent/sessions", mono = true)
                    Caption(stringResource(R.string.agents_pi_dirs_caption))
                }
            }

            HorizontalDivider(color = colors.hairline)
            if (customIncomplete) Caption(stringResource(R.string.agents_pi_runtime_incomplete))
            PrimaryButton(
                text = stringResource(R.string.agents_pi_save_runtime),
                onClick = { handleSaveRuntime() },
                enabled = !savingRuntime && !customIncomplete,
                loading = savingRuntime,
            )
        }

        // ---- Credentials card ----
        FormSection(stringResource(R.string.agents_pi_config_section), footer = stringResource(R.string.agents_pi_config_footer)) {
            SelectField(
                label = stringResource(R.string.agents_pi_provider),
                value = selectedProvider,
                options = listOf(PI_CUSTOM_PROVIDER_SENTINEL to stringResource(R.string.agents_pi_provider_custom)) +
                    providerOptions.map { it.first to it.second },
                onSelect = { value ->
                    selectedProvider = value
                    if (value == PI_CUSTOM_PROVIDER_SENTINEL && customId.trim().isEmpty()) {
                        customProviders.firstOrNull()?.let {
                            customId = it.id; customBaseUrl = it.baseUrl; customApi = it.api.ifEmpty { piCustomApiProtocols[0] }
                        }
                    }
                },
            )
            if (isCustomProvider) {
                CodegTextField(customId, { customId = it }, stringResource(R.string.agents_pi_provider_id), placeholder = "my-provider", mono = true)
                SelectField(
                    label = stringResource(R.string.agents_pi_api_protocol),
                    value = customApi,
                    options = piCustomApiProtocols.map { it to it },
                    onSelect = { customApi = it },
                )
                CodegTextField(customBaseUrl, { customBaseUrl = it }, stringResource(R.string.agents_pi_api_endpoint), placeholder = "https://api.example.com/v1", mono = true)
                Caption(stringResource(R.string.agents_pi_custom_caption))
            }
            CodegTextField(modelText, { modelText = it }, stringResource(R.string.agents_pi_model), placeholder = "claude-sonnet-4-20250514", mono = true)
            SelectField(
                label = stringResource(R.string.agents_pi_thinking),
                value = thinkingLevel.ifEmpty { "off" },
                options = piThinkingLevels.map { it to piThinkingLabel(it) },
                onSelect = { thinkingLevel = it },
            )
            SecretField(apiKey, { apiKey = it }, stringResource(R.string.agents_field_api_key))
            Caption(stringResource(if (providerHasKey) R.string.agents_pi_api_key_saved_caption else R.string.agents_pi_api_key_caption))
            PrimaryButton(
                text = stringResource(R.string.agents_pi_save_config),
                onClick = { handleSaveCreds() },
                enabled = !savingCreds && !loadingCreds && !credsIncomplete,
                loading = savingCreds,
            )
        }

        // ---- Trust card ----
        FormSection(stringResource(R.string.agents_pi_trust_section)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(stringResource(R.string.agents_pi_trust_title), fontSize = 14.sp, color = colors.textPrimary)
                    Text(stringResource(R.string.agents_pi_trust_desc), fontSize = 11.sp, color = colors.textTertiary)
                }
                Switch(
                    checked = trustWorkspace,
                    onCheckedChange = { toggleTrust(it) },
                    enabled = !savingTrust,
                    colors = SwitchDefaults.colors(checkedTrackColor = colors.accent, checkedThumbColor = colors.onAccent),
                )
            }
            Caption(stringResource(R.string.agents_pi_trust_caption))
        }

        banner?.let { PiResultBanner(it) }
    }
}

@Composable
private fun PiActionButton(text: String, busy: Boolean, enabled: Boolean, onClick: () -> Unit) {
    val colors = CodegTheme.colors
    OutlinedButton(onClick = onClick, enabled = enabled && !busy) {
        if (busy) CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp, color = colors.accent)
        else Text(text, color = colors.accent)
    }
}

@Composable
private fun PiResultBanner(banner: PiBanner) {
    val colors = CodegTheme.colors
    val tint = if (banner.isError) colors.danger else SuccessGreen
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(tint.copy(alpha = 0.12f)).padding(12.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Icon(if (banner.isError) Icons.Rounded.ErrorOutline else Icons.Rounded.CheckCircle, contentDescription = null, tint = tint, modifier = Modifier.size(16.dp))
        Text(banner.text, fontSize = 12.sp, color = if (banner.isError) colors.danger else colors.textSecondary)
    }
}

@Composable
private fun piThinkingLabel(level: String): String = stringResource(
    when (level) {
        "off" -> R.string.agents_pi_thinking_off
        "minimal" -> R.string.agents_pi_thinking_minimal
        "low" -> R.string.agents_pi_thinking_low
        "medium" -> R.string.agents_pi_thinking_medium
        "high" -> R.string.agents_pi_thinking_high
        "xhigh" -> R.string.agents_pi_thinking_xhigh
        else -> R.string.agents_pi_thinking_off
    },
)
