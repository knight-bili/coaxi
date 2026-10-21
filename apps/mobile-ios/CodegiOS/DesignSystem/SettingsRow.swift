import SwiftUI

/// Shared metrics for grouped Settings rows so the inset divider lines up under
/// the title (past the leading icon badge), iOS-style.
enum SettingsRowMetrics {
    static let badgeSize: CGFloat = 29
    static let iconGap: CGFloat = 12
    static let hInset: CGFloat = 16
    static let vInset: CGFloat = 10
    /// Leading inset for the inter-row divider: aligns with the title text.
    static var dividerInset: CGFloat { hInset + badgeSize + iconGap }
}

/// An iOS-Settings-style icon: a legible glyph centered on a rounded-square fill.
/// The fill is the single app accent by default so every badge tracks the theme
/// palette live; the glyph uses `Theme.onAccent` so it stays readable on any
/// accent. Pass `tint` to override the fill per row.
struct SettingsIconBadge: View {
    let icon: String
    var tint: Color = Theme.accent

    var body: some View {
        Image(systemName: icon)
            .font(.system(size: 15, weight: .semibold))
            .foregroundStyle(Theme.onAccent)
            .frame(width: SettingsRowMetrics.badgeSize, height: SettingsRowMetrics.badgeSize)
            .background(tint, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
    }
}

/// The shared visual for one row inside a grouped Settings section: a tinted icon
/// badge, a title, an optional trailing detail value, and a chevron. It draws no
/// surface of its own — the enclosing `EditorSection` glass card provides the
/// background and rows are separated by `SettingsRowDivider`. The whole row is
/// hit-testable via `.contentShape`, so a tap anywhere (including the blank gap)
/// registers, not just on the glyph or text.
struct SettingsGroupedRowLabel: View {
    let icon: String
    var tint: Color = Theme.accent
    let title: LocalizedStringKey
    var detail: LocalizedStringKey? = nil

    var body: some View {
        HStack(spacing: SettingsRowMetrics.iconGap) {
            SettingsIconBadge(icon: icon, tint: tint)
            Text(title)
                .foregroundStyle(Theme.textPrimary)
            Spacer(minLength: 8)
            if let detail {
                Text(detail)
                    .font(.subheadline)
                    .foregroundStyle(Theme.textSecondary)
                    .lineLimit(1)
            }
            Image(systemName: "chevron.right")
                .font(.caption.weight(.semibold))
                .foregroundStyle(Theme.textTertiary)
        }
        .padding(.horizontal, SettingsRowMetrics.hInset)
        .padding(.vertical, SettingsRowMetrics.vInset)
        .frame(maxWidth: .infinity, alignment: .leading)
        .contentShape(Rectangle()) // whole-row hit area — fixes dead-zone taps
    }
}

/// A whole-row `NavigationLink` to a leaf Settings screen, rendered as a grouped
/// section row (icon badge + title + optional detail + chevron). Value-based
/// (`NavigationLink(value:)`), so the same row serves taps and programmatic /
/// deep-link navigation. Generic over the pushed value to keep this layer free
/// of any Feature type.
struct SettingsGroupedNavRow<Value: Hashable>: View {
    let icon: String
    var tint: Color = Theme.accent
    let title: LocalizedStringKey
    var detail: LocalizedStringKey? = nil
    let value: Value

    var body: some View {
        NavigationLink(value: value) {
            SettingsGroupedRowLabel(icon: icon, tint: tint, title: title, detail: detail)
        }
        .buttonStyle(.plain)
    }
}

/// Inset hairline between grouped rows — starts under the title, not at the card
/// edge, mirroring the iOS grouped-list separator. A thin alias over the generic
/// ``InsetDivider`` at the Settings row's title inset.
struct SettingsRowDivider: View {
    var body: some View {
        InsetDivider(leading: SettingsRowMetrics.dividerInset)
    }
}

/// A grouped-list "radio" option row: a leading SF Symbol, a title, and a
/// trailing checkmark when selected; the whole row is tappable. Shared by the
/// Appearance (theme) and Language pickers, which present a single-choice list
/// of an enum's cases. Lives inside an ``EditorSection`` with ``InsetDivider``s
/// between rows.
struct SelectableRow: View {
    let symbol: String
    let title: LocalizedStringKey
    let isSelected: Bool
    /// Accent tint for the leading glyph and selected checkmark. Defaults to the
    /// global `Theme.accent` token; callers that render where the bridged accent
    /// trait can't reach — the Appearance page, which also shows inside the iPad
    /// Settings sheet — pass an explicitly resolved color so the row recolors there.
    var tint: Color = Theme.accent
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 12) {
                Image(systemName: symbol)
                    .font(.system(size: 16, weight: .semibold))
                    .foregroundStyle(tint)
                    .frame(width: 26)
                Text(title)
                    .foregroundStyle(Theme.textPrimary)
                Spacer(minLength: 8)
                if isSelected {
                    Image(systemName: "checkmark")
                        .font(.subheadline.weight(.bold))
                        .foregroundStyle(tint)
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 14)
            .frame(maxWidth: .infinity, alignment: .leading)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

// MARK: - Generic grouped-list building blocks

/// A hairline separator for grouped lists whose leading inset can start under the
/// row's title (past a leading badge/tile), iOS grouped-list style. The generic
/// primitive behind ``SettingsRowDivider`` and the Folders feature's grouped rows
/// — pass the row's `(hInset + leadingGlyphWidth + gap)` so the line aligns under
/// the title text. `leading: 0` draws edge-to-edge.
struct InsetDivider: View {
    var leading: CGFloat = 0

    var body: some View {
        Divider()
            .overlay(Theme.hairline)
            .padding(.leading, leading)
    }
}

/// A content-agnostic borderless row for grouped lists (inside an `EditorSection`
/// / `GlassCard(padding: 0)` surface, or `List` rows over `CodegBackground`). It
/// draws no surface of its own — the enclosing card provides the background and
/// ``InsetDivider``s separate rows. The whole row is hit-testable via
/// `.contentShape(Rectangle())`, so a tap anywhere (including the blank gaps)
/// registers, not just on the glyph or text (the dead-zone fix mirrored from
/// ``SettingsGroupedRowLabel``). Callers supply only the inner content (typically
/// an `HStack`); use the matching ``InsetDivider`` leading inset to align the
/// separators under the content's title.
struct GroupedRow<Content: View>: View {
    var hInset: CGFloat = SettingsRowMetrics.hInset
    var vInset: CGFloat = 11
    @ViewBuilder var content: () -> Content

    var body: some View {
        content()
            .padding(.horizontal, hInset)
            .padding(.vertical, vInset)
            .frame(maxWidth: .infinity, alignment: .leading)
            .contentShape(Rectangle())
    }
}
