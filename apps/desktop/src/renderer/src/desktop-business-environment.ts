import type { BusinessEnvironment } from './business-environment'
import { desktopThreadClient } from './desktop-camp-client'
import { desktopFilePreviewApi } from './desktop-file-preview-api'

export const desktopBusinessEnvironment: BusinessEnvironment = {
  client: desktopThreadClient,
  files: desktopFilePreviewApi,
  get preferences() { return window.coaxi },
  get desktop() { return window.coaxi },
  selectWorkspaceDirectory: () => window.coaxi.selectWorkspaceDirectory(),
  revealProjectDirectory: path => window.coaxi.revealProjectDirectory(path)
}
