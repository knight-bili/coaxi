// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import type { ThreadClient } from './camp-client'
import { ThreadClientProvider } from './camp-client'
import { WorkspaceGitViews } from './WorkspaceGitViews'

const diffPreview = vi.hoisted(() => vi.fn())

vi.mock('./codeg-native-content/components/diff/unified-diff-preview', () => ({
  UnifiedDiffPreview: (props: { diffText: string }) => diffPreview(props)
}))

afterEach(cleanup)

beforeEach(() => {
  diffPreview.mockReset()
  diffPreview.mockImplementation(({ diffText }: { diffText: string }) => <pre data-testid="git-diff">{diffText}</pre>)
})

type CommitFile = { path: string; originalPath?: string; kind: 'added' | 'deleted' | 'renamed' | 'modified'; binary: boolean }
type GitResult = { success: boolean; output: string; error: string; files?: CommitFile[]; filesError?: string }
type GitRead = { scope: { threadId: string }; action: string; target?: string; paths?: string[] }
type GitRequest = (method: string, params: GitRead) => Promise<GitResult>

function logOutput(records: Array<{ sha: string; parents: string[]; message: string }>): string {
  return records.map(record => [
    record.sha,
    record.parents.join(' '),
    '测试作者',
    '2026-10-24T01:02:03Z',
    record.message
  ].join('\0')).join('\x1e')
}

function mountLog(output: string, request: GitRequest) {
  const client = { request: request as unknown as ThreadClient['request'] } as ThreadClient
  return render(
    <ThreadClientProvider client={client}>
      <WorkspaceGitViews
        threadId="history-files-thread"
        view="log"
        result={{ success: true, output, error: '' }}
        disabled={false}
        onAction={() => undefined}
        onDirtyChange={() => undefined}
        onOpenFile={() => undefined}
      />
    </ThreadClientProvider>
  )
}

function selectCommit(message: string): void {
  const label = screen.getByText(message)
  const row = label.closest('button')
  if (!row) throw new Error('提交记录按钮不存在：' + message)
  fireEvent.click(row)
}

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>(done => { resolve = done })
  return { promise, resolve }
}

