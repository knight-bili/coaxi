import * as DropdownMenu from '@radix-ui/react-dropdown-menu'
import { FilePlus, History, MessageSquarePlus, Plus } from 'lucide-react'
import { UiText, uiAttribute } from './interface-language'

export function ComposerToolsMenu({ disabled, attachmentDisabled, onAttach, onFeedback, onSources }: {
  disabled: boolean
  attachmentDisabled?: boolean
  onAttach(): void
  onFeedback(): void
  onSources(): void
}): React.JSX.Element {
  return <DropdownMenu.Root>
    <DropdownMenu.Trigger className="composer-attachment-button" disabled={disabled}
      aria-label={uiAttribute('添加文件或引用')} title={uiAttribute('添加文件或引用')}>
      <Plus size={18} aria-hidden="true" />
    </DropdownMenu.Trigger>
    <DropdownMenu.Portal>
      <DropdownMenu.Content className="composer-tools-menu" side="top" align="start" sideOffset={8} collisionPadding={12}>
        <DropdownMenu.Item disabled={attachmentDisabled} onSelect={onAttach}><FilePlus size={16} /><UiText zh="添加文件" /></DropdownMenu.Item>
        <DropdownMenu.Item onSelect={onFeedback}><MessageSquarePlus size={16} /><UiText zh="补充当前工作" /></DropdownMenu.Item>
        <DropdownMenu.Item onSelect={onSources}><History size={16} /><UiText zh="引用旧对话" /></DropdownMenu.Item>
      </DropdownMenu.Content>
    </DropdownMenu.Portal>
  </DropdownMenu.Root>
}
