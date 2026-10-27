// Fixed Codeg aff1e6c; Apache-2.0, integration/licenses/component-a/LICENSE.
"use client"
import { memo, useMemo, type ComponentProps } from "react"
import { Streamdown, defaultRehypePlugins, defaultRemarkPlugins } from "streamdown"
import { cn } from "../../ui/source-primitives"
import { markdownLinkComponents } from "../../ui/native-markdown-links"
import { markdownLocalImageComponents } from "../../ui/native-markdown-links"
import { messageBlockComponents } from "./json-render-pre"
import { maskLiteralSpans } from "./markdown-mask"
import { rehypePluginsAllowingCoaxi } from "./rehype-allow-coaxi"
import { remarkTrimCjkAutolinkTail } from "./remark-cjk-autolink-tail"
import { withRelativeFileLinks } from "./rehype-relative-file-links"
import { remarkRewriteFileUriLinks } from "./remark-file-uri-links"
import { remarkRestoreWindowsPaths } from "./remark-windows-paths"
import { remarkLocalImages } from "./remark-local-images"
import { MATH_FENCE_PAD, useStreamdownPlugins } from "./streamdown-plugins"

export type MessageResponseProps = ComponentProps<typeof Streamdown>

// remark-math uses dollar delimiters. `\[...\]` / `\(...\)` are rewritten
// to `$$...$$`. Single-dollar `$...$` is disabled (`singleDollarTextMath:
// false`) so currency (`$9.99`) and shell vars (`$HOME`, `$1`) stay prose.
//
// A single-line `$$x$$` inside a paragraph stays *inline* math (mdast tags
// every math-text node `math-inline` regardless of dollar count). The one
// hazard is math FLOW: a `$$` that starts a line's block content opens a
// display fence and swallows the rest of that line as fence metadata. Both
// delimiters of a multi-line `\(...\)` can land there, and both are kept off
// it by putting a zero-width space in front:
//
//   opener — padded only when the `$$` would land exactly at block content
//            start, since the pad sits in the surrounding Markdown, where it
//            is a real character to later constructs. (An emphasis run or a
//            link-reference label padded on one side stops matching.)
//   closer — padded unconditionally. This pad lands INSIDE the formula,
//            where KaTeX ignores it, so it costs nothing to always emit and
//            saves having to decide whether the closing line is container
//            prefix or TeX. That question is not answerable from the line
//            alone: `\(a\n2. \)` and a list continuation look identical, as
//            do a tab-indented `>` and a blockquote marker, and a lazily
//            continued quote carries no marker at all.
//
// Code and inline code are masked BEFORE line endings are folded: the inline
// mask excludes LF but tolerates a bare CR, so folding first would un-mask
// multi-line inline code and rewrite its contents.
//
// Known limit, pre-existing: an INDENTED code block is not masked, so
// `\(...\)` inside one is rewritten (it already is on main). Padding cannot
// avoid it — 4 spaces means "indented code" at top level but "content column"
// inside a nested list, and a prefix scan cannot tell those apart.
export function normalizeMathDelimiters(text: string): string {
  const { masked, restore } = maskLiteralSpans(text)
  // Fold line endings only after masking, so the offsets and line scans below
  // match what remark-parse sees while masked code keeps its own bytes.
  const source = masked.replace(/\r\n|\r/g, "\n")
  const normalized = source
    .replace(/\\\[([\s\S]*?)\\\]/g, (_m, inner: string) => `$$${inner}$$`)
    .replace(/\\\(([\s\S]*?)\\\)/g, (_m, inner: string, offset: number) => {
      if (!inner.includes("\n")) return `$$${inner}$$`
      const lineStart = source.lastIndexOf("\n", offset - 1) + 1
      const atContentStart =
        containerPrefixEnd(source, lineStart, offset) === offset
      const open = atContentStart ? MATH_FENCE_PAD : ""
      return `${open}$$${inner}${MATH_FENCE_PAD}$$`
    })
  return restore(normalized)
}

function isSpaceOrTab(code: number): boolean {
  return code === 32 || code === 9
}

