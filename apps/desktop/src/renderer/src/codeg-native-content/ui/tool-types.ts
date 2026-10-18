// Only the AI SDK UI state/input/output type shape used by the source tool shell.
// This does not load an SDK runtime or authorize tool execution.
export interface ToolUIPart {
  type: `tool-${string}`
  state: 'approval-requested' | 'approval-responded' | 'input-available' | 'input-streaming' | 'output-available' | 'output-denied' | 'output-error'
  input?: unknown
  output?: unknown
}
export interface DynamicToolUIPart extends Omit<ToolUIPart, 'type'> { type: 'dynamic-tool' }
