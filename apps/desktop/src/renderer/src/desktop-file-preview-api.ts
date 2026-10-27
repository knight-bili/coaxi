import type { FilePreviewApi } from '@contracts'

/** Native file operations stay lazy; explicit browser adapters never enter this path. */
export const desktopFilePreviewApi: FilePreviewApi = {
  nativeReference: input => window.coaxi.filePreview.nativeReference!(input),
  readFileBytes: input => window.coaxi.filePreview.readBinary(input),
  updateRetention: input => window.coaxi.filePreview.updateRetention?.(input) ?? Promise.resolve(),
  onResourcesReleased: listener => window.coaxi.filePreview.onResourcesReleased?.(listener) ?? (() => undefined),
  bindThread: id => window.coaxi.filePreview.bindThread(id),
  open: input => window.coaxi.filePreview.open(input),
  restore: input => window.coaxi.filePreview.restore(input),
  reopen: input => window.coaxi.filePreview.reopen(input),
  readText: input => window.coaxi.filePreview.readText(input),
  readPage: input => window.coaxi.filePreview.readPage(input),
  resolveLine: input => window.coaxi.filePreview.resolveLine(input),
  readBinary: input => window.coaxi.filePreview.readBinary(input),
  prepareHtmlSite: input => window.coaxi.filePreview.prepareHtmlSite(input),
  releaseHtmlSite: input => window.coaxi.filePreview.releaseHtmlSite(input),
  prepareHtml: input => window.coaxi.filePreview.prepareHtml(input),
  reload: input => window.coaxi.filePreview.reload(input),
  release: input => window.coaxi.filePreview.release(input),
  openInSystem: input => window.coaxi.filePreview.openInSystem(input),
  revealInFolder: input => window.coaxi.filePreview.revealInFolder(input),
  copyPath: input => window.coaxi.filePreview.copyPath(input),
  chooseAuthorizedRoot: input => window.coaxi.filePreview.chooseAuthorizedRoot(input),
  onExternalUpdate: listener => window.coaxi.filePreview.onExternalUpdate(listener)
}
