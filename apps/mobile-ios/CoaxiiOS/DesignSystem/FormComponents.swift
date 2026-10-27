import SwiftUI

/// A titled glass section grouping related fields, with an optional footer.
/// Shared by the editor-style sheets (Add/Edit Server, Clone Repository).
struct EditorSection<Content: View>: View {
    let title: LocalizedStringKey
    var footer: LocalizedStringKey?
    /// Render the grouping container flat (``FlatCard``: plain fill, no Liquid Glass
    /// elevation/hairline) instead of the default glass. Sheets presented on the
    /// light near-white backdrop — e.g. the commit composer — use this because a
    /// frosted glass card reads as a shadowed plate there; a flat fill sits calmly.
    var flat: Bool = false
    @ViewBuilder var content: () -> Content

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // `.textCase(.uppercase)` (not `String.uppercased()`) so the key still
            // matches the source literal for catalog lookup, and Chinese — which
            // has no case — is left untouched while English headers uppercase.
            Text(title)
                .textCase(.uppercase)
                .font(.caption.weight(.semibold))
                .foregroundStyle(Theme.textTertiary)
                .tracking(0.6)
                .padding(.leading, 4)

            if flat {
                FlatCard(cornerRadius: Theme.Radius.lg, padding: 0) {
                    VStack(spacing: 0) { content() }
                }
            } else {
                GlassCard(cornerRadius: Theme.Radius.lg, padding: 0) {
                    VStack(spacing: 0) { content() }
                }
            }

            if let footer {
                Text(footer)
                    .font(.caption)
                    .foregroundStyle(Theme.textTertiary)
                    .padding(.leading, 4)
                    .padding(.top, 1)
            }
        }
    }
}

/// A labeled field row: small caption label above the editable control.
struct FieldRow<Content: View>: View {
    let label: LocalizedStringKey
    @ViewBuilder var content: () -> Content

    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(label)
                .font(.caption.weight(.medium))
                .foregroundStyle(Theme.textSecondary)
            content()
                .font(.body)
                .foregroundStyle(Theme.textPrimary)
                .tint(Theme.accent)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 16)
        .padding(.vertical, 13)
    }
}

// MARK: - Dropdown (select box)

/// The shared dropdown chrome: a bordered "select box" showing the current value
/// with a trailing chevron, so it reads as a tappable control. (A bare
/// `.pickerStyle(.menu)` renders as plain accent text with a hard-to-see chevron
/// — it doesn't look interactive.) Tapping opens `menu`; pass a `Picker` with
/// `.pickerStyle(.inline)` (optionally grouped into `Section`s) as the content.
/// `SelectField` is the common flat-list convenience on top of this.
struct SelectBox<MenuContent: View>: View {
    private let label: Text
    private let isPlaceholder: Bool
    @ViewBuilder var menu: () -> MenuContent

    /// String display. Fixed labels (e.g. "Low") localize via a runtime catalog
    /// lookup; dynamic values (provider/model names) fall back to the key. An
    /// empty `display` shows `placeholder` (or "—") in the tertiary color.
    init(display: String, placeholder: String = "", @ViewBuilder menu: @escaping () -> MenuContent) {
        let empty = display.isEmpty
        self.isPlaceholder = empty
        self.label = Text(LocalizedStringKey(stringLiteral: empty ? (placeholder.isEmpty ? "—" : placeholder) : display))
        self.menu = menu
    }

    /// `LocalizedStringKey` display, for labels with format arguments (e.g.
    /// "System default (%@)") that a `stringLiteral` lookup would flatten. Resolves
    /// against the in-app locale via `Text`, unlike `String(localized:)`.
    init(display: LocalizedStringKey, @ViewBuilder menu: @escaping () -> MenuContent) {
        self.isPlaceholder = false
        self.label = Text(display)
        self.menu = menu
    }

    var body: some View {
        Menu {
            menu()
        } label: {
            HStack(spacing: 8) {
                label
                    .foregroundStyle(isPlaceholder ? Theme.textTertiary : Theme.textPrimary)
                    .lineLimit(1)
                    .truncationMode(.tail)
                Spacer(minLength: 8)
                Image(systemName: "chevron.up.chevron.down")
                    .font(.caption2.weight(.bold))
                    .foregroundStyle(Theme.accent)
            }
            .padding(.horizontal, 13)
            .padding(.vertical, 11)
            .frame(maxWidth: .infinity)
            .background(Theme.bgElevated, in: RoundedRectangle(cornerRadius: Theme.Radius.sm))
            .overlay(RoundedRectangle(cornerRadius: Theme.Radius.sm).stroke(Theme.hairline, lineWidth: 1))
            .contentShape(RoundedRectangle(cornerRadius: Theme.Radius.sm))
        }
    }
}

/// One choice in a `SelectField`. `id == value` so a list of options is
/// `ForEach`-able without a separate identifier.
struct SelectOption<Value: Hashable>: Identifiable {
    let value: Value
    let label: String
    var id: Value { value }
}

/// A flat single-select dropdown bound to `selection`. Renders the selected
/// option's label in a `SelectBox`; `placeholder` shows when nothing matches
/// (e.g. an unset optional). For grouped menus use `SelectBox` directly.
struct SelectField<Value: Hashable>: View {
    @Binding var selection: Value
    let options: [SelectOption<Value>]
    var placeholder: String = ""

    private var currentLabel: String { options.first { $0.value == selection }?.label ?? "" }

    var body: some View {
        SelectBox(display: currentLabel, placeholder: placeholder) {
            Picker("", selection: $selection) {
                ForEach(options) { Text(LocalizedStringKey(stringLiteral: $0.label)).tag($0.value) }
            }
            .pickerStyle(.inline)
        }
    }
}
