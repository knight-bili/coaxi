import type { ThreadComposerDraftView, ThreadMessageAttachmentView } from '@contracts'
import type { ThreadClient } from './camp-client'
import { MAX_COMPOSER_ATTACHMENTS } from './camp-composer-local-store'

/** Stage a resource and its image before publishing either to the draft. */
export async function prepareComposerFileBatch(
  client: Pick<ThreadClient, 'composerAttachments'>,
  draft: ThreadComposerDraftView,
  files: File[],
  isCurrent: () => boolean
): Promise<ThreadMessageAttachmentView[]> {
  if (draft.attachments.length + files.length > MAX_COMPOSER_ATTACHMENTS) {
    throw new Error(`最多添加 ${MAX_COMPOSER_ATTACHMENTS} 个附件，请先移除部分附件。`)
  }
  const staged: ThreadMessageAttachmentView[] = []
  const assertCurrent = (): void => {
    if (!isCurrent()) throw new Error('原对话已切换，资料没有加入其他对话。')
  }
  try {
    for (const file of files) {
      assertCurrent()
      staged.push(await client.composerAttachments.prepare(draft.threadId, draft.revision, file))
      assertCurrent()
    }
    return [...draft.attachments, ...staged]
  } catch (error) {
    if (staged.length) await client.composerAttachments.discard?.(draft.threadId, staged.map(item => item.id)).catch(() => undefined)
    throw error
  }
}
