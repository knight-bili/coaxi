import { translateUi, useInterfaceLanguage } from './interface-language'

// Contextual labels for the current Thread header and detail panels.
const campDetailCopy = {
  tasks: '任务',
  members: '队员',
  singleChat: '单聊',
  singleChatTitle: '单聊'
} as const

export function useThreadDetailCopy(): typeof campDetailCopy & { execution: string } {
  const language = useInterfaceLanguage()
  return {
    tasks: translateUi(language, campDetailCopy.tasks),
    members: translateUi(language, campDetailCopy.members),
    singleChat: translateUi(language, campDetailCopy.singleChat),
    singleChatTitle: translateUi(language, campDetailCopy.singleChatTitle),
    execution: translateUi(language, '执行')
  }
}
