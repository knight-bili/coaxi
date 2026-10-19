// Fixed Codeg aff1e6c; Apache-2.0, integration/licenses/codeg/LICENSE.
"use client"
import type { ReactNode } from "react"
import { useMemo } from "react"
import { useTranslations } from "../../ui/source-primitives"
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuTrigger } from "../ui/context-menu"
import { parseLocalFileTarget, useNativeFileActions, useNativeFileHost } from "../../ui/native-reference-host"

export function systemFileManagerLabelKey():
  | "openInFinder"
  | "openInExplorer"
  | "openInFileManager" {
  if (typeof navigator === "undefined") return "openInFileManager"
  const platform = `${navigator.platform} ${navigator.userAgent}`.toLowerCase()
  if (platform.includes("mac")) return "openInFinder"
  if (platform.includes("win")) return "openInExplorer"
  return "openInFileManager"
}


function FileReferenceActionsMenu({ target }: { target: string }) {
  const t = useTranslations("Folder.chat.fileActions")
  const { paths, loading, error, retry, reveal: handleReveal, copy: handleCopy, download: handleDownload } = useNativeFileActions(target)
  const { local, download, loadingLabel, retryLabel } = useNativeFileHost()
  const isLocalDesktop = () => local
  const isWorkspaceFileApiAvailable = () => download
  return (
    <>
      {loading && <ContextMenuItem disabled aria-busy="true">{loadingLabel}</ContextMenuItem>}
      {error && <ContextMenuItem onSelect={event => { event.preventDefault(); retry() }}>{retryLabel}</ContextMenuItem>}
      {/* `revealItemInDir` is a no-op in web mode and for a desktop window
          bound to a remote workspace, so the row is hidden rather than dead. */}
      {isLocalDesktop() && (
        <ContextMenuItem disabled={!paths} onSelect={handleReveal}>
          {t(systemFileManagerLabelKey())}
        </ContextMenuItem>
      )}
      <ContextMenuItem
        disabled={!paths?.relative}
        onSelect={() => handleCopy(paths?.relative ?? null)}
      >
        {t("copyRelativePath")}
      </ContextMenuItem>
      <ContextMenuItem
        disabled={!paths?.absolute}
        onSelect={() => handleCopy(paths?.absolute ?? null)}
      >
        {t("copyAbsolutePath")}
      </ContextMenuItem>
      {/* Web and remote-desktop windows have no reach into the machine that
          holds the file, so downloading is how the byte stream gets to the
          user — mirroring the file tree's own row. A local desktop window can
          just open it, so the row is hidden rather than dead. */}
      {isWorkspaceFileApiAvailable() && (
        <ContextMenuItem disabled={!paths?.relative} onSelect={handleDownload}>
          {t("download")}
        </ContextMenuItem>
      )}
    </>
  )
}

export function FileReferenceActions({
  target,
  children,
}: {
  target: string
  children: ReactNode
}) {
  // Folder-independent: whether this target is a local file at all. The full
  // resolution (which needs the active folder) happens inside the open menu.
  const isLocalFile = useMemo(
    () => parseLocalFileTarget(target) !== null,
    [target]
  )

  if (!isLocalFile) return <>{children}</>

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>
        <span
          data-file-actions=""
          className="inline-flex max-w-full items-center align-middle"
          // Radix's own handler still runs (it is merged onto this same
          // element); only the ancestor conversation-panel trigger is cut off.
          onContextMenu={(event) => event.stopPropagation()}
          // Touch/pen open a context menu from a long press, which is armed on
          // pointerdown — stop that press from arming the ancestor trigger too.
          // Mouse presses keep bubbling, so the panel's selection bookkeeping is
          // untouched.
          onPointerDown={(event) => {
            if (event.pointerType !== "mouse") event.stopPropagation()
          }}
        >
          {children}
        </span>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <FileReferenceActionsMenu target={target} />
      </ContextMenuContent>
    </ContextMenu>
  )
}