/**
 * Index just past the CommonMark container prefix (blockquote markers, list
 * markers, indentation) opening the line `[start, end)` — i.e. where that
 * line's block content begins. Returns `start` when content starts
 * immediately. Single pass, no backtracking: every iteration consumes at
 * least one character or returns.
 *
 * Indentation is deliberately not capped at 3. A deeper run only happens
 * inside a nested list (where it IS the content column, so the guard is
 * needed) or in an indented code block (where the guard is inert).
 */
function containerPrefixEnd(
  source: string,
  start: number,
  end: number
): number {
  let i = start
  for (;;) {
    while (i < end && isSpaceOrTab(source.charCodeAt(i))) i += 1
    if (i >= end) return i

    const ch = source.charCodeAt(i)
    if (ch === 62 /* > */) {
      i += 1
      continue
    }

    if (ch === 42 /* * */ || ch === 45 /* - */ || ch === 43 /* + */) {
      if (i + 1 < end && isSpaceOrTab(source.charCodeAt(i + 1))) {
        i += 2
        continue
      }
      return i
    }

    if (ch >= 48 && ch <= 57) {
      let j = i
      let digits = 0
      while (
        j < end &&
        digits < 9 &&
        source.charCodeAt(j) >= 48 &&
        source.charCodeAt(j) <= 57
      ) {
        digits += 1
        j += 1
      }
      const marker = j < end ? source.charCodeAt(j) : 0
      if (
        (marker === 46 /* . */ || marker === 41) /* ) */ &&
        j + 1 < end &&
        isSpaceOrTab(source.charCodeAt(j + 1))
      ) {
        i = j + 2
        continue
      }
      return i
    }

    return i
  }
}

const remarkPlugins = [
  ...Object.values(defaultRemarkPlugins),
  // Before remarkRewriteFileUriLinks, which reshapes a drive path's url.
  remarkRestoreWindowsPaths,
  remarkLocalImages,
  remarkRewriteFileUriLinks,
  remarkTrimCjkAutolinkTail,
]

// Streamdown's default rehype pipeline strips `coaxi://` reference hrefs in
// sanitization (rendering them as "[blocked]"); re-derive it so they survive to
// MarkdownLink → ReferenceBadge. See rehype-allow-coaxi for the full rationale.
// …and relative local links (`./a.md`, `src/a.md`, `~/a.md`) keep their href
// through harden, which would otherwise flatten them to a root path or block
// them. See rehype-relative-file-links.
const rehypePlugins = rehypePluginsAllowingCoaxi(
  withRelativeFileLinks(defaultRehypePlugins)
)

/**
 * How finished Markdown renders. Streamdown defaults to `mode="streaming"` +
 * remend; remend 1.2.0 appends a closer after spans that are ALREADY complete —
 * a glob inside code (`` `foo/*` ``), an identifier like `_meta` / `_blank` —
 * so a finished reply grew a stray `*` / `_`. When the reply ends exactly at a
 * closing fence that `_` lands after the final ```, the fence stops closing and
 * the backticks become code content (#555).
 *
 * `mode` is the half that does the work, and it is NOT interchangeable with
 * the other one. Streamdown runs remend only when `mode === "streaming" &&
 * parseIncompleteMarkdown`, so static already bypasses it — but static is also
 * the only half that repaints: Streamdown's own memo compares `mode` and NOT
 * `parseIncompleteMarkdown`, so at the live → finished flip, with the text
 * unchanged (the common case — the last delta already carried the whole
 * reply), flipping only `parseIncompleteMarkdown` leaves the remended DOM on
 * screen. `parseIncompleteMarkdown` is therefore redundant while `mode` is
 * static, and kept as an explicit invariant: it is what keeps a caller who
 * passes `mode="streaming"` for finished content from re-growing the closer.
 *
 * The impl default and the memo comparator below must agree on these, or a
 * caller passing the default explicitly would compare unequal to one omitting
 * it — hence the shared constants.
 */
const FINISHED_MODE = "static" as const
const FINISHED_PARSE_INCOMPLETE_MARKDOWN = false

