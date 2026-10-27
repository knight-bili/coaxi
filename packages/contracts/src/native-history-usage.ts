// Fixed Coaxi aff1e6c source DTOs; native history statistics only.
export type TokenUsageBucket = "day" | "week" | "month"

export interface TokenUsageFilter {
  /** Inclusive lower bound, ISO-8601. Omit for "since the first recorded turn". */
  start?: string | null
  /** Exclusive upper bound, ISO-8601. Omit for "up to now". */
  end?: string | null
  /** Selected folders; each is expanded server-side to its worktree children. */
  folderIds?: number[] | null
  /** `conversation.agent_type` wire names. */
  agentTypes?: string[] | null
  models?: string[] | null
  bucket: TokenUsageBucket
  /** `-new Date().getTimezoneOffset()` — all buckets are local-time buckets. */
  tzOffsetMinutes: number
  /** Also compute the equally-long window before `start`, for delta chips. */
  comparePrevious?: boolean
}

export interface TokenUsageTotals {
  input_tokens: number
  output_tokens: number
  cache_creation_tokens: number
  cache_read_tokens: number
  total_tokens: number
  turn_count: number
  conversation_count: number
  /** Summed generation time of the counted turns, not time spent in the app. */
  duration_ms: number
  active_days: number
}

export interface TokenUsagePoint {
  /** `YYYY-MM-DD` (day/week) or `YYYY-MM` (month), in the viewer's local time. */
  bucket_key: string
  start: string
  end: string
  input_tokens: number
  output_tokens: number
  cache_creation_tokens: number
  cache_read_tokens: number
  total_tokens: number
  turn_count: number
  conversation_count: number
}

export interface TokenUsageBreakdownItem {
  /** Folder id as a string, agent wire name, or model name. */
  key: string
  label: string
  input_tokens: number
  output_tokens: number
  cache_creation_tokens: number
  cache_read_tokens: number
  total_tokens: number
  turn_count: number
  conversation_count: number
}

export interface TokenUsageHeatCell {
  /** 0 = Monday … 6 = Sunday, local time. */
  weekday: number
  /** 0–23, local time. */
  hour: number
  total_tokens: number
  turn_count: number
}

export interface TokenUsageConversationItem {
  conversation_id: number
  title: string | null
  agent_type: string
  folder_label: string | null
  total_tokens: number
  turn_count: number
  last_activity_at: string
}

export interface TokenUsageStreak {
  longest_days: number
  current_days: number
  current_ends_on: string | null
}

export interface TokenUsageReport {
  range_start: string | null
  range_end: string | null
  bucket: TokenUsageBucket
  totals: TokenUsageTotals
  previous_totals: TokenUsageTotals | null
  series: TokenUsagePoint[]
  by_folder: TokenUsageBreakdownItem[]
  by_agent: TokenUsageBreakdownItem[]
  by_model: TokenUsageBreakdownItem[]
  heatmap: TokenUsageHeatCell[]
  top_conversations: TokenUsageConversationItem[]
  streak: TokenUsageStreak
  first_activity_at: string | null
  last_activity_at: string | null
  /** The scan hit its row cap — the numbers cover only the most recent slice. */
  truncated: boolean
}

export interface TokenUsageFolderFacet {
  folder_id: number
  /** Compact display name: the alias when set, else the folder name. The filter
   *  list renders `alias [ name ]` from `name` + `alias` instead. */
  label: string
  /** The folder's real (on-disk) directory name, alias or not. */
  name: string
  /** User-set display alias, or null when unset. */
  alias: string | null
  path: string
  parent_id: number | null
}

export interface TokenUsageFacets {
  folders: TokenUsageFolderFacet[]
  agents: string[]
  models: string[]
  data_start: string | null
  data_end: string | null
}

export interface TokenUsageSyncStatus {
  total_conversations: number
  synced_conversations: number
  stale_conversations: number
  fact_rows: number
  last_synced_at: string | null
  running: boolean
}

export interface TokenUsageSyncResult {
  scanned: number
  synced: number
  skipped: number
  /** Real faults — retried next pass. The only counter that warrants a toast. */
  failed: number
  /** Transcripts that are gone for good: facts kept, stamp settled, never
   *  retried. Deliberately silent — the reader cannot act on it. */
  lost: number
  turns_written: number
  tokens_written: number
  pruned_conversations: number
}

/** Payload of the `token-usage-sync://progress` event. */
export interface TokenUsageSyncProgress {
  done: number
  total: number
  current_title: string | null
  /** Present only on the final tick. */
  result: TokenUsageSyncResult | null
}
