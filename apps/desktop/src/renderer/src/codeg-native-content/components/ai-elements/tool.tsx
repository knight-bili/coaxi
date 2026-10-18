// Fixed Codeg aff1e6c; Apache-2.0, integration/licenses/codeg/LICENSE.
"use client"

import type { DynamicToolUIPart, ToolUIPart } from "../../ui/tool-types"
import type { ComponentProps, ReactNode } from "react"

import { Badge } from "../../ui/source-primitives"
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "../ui/instant-collapsible"
import { cn } from "../../ui/source-primitives"
import {
  CheckCircleIcon,
  ChevronDownIcon,
  CircleIcon,
  ClockIcon,
  WrenchIcon,
  XCircleIcon,
} from "lucide-react"
import { useTranslations } from "../../ui/source-primitives"

import { CodeBlock } from "./code-block"

export type ToolProps = ComponentProps<typeof Collapsible>

export const Tool = ({ className, ...props }: ToolProps) => (
  <Collapsible
    className={cn("group w-full rounded-md border border-border/60", className)}
    {...props}
  />
)

export type ToolPart = ToolUIPart | DynamicToolUIPart

export type ToolHeaderProps = {
  title?: ReactNode
  titleSuffix?: ReactNode
  icon?: ReactNode
  className?: string
} & (
  | { type: ToolUIPart["type"]; state: ToolUIPart["state"]; toolName?: never }
  | {
      type: DynamicToolUIPart["type"]
      state: DynamicToolUIPart["state"]
      toolName: string
    }
)

const statusIcons: Record<ToolPart["state"], ReactNode> = {
  "approval-requested": <ClockIcon className="size-4 text-yellow-600" />,
  "approval-responded": <CheckCircleIcon className="size-4 text-blue-600" />,
  "input-available": <ClockIcon className="size-4 animate-pulse" />,
  "input-streaming": <CircleIcon className="size-4" />,
  "output-available": <CheckCircleIcon className="size-4 text-green-600" />,
  "output-denied": <XCircleIcon className="size-4 text-orange-600" />,
  "output-error": <XCircleIcon className="size-4 text-red-600" />,
}

export const getStatusBadge = (status: ToolPart["state"], label: string) => (
  <Badge className="gap-1.5 rounded-full text-xs" variant="secondary">
    {statusIcons[status]}
    {label}
  </Badge>
)

export const ToolHeader = ({
  className,
  title,
  titleSuffix,
  icon,
  type,
  state,
  toolName,
  ...props
}: ToolHeaderProps) => {
  const t = useTranslations("Folder.chat.tool")
  const derivedName =
    type === "dynamic-tool" ? toolName : type.split("-").slice(1).join("-")
  const statusLabel =
    state === "approval-requested"
      ? t("status.approvalRequested")
      : state === "approval-responded"
        ? t("status.approvalResponded")
        : state === "input-available"
          ? t("status.inputAvailable")
          : state === "input-streaming"
            ? t("status.inputStreaming")
            : state === "output-available"
              ? t("status.outputAvailable")
              : state === "output-denied"
                ? t("status.outputDenied")
                : t("status.outputError")

  return (
    <CollapsibleTrigger
      className={cn(
        "flex w-full min-w-0 items-center justify-between gap-4 px-3 py-2",
        className
      )}
      {...props}
    >
      <div className="flex min-w-0 items-center gap-2">
        <span className="shrink-0">
          {icon ?? <WrenchIcon className="size-4 text-muted-foreground" />}
        </span>
        <span className="min-w-0 flex-1 truncate whitespace-nowrap font-medium text-sm">
          {title ?? derivedName}
        </span>
        {titleSuffix ? <span className="shrink-0">{titleSuffix}</span> : null}
        <span className="shrink-0">{getStatusBadge(state, statusLabel)}</span>
      </div>
      <ChevronDownIcon className="size-4 shrink-0 text-muted-foreground transition-transform group-data-[state=open]:rotate-180" />
    </CollapsibleTrigger>
  )
}

export type ToolContentProps = ComponentProps<typeof CollapsibleContent>

export const ToolContent = ({
  className,
  children,
  ...props
}: ToolContentProps) => (
  <CollapsibleContent
    className={cn(
      "tool-collapsible-content overflow-hidden text-popover-foreground outline-none",
      className
    )}
    {...props}
  >
    <div className="space-y-4 p-4">{children}</div>
  </CollapsibleContent>
)

export type ToolInputProps = ComponentProps<"div"> & {
  input: ToolPart["input"]
}

export const ToolInput = ({ className, input, ...props }: ToolInputProps) => {
  const t = useTranslations("Folder.chat.tool")
  const formattedCode = (() => {
    if (typeof input === "string") {
      try {
        const parsed = JSON.parse(input)
        return JSON.stringify(parsed, null, 2)
      } catch {
        return input
      }
    }
    return JSON.stringify(input, null, 2)
  })()

  return (
    <div className={cn("space-y-2 overflow-hidden", className)} {...props}>
      <h4 className="font-medium text-muted-foreground text-xs uppercase tracking-wide">
        {t("parameters")}
      </h4>
      <div className="rounded-md bg-muted/50">
        <CodeBlock code={formattedCode} language="json" />
      </div>
    </div>
  )
}