/**
 * How remend repairs an unclosed link while the text is still streaming: it
 * shows the link's text, and nothing else, until the closing `)` arrives.
 *
 * remend's default instead points the partial link at the placeholder
 * `streamdown:incomplete-link`. sanitize does not allow that scheme, so it
 * drops the href, and harden turns the bare anchor into "label [blocked]" — on
 * every link, for as long as its destination is still arriving. Worse,
 * Streamdown memoizes list items, paragraphs and headings by their source span
 * alone, and the placeholder is 26 characters long: a link whose destination
 * is 26 characters too, and that ends its list item, paragraph or heading,
 * spans the same source before and after it closes, so the element never
 * repaints and the link stays "[blocked]" until the turn ends. As bare text the
 * unclosed link is always shorter than the closed one, so closing it always
 * moves the span and the element repaints.
 *
 * A caller may tune remend's other repairs, but not this one.
 */
export const LIVE_REMEND = {
  linkMode: "text-only",
} as const satisfies NonNullable<MessageResponseProps["remend"]>

function MessageResponseImpl({
  className,
  children,
  // Finished content by default; a live turn opts back into remend by passing
  // `mode="streaming" parseIncompleteMarkdown`. See FINISHED_MODE above.
  mode = FINISHED_MODE,
  parseIncompleteMarkdown = FINISHED_PARSE_INCOMPLETE_MARKDOWN,
  remend,
  ...props
}: MessageResponseProps) {
  const normalized = useMemo(
    () =>
      typeof children === "string"
        ? normalizeMathDelimiters(children)
        : children,
    [children]
  )
  const plugins = useStreamdownPlugins(
    typeof normalized === "string" ? normalized : undefined
  )

  return (
    <Streamdown
      className={cn(
        "size-full [&>*:first-child]:mt-0 [&>*:last-child]:mb-0 [&_ul]:list-disc [&_ol]:list-decimal [&_ul]:pl-3 [&_ol]:pl-3",
        // Streamdown gives `blockquote` its own `border-l-4
        // border-muted-foreground/30 … italic`, but those class names live in
        // node_modules, which Tailwind v4 does not scan — so the two border
        // utilities generate no CSS and a quote renders as bare indented text
        // with no rule (the `pl-4`/`my-4`/`text-muted-foreground` on the same
        // element only survive because they happen to be used elsewhere in src).
        // Re-declare the rule here, where it IS scanned; the descendant selector
        // also outranks Streamdown's plain utility class. No `pl-*` — its `pl-4`
        // already works and a same-specificity duplicate would be a coin flip.
        // Upright, not italic: CJK has no true italic, so a browser fakes it by
        // skewing, which looks broken.
        "[&_blockquote]:border-l-2 [&_blockquote]:border-border [&_blockquote]:not-italic",
        className
      )}
      plugins={plugins}
      remarkPlugins={remarkPlugins}
      rehypePlugins={rehypePlugins}
      mode={mode}
      parseIncompleteMarkdown={parseIncompleteMarkdown}
      remend={remend ? { ...remend, ...LIVE_REMEND } : LIVE_REMEND}
      {...props}
      // Merge after spreading props so a caller can still override other
      // elements, but the link icon + safety routing on `a` — and the diagram
      // and json-render blocks on `pre` — always win.
      components={{
        ...props.components,
        ...markdownLinkComponents,
        ...markdownLocalImageComponents,
        ...messageBlockComponents,
      }}
    >
      {normalized}
    </Streamdown>
  )
}

export const MessageResponse = memo(
  MessageResponseImpl,
  (prevProps, nextProps) =>
    prevProps.children === nextProps.children &&
    (prevProps.mode ?? FINISHED_MODE) === (nextProps.mode ?? FINISHED_MODE) &&
    (prevProps.parseIncompleteMarkdown ??
      FINISHED_PARSE_INCOMPLETE_MARKDOWN) ===
      (nextProps.parseIncompleteMarkdown ?? FINISHED_PARSE_INCOMPLETE_MARKDOWN)
)

MessageResponse.displayName = "MessageResponse"

export type MessageToolbarProps = ComponentProps<"div">

export const MessageToolbar = ({
  className,
  children,
  ...props
}: MessageToolbarProps) => (
  <div
    className={cn(
      "mt-4 flex w-full items-center justify-between gap-4",
      className
    )}
    {...props}
  >
    {children}
  </div>
)
