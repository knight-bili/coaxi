package app.codeg.android.feature.sessiondetail.interactive

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Checklist
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.codeg.android.R
import app.codeg.android.core.designsystem.markdown.MarkdownContent
import app.codeg.android.core.designsystem.theme.CodegTheme
import app.codeg.android.core.model.PlanApprovalDecision

/**
 * The inline card for Grok's native `exit_plan_mode`. When the agent finishes planning
 * it BLOCKS on the user's decision, so this is pinned above the compose bar with the
 * plan and Grok's own three outcomes (mirroring its TUI approval bar): approve and
 * build, request changes, or abandon the plan.
 *
 * Distinct from [PermissionRequestCard], which handles Claude's ExitPlanMode — that
 * arrives as an ordinary permission with an option list, while Grok's is its own
 * request with a fixed set of outcomes plus freeform revision notes.
 *
 * Follows [AskQuestionCard]'s in-flight pattern: on success the backend's
 * `plan_approval_resolved` clears the pending state and unmounts this card, so the
 * controls stay disabled rather than flashing back on.
 */
@Composable
fun PlanApprovalCard(
    planMarkdown: String,
    onAnswer: (PlanApprovalDecision, String?) -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = CodegTheme.colors
    val plan = planMarkdown.trim()
    var submitting by remember(planMarkdown) { mutableStateOf(false) }
    var changesOpen by remember(planMarkdown) { mutableStateOf(false) }
    var feedback by remember(planMarkdown) { mutableStateOf("") }
    val notes = feedback.trim()

    fun submit(decision: PlanApprovalDecision, text: String?) {
        if (submitting) return
        submitting = true
        onAnswer(decision, text)
    }

    Column(
        modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(14.dp))
            .background(colors.bgElevated.copy(alpha = 0.96f))
            .border(0.5.dp, colors.surfaceStroke, RoundedCornerShape(14.dp))
            .padding(14.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Row(verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Icon(Icons.Rounded.Checklist, contentDescription = null, tint = colors.accent, modifier = Modifier.size(20.dp))
            Column(Modifier.weight(1f)) {
                Text(
                    stringResource(R.string.plan_approval_title),
                    fontSize = 14.sp, fontWeight = FontWeight.SemiBold, color = colors.textPrimary,
                )
                Text(stringResource(R.string.plan_approval_subtitle), fontSize = 12.sp, color = colors.textSecondary)
            }
        }

        // An empty plan still opens the approval surface (the turn is blocked either
        // way), so it gets an explicit notice instead of a blank card.
        if (plan.isEmpty()) {
            Text(stringResource(R.string.plan_approval_empty), fontSize = 13.sp, color = colors.textTertiary)
        } else {
            Column(Modifier.heightIn(max = 320.dp).verticalScroll(rememberScrollState())) {
                MarkdownContent(plan)
            }
        }

        AnimatedVisibility(changesOpen) {
            Box(
                Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(8.dp))
                    .background(colors.codeSurface)
                    .border(0.5.dp, colors.surfaceStroke, RoundedCornerShape(8.dp))
                    .padding(horizontal = 10.dp, vertical = 8.dp),
            ) {
                if (feedback.isEmpty()) {
                    Text(stringResource(R.string.plan_approval_notes_placeholder), color = colors.textTertiary, fontSize = 13.sp)
                }
                BasicTextField(
                    value = feedback,
                    onValueChange = { feedback = it },
                    enabled = !submitting,
                    textStyle = TextStyle(color = colors.textPrimary, fontSize = 13.sp),
                    cursorBrush = SolidColor(colors.accent),
                    maxLines = 5,
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        }

        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            OutlinedButton(
                onClick = {
                    if (changesOpen) submit(PlanApprovalDecision.REQUEST_CHANGES, notes) else changesOpen = true
                },
                // Opening the notes field is free; sending them needs text.
                enabled = !submitting && (!changesOpen || notes.isNotEmpty()),
                modifier = Modifier.weight(1f),
            ) {
                Text(
                    stringResource(if (changesOpen) R.string.plan_approval_send_notes else R.string.plan_approval_request_changes),
                    color = colors.textPrimary,
                )
            }
            Button(
                onClick = { submit(PlanApprovalDecision.APPROVE, null) },
                enabled = !submitting,
                colors = ButtonDefaults.buttonColors(
                    containerColor = colors.accent,
                    contentColor = colors.onAccent,
                    disabledContainerColor = colors.accent.copy(alpha = 0.4f),
                    disabledContentColor = colors.onAccent.copy(alpha = 0.8f),
                ),
                modifier = Modifier.weight(1f),
            ) {
                if (submitting) {
                    CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp, color = colors.onAccent)
                } else {
                    Text(stringResource(R.string.plan_approval_approve), fontWeight = FontWeight.SemiBold)
                }
            }
        }

        TextButton(
            onClick = { submit(PlanApprovalDecision.ABANDON, null) },
            enabled = !submitting,
            modifier = Modifier.align(Alignment.CenterHorizontally),
        ) {
            Text(stringResource(R.string.plan_approval_abandon), color = colors.textSecondary, fontSize = 12.sp)
        }
    }
}
