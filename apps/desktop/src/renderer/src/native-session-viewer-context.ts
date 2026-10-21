import { createContext } from 'react'
import type { NativeReaderTarget } from './native-child-reader'

export interface NativeSessionTarget {
  kind: 'native'
  identity: string
  threadId: string
  parentConversationId: string
  runtime: 'codex' | 'grok' | 'claude' | 'qoder'
  subtitle: string | null
  source: NativeReaderTarget
}
export const NativeSessionViewerContext = createContext<{
  open(target: NativeSessionTarget, trigger?: HTMLElement): void
  visible: boolean
} | null>(null)
