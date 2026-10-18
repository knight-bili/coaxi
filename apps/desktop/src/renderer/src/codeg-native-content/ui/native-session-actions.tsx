import { createContext, useContext, useEffect, type ReactNode } from 'react'

export type SourceSessionTarget = {
  kind: 'agentSession'; sessionId: string; agentType: string
  subagentType: string | null; description: string | null; live?: boolean
}
type SourceSessionHost = { open(target: SourceSessionTarget): void }
const Context = createContext<SourceSessionHost | null>(null)
export function NativeSessionActionsProvider({ open, children }: SourceSessionHost & { children: ReactNode }): React.JSX.Element {
  return <Context.Provider value={{ open }}>{children}</Context.Provider>
}
export function useSessionViewerHost(): SourceSessionHost {
  const host = useContext(Context)
  if (!host) throw new Error('Native source cards require their Core session host')
  return host
}
// The source App's standalone fallback joins the same Core-owned viewer stack.
// It never imports the donor's session store, Tauri APIs or filesystem reader.
export function SubagentSessionDialog({ open, onOpenChange, sessionId, agentType, subagentType, description, live }: Omit<SourceSessionTarget, 'kind'> & { open: boolean; onOpenChange(open: boolean): void }): null {
  const host = useSessionViewerHost()
  useEffect(() => {
    if (!open) return
    host.open({ kind: 'agentSession', sessionId, agentType, subagentType, description, live })
    onOpenChange(false)
  }, [open, host, onOpenChange, sessionId, agentType, subagentType, description, live])
  return null
}
