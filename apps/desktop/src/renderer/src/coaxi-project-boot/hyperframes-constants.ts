// Adapted from Codeg aff1e6c, Apache-2.0; see integration/licenses/component-a/LICENSE.
// Canvas resolution presets for `hyperframes init --resolution <preset>`,
// rendered as selectable cards (title + dimension hint). "default" keeps the
// template's own dimensions and omits the flag.
export const HYPERFRAMES_RESOLUTION_OPTIONS = [
  { value: "default", label: "Template default", hint: "Keep size" },
  { value: "landscape", label: "Landscape", hint: "1920×1080" },
  { value: "portrait", label: "Portrait", hint: "1080×1920" },
  { value: "square", label: "Square", hint: "1080×1080" },
  { value: "landscape-4k", label: "Landscape 4K", hint: "3840×2160" },
  { value: "portrait-4k", label: "Portrait 4K", hint: "2160×3840" },
]

// coaxi's six agents, mapped to the `skills` CLI `--agent` ids used by the
// global HyperFrames skills install. Order matches the backend allowlist.
export const HYPERFRAMES_SKILL_AGENTS = [
  { id: "claude-code", label: "Claude Code" },
  { id: "codex", label: "Codex" },
  { id: "opencode", label: "OpenCode" },
  { id: "gemini-cli", label: "Gemini" },
  { id: "openclaw", label: "OpenClaw" },
  { id: "cline", label: "Cline" },
]
