import { createContext, useContext } from 'react'
export const NativeMessageScroll = createContext<{ scrollToIndex(index: number, options: { align: 'start'; smooth: boolean }): void } | null>(null)
export const useMessageScroll = () => useContext(NativeMessageScroll)
export const useChatAnimationsEnabled = () => typeof matchMedia !== 'undefined' && !matchMedia('(prefers-reduced-motion: reduce)').matches
export const useModelLabel = () => (id: string) => id
// This readonly native source has no task-write owner. TurnStats hides that
// action; the original owning Thread keeps its existing task/mission controls.
export function useCreateTaskFromMessage(_getText: () => string): (() => void) | undefined { return undefined }
