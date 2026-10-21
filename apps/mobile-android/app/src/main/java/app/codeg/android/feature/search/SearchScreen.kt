package app.codeg.android.feature.search

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material.icons.rounded.History
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.codeg.android.R
import app.codeg.android.core.designsystem.component.EmptyState
import app.codeg.android.core.designsystem.component.InlineError
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.feature.sessions.SessionRow

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SearchScreen(
    onOpenConversation: (Int) -> Unit,
    viewModel: SearchViewModel = hiltViewModel(),
) {
    val ui by viewModel.ui.collectAsStateWithLifecycle()
    val query by viewModel.query.collectAsStateWithLifecycle()
    val recent by viewModel.recent.collectAsStateWithLifecycle()
    val colors = CodegTheme.colors

    Scaffold(
        containerColor = Color.Transparent,
        contentWindowInsets = WindowInsets(0, 0, 0, 0),
        // Like every other tab, a transparent TopAppBar — it carries the title and,
        // crucially, applies the status-bar inset so the search field clears the
        // system bar / camera cutout under edge-to-edge.
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.tab_search)) },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = Color.Transparent,
                    titleContentColor = colors.textPrimary,
                ),
            )
        },
    ) { padding ->
        Column(Modifier.padding(padding).fillMaxSize()) {
            // Search field — a native Material text field with an IME "Search"
            // action, a clear button, and an inline progress spinner.
            TextField(
                value = query,
                onValueChange = viewModel::onQueryChange,
                modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp),
                placeholder = { Text(stringResource(R.string.search_placeholder)) },
                leadingIcon = { Icon(Icons.Rounded.Search, contentDescription = null) },
                trailingIcon = {
                    when {
                        ui.searching -> CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp, color = colors.accent)
                        query.isNotEmpty() -> IconButton(onClick = { viewModel.onQueryChange("") }) {
                            Icon(Icons.Rounded.Close, contentDescription = stringResource(R.string.search_clear))
                        }
                    }
                },
                singleLine = true,
                shape = RoundedCornerShape(28.dp),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                keyboardActions = KeyboardActions(onSearch = { viewModel.submit() }),
                colors = TextFieldDefaults.colors(
                    focusedContainerColor = colors.codeSurface,
                    unfocusedContainerColor = colors.codeSurface,
                    focusedIndicatorColor = Color.Transparent,
                    unfocusedIndicatorColor = Color.Transparent,
                    disabledIndicatorColor = Color.Transparent,
                    cursorColor = colors.accent,
                    focusedLeadingIconColor = colors.textSecondary,
                    unfocusedLeadingIconColor = colors.textTertiary,
                    focusedTrailingIconColor = colors.textSecondary,
                    unfocusedTrailingIconColor = colors.textTertiary,
                    focusedTextColor = colors.textPrimary,
                    unfocusedTextColor = colors.textPrimary,
                    focusedPlaceholderColor = colors.textTertiary,
                    unfocusedPlaceholderColor = colors.textTertiary,
                ),
            )

            Box(Modifier.fillMaxSize()) {
                when {
                    query.isBlank() && recent.isNotEmpty() -> RecentList(recent, viewModel::useRecent, viewModel::clearRecent)
                    query.isBlank() ->
                        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                            EmptyState(Icons.Rounded.Search, stringResource(R.string.search_title), stringResource(R.string.search_hint))
                        }
                    ui.error != null ->
                        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                            InlineError(Icons.Rounded.ErrorOutline, stringResource(R.string.search_failed), ui.error!!, onRetry = viewModel::retry, retryLabel = stringResource(R.string.common_retry))
                        }
                    ui.results.isEmpty() && !ui.searching ->
                        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                            EmptyState(Icons.Rounded.Search, stringResource(R.string.search_no_results), stringResource(R.string.search_no_results_for, ui.query))
                        }
                    else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp)) {
                        items(ui.results, key = { it.id }) { c ->
                            SessionRow(c, onClick = { viewModel.submit(); onOpenConversation(c.id) }, modifier = Modifier.animateItem(), folderName = ui.folderNames[c.folderId])
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun RecentList(recent: List<String>, onUse: (String) -> Unit, onClear: () -> Unit) {
    val colors = CodegTheme.colors
    Column(Modifier.fillMaxSize()) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(stringResource(R.string.search_recent), fontSize = 13.sp, color = colors.textSecondary, modifier = Modifier.weight(1f))
            Text(stringResource(R.string.search_clear), fontSize = 13.sp, color = colors.accent, modifier = Modifier.clip(RoundedCornerShape(6.dp)).clickable { onClear() }.padding(horizontal = 8.dp, vertical = 4.dp))
        }
        LazyColumn {
            items(recent, key = { it }) { term ->
                Row(
                    Modifier.fillMaxWidth().clickable { onUse(term) }.padding(horizontal = 16.dp, vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Icon(Icons.Rounded.History, contentDescription = null, tint = colors.textTertiary, modifier = Modifier.size(16.dp))
                    Text(term, fontSize = 14.sp, color = colors.textPrimary)
                }
            }
        }
    }
}
