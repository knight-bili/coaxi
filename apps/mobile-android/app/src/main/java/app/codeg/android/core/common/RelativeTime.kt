package app.codeg.android.core.common

import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

/**
 * Compact relative-time formatting for dense list rows, ported from the iOS
 * `RelativeTime`. No third-party dependency — plain `java.time` arithmetic.
 */
object RelativeTime {
    private val monthDay: DateTimeFormatter =
        DateTimeFormatter.ofPattern("MMM d", Locale.getDefault())

    /**
     * Ultra-compact magnitude: "now", "5m", "2h", "6d", then a short date
     * ("Mar 5") past a week. No "ago"/"in" suffix (iOS `RelativeTime.compact`).
     */
    fun compact(instant: Instant, now: Instant = Instant.now()): String {
        val seconds = now.epochSecond - instant.epochSecond
        if (seconds < 60) return "now"
        val minutes = seconds / 60
        if (minutes < 60) return "${minutes}m"
        val hours = minutes / 60
        if (hours < 24) return "${hours}h"
        val days = hours / 24
        if (days < 7) return "${days}d"
        return monthDay.format(instant.atZone(ZoneId.systemDefault()))
    }
}
