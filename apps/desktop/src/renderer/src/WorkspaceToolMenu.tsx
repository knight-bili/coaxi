import * as DropdownMenu from '@radix-ui/react-dropdown-menu'
import { Archive, Boxes, Files, GitBranch, Globe, History, Monitor, TerminalSquare, Trash2 } from 'lucide-react'
import { useUiText } from './interface-language'
import type { WorkspaceTool } from './file-preview-controller'

export const workspaceTools = [
  { id: 'files', label: '工程文件', Icon: Files },
  { id: 'git', label: 'Git', Icon: GitBranch },
  { id: 'browser', label: '浏览器', Icon: Globe },
  { id: 'terminals', label: '终端', Icon: TerminalSquare },
  { id: 'computer', label: '电脑', Icon: Monitor },
  { id: 'history', label: '会话历史', Icon: History },
  { id: 'organizer', label: '整理', Icon: Archive },
  { id: 'forge', label: '仓库面板', Icon: Boxes },
  { id: 'trash', label: '回收站', Icon: Trash2 }
] as const
export type WorkspaceEntryTool = WorkspaceTool | 'terminals'
export function requestWorkspaceTool(threadId: string, tool: WorkspaceEntryTool): void {
  window.dispatchEvent(new CustomEvent('coaxi:workspace-tool', { detail: { threadId, tool } }))
}
export function WorkspaceToolItems({ onSelect, disabled = false }: { onSelect(tool: WorkspaceEntryTool): void; disabled?: boolean }): React.JSX.Element {
  const t = useUiText()
  return <>{workspaceTools.map(({ id, label, Icon }) => <DropdownMenu.Item key={id}
    className="workspace-menu-item" disabled={disabled} onSelect={() => onSelect(id)}>
    <Icon size={16} aria-hidden="true" /><span>{t(label)}</span>
  </DropdownMenu.Item>)}</>
}
