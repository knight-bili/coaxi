import { createContext, useContext, type ReactNode } from 'react'

export const NativeFileActions = createContext<((path: string, line?: number | null) => Promise<void>) | null>(null)
export function NativeFileActionsProvider({ open, children }: { open: ((path: string, line?: number | null) => Promise<void>) | null; children: ReactNode }): React.JSX.Element {
  return <NativeFileActions.Provider value={open}>{children}</NativeFileActions.Provider>
}
// A transcript path never acquires authority from the currently selected folder.
export function useActiveFolder(): { activeFolder: { path: string } | null } { return { activeFolder: null } }
export function useNativeFileAction(): ((path: string, line?: number | null) => Promise<void>) | null {
  return useContext(NativeFileActions)
}