describe('WorkspaceGitViews 提交文件历史', () => {
  it('使用结构化文件元数据保留特殊路径、标记并按原路径读取历史全文', async () => {
    const sha = 'a'.repeat(40)
    const firstParent = 'b'.repeat(40)
    const secondParent = 'c'.repeat(40)
    const files: CommitFile[] = [
      { path: 'hello world.txt', kind: 'added', binary: false },
      { path: '说明 文件.md', kind: 'modified', binary: false },
      { path: 'new name.txt', originalPath: 'old name.txt', kind: 'renamed', binary: false },
      { path: 'deleted note.txt', kind: 'deleted', binary: false },
      { path: 'picture asset.bin', kind: 'modified', binary: true },
      { path: '../unsafe.txt', kind: 'modified', binary: false }
    ]
    const request = vi.fn(async (_method: string, params: GitRead): Promise<GitResult> => {
      if (params.action === 'diff_compare') return { success: true, output: 'merge patch preview', error: '', files }
      if (params.action === 'file_at') return { success: true, output: '完整历史文件正文', error: '' }
      throw new Error('意外的 Git 读取动作：' + params.action)
    })
    mountLog(logOutput([{ sha, parents: [firstParent, secondParent], message: '结构化清单提交' }]), request)

    selectCommit('结构化清单提交')
    await waitFor(() => expect(screen.getByTestId('git-diff').textContent).toBe('merge patch preview'))
    expect(request.mock.calls[0]?.[0]).toBe('workbench.git.read')
    expect(request.mock.calls[0]?.[1]).toMatchObject({
      scope: { threadId: 'history-files-thread' },
      action: 'diff_compare',
      target: firstParent + '...' + sha
    })
    expect(screen.getByText('hello world.txt')).toBeTruthy()
    expect(screen.getByText('说明 文件.md')).toBeTruthy()
    expect(screen.getByText(/old name\.txt/)).toBeTruthy()
    expect(screen.getByText(/已重命名|Renamed/i)).toBeTruthy()
    expect(screen.getByText(/^(已删除|Deleted)$/i)).toBeTruthy()
    expect(screen.getByText(/二进制|binary/i)).toBeTruthy()
    expect((screen.getByRole('button', { name: /unsafe\.txt/ }) as HTMLButtonElement).disabled).toBe(true)

    fireEvent.click(screen.getByRole('button', { name: /hello world\.txt/ }))
    await waitFor(() => expect(screen.getByText('完整历史文件正文')).toBeTruthy())
    const fileAtRequest = request.mock.calls.find(([, params]) => params.action === 'file_at')
    expect(fileAtRequest?.[0]).toBe('workbench.git.read')
    expect(fileAtRequest?.[1]).toMatchObject({
      scope: { threadId: 'history-files-thread' },
      action: 'file_at',
      target: sha,
      paths: ['hello world.txt']
    })

    fireEvent.click(screen.getByRole('button', { name: /deleted note\.txt/ }))
    await waitFor(() => expect(screen.getByText(/已删除该文件|deleted this file/i)).toBeTruthy())
    expect(request.mock.calls.filter(([, params]) => params.action === 'file_at')).toHaveLength(1)
    fireEvent.click(screen.getByRole('button', { name: /picture asset\.bin/ }))
    await waitFor(() => expect(screen.getByText(/二进制文件|binary file/i)).toBeTruthy())
    expect(request.mock.calls.filter(([, params]) => params.action === 'file_at')).toHaveLength(1)
    fireEvent.click(screen.getByRole('button', { name: /返回提交差异|Back to commit diff/i }))
    expect(screen.getByTestId('git-diff').textContent).toBe('merge patch preview')
  })

  it('metadata 缺失或读取失败时明确提示，仍显示差异正文', async () => {
    const missingSha = 'd'.repeat(40)
    const failedSha = 'e'.repeat(40)
    const request = vi.fn(async (_method: string, params: GitRead): Promise<GitResult> => params.target === missingSha
      ? { success: true, output: 'patch with missing metadata', error: '' }
      : { success: true, output: 'patch with metadata error', error: '', filesError: 'metadata read failed' })
    mountLog(logOutput([
      { sha: missingSha, parents: ['f'.repeat(40)], message: '元数据缺失提交' },
      { sha: failedSha, parents: ['0'.repeat(40)], message: '元数据失败提交' }
    ]), request)

    selectCommit('元数据缺失提交')
    await waitFor(() => expect(screen.getByRole('status').textContent).toMatch(/暂不可用|Not available temporarily/))
    expect(screen.getByTestId('git-diff').textContent).toBe('patch with missing metadata')

    selectCommit('元数据失败提交')
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('metadata read failed'))
    expect(screen.getByTestId('git-diff').textContent).toBe('patch with metadata error')
  })

  it('较早提交的迟到回执不会覆盖新选中的提交文件和差异', async () => {
    const oldSha = '1'.repeat(40)
    const newSha = '2'.repeat(40)
    const oldReply = deferred<GitResult>()
    const newReply = deferred<GitResult>()
    const request = vi.fn((_method: string, params: GitRead): Promise<GitResult> => params.target === oldSha ? oldReply.promise : newReply.promise)
    mountLog(logOutput([
      { sha: oldSha, parents: ['3'.repeat(40)], message: '较早提交' },
      { sha: newSha, parents: ['4'.repeat(40)], message: '当前提交' }
    ]), request)

    selectCommit('较早提交')
    selectCommit('当前提交')
    await act(async () => {
      newReply.resolve({ success: true, output: '当前提交差异', error: '', files: [{ path: 'current only.txt', kind: 'modified', binary: false }] })
      await newReply.promise
    })
    await waitFor(() => expect(screen.getByTestId('git-diff').textContent).toBe('当前提交差异'))
    expect(screen.getByText('current only.txt')).toBeTruthy()

    await act(async () => {
      oldReply.resolve({ success: true, output: '旧提交迟到差异', error: '', files: [{ path: 'old only.txt', kind: 'modified', binary: false }] })
      await oldReply.promise
    })
    expect(screen.getByTestId('git-diff').textContent).toBe('当前提交差异')
    expect(screen.getByText('current only.txt')).toBeTruthy()
    expect(screen.queryByText('old only.txt')).toBeNull()
  })
})
