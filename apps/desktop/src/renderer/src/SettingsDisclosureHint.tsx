import { UiText } from './interface-language'

/** Visible action for native details; open state stays owned by the details element. */
export function SettingsDisclosureHint(): React.JSX.Element {
  return <span className="settings-disclosure-hint" aria-hidden="true">
    <span className="when-closed"><UiText zh="展开" /></span>
    <span className="when-open"><UiText zh="收起" /></span>
  </span>
}
