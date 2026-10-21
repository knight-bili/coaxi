package app.codeg.android.core.designsystem.diff

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.codeg.android.R
import app.codeg.android.core.designsystem.theme.CodegTheme

private val AddedFg = Color(0xFF8CE69E)
private val DeletedFg = Color(0xFFF58585)
private val AddedBg = Color(0xFF3DDC84).copy(alpha = 0.12f)
private val DeletedBg = Color(0xFFF55555).copy(alpha = 0.12f)

/** Renders a parsed unified diff (one or more files). Port of the iOS `DiffView`. */
@Composable
fun DiffView(files: List<DiffFile>, modifier: Modifier = Modifier, collapseAfter: Int = 80) {
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        for (file in files) DiffFileView(file, collapseAfter)
    }
}

@Composable
private fun DiffFileView(file: DiffFile, collapseAfter: Int) {
    val colors = CodegTheme.colors
    val allRows = remember(file) { file.hunks.flatMap { it.rows } }
    val collapsible = allRows.size > collapseAfter
    var expanded by remember(file) { mutableStateOf(false) }
    val rows = if (collapsible && !expanded) allRows.take(collapseAfter) else allRows

    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(10.dp))
            .background(colors.codeSurface)
            .border(0.5.dp, colors.hairline, RoundedCornerShape(10.dp)),
    ) {
        // Header: mode badge + path + counts.
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 10.dp, vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            ModeBadge(file.mode)
            Text(
                text = shortPath(file.path),
                fontFamily = FontFamily.Monospace,
                fontSize = 11.5.sp,
                color = colors.textSecondary,
                modifier = Modifier.weight(1f),
                maxLines = 1,
            )
            if (file.additions > 0) {
                Text("+${file.additions}", color = AddedFg, fontSize = 11.sp, fontWeight = FontWeight.SemiBold)
            }
            if (file.deletions > 0) {
                Text("−${file.deletions}", color = DeletedFg, fontSize = 11.sp, fontWeight = FontWeight.SemiBold)
            }
        }
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState())) {
            Column {
                for (row in rows) DiffRowView(row)
            }
        }
        if (collapsible) {
            Text(
                text = if (expanded) stringResource(R.string.diff_show_less) else stringResource(R.string.diff_show_more_lines, allRows.size - collapseAfter),
                style = MaterialTheme.typography.labelSmall,
                color = colors.accent,
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { expanded = !expanded }
                    .padding(horizontal = 10.dp, vertical = 6.dp),
            )
        }
    }
}

@Composable
private fun DiffRowView(row: DiffRow) {
    val colors = CodegTheme.colors
    val (fg, bg, sign) = when (row.kind) {
        DiffRow.Kind.ADDED -> Triple(AddedFg, AddedBg, "+")
        DiffRow.Kind.DELETED -> Triple(DeletedFg, DeletedBg, "−")
        DiffRow.Kind.CONTEXT -> Triple(colors.textSecondary, Color.Transparent, " ")
    }
    Row(
        Modifier.background(bg).padding(vertical = 1.dp),
        verticalAlignment = Alignment.Top,
    ) {
        Gutter(row.oldLine)
        Gutter(row.newLine)
        Text(
            text = sign,
            fontFamily = FontFamily.Monospace,
            fontSize = 12.sp,
            color = fg,
            modifier = Modifier.width(14.dp).padding(start = 4.dp),
        )
        Text(
            text = if (row.text.isEmpty()) " " else row.text,
            fontFamily = FontFamily.Monospace,
            fontSize = 12.sp,
            lineHeight = 16.sp,
            color = if (row.kind == DiffRow.Kind.CONTEXT) colors.textPrimary else fg,
            modifier = Modifier.padding(end = 10.dp),
        )
    }
}

@Composable
private fun Gutter(line: Int?) {
    Text(
        text = line?.toString() ?: "",
        fontFamily = FontFamily.Monospace,
        fontSize = 10.sp,
        color = CodegTheme.colors.textTertiary,
        modifier = Modifier.width(34.dp).padding(start = 6.dp),
    )
}

@Composable
private fun ModeBadge(mode: DiffFile.Mode) {
    val (label, color) = when (mode) {
        DiffFile.Mode.ADDED -> stringResource(R.string.diff_mode_added) to AddedFg
        DiffFile.Mode.MODIFIED -> stringResource(R.string.diff_mode_modified) to CodegTheme.colors.accent
        DiffFile.Mode.DELETED -> stringResource(R.string.diff_mode_deleted) to DeletedFg
        DiffFile.Mode.RENAMED -> stringResource(R.string.diff_mode_renamed) to CodegTheme.colors.textSecondary
    }
    Text(
        text = label,
        fontSize = 9.sp,
        fontWeight = FontWeight.Bold,
        color = color,
        modifier = Modifier
            .clip(RoundedCornerShape(4.dp))
            .background(color.copy(alpha = 0.14f))
            .padding(horizontal = 5.dp, vertical = 2.dp),
    )
}

private fun shortPath(path: String): String {
    val parts = path.split("/")
    return if (parts.size <= 3) path else ".../" + parts.takeLast(2).joinToString("/")
}
