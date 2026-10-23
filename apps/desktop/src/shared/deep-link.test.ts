import {describe, expect, it} from 'vitest'
import {parseDesktopDeepLink} from './deep-link'

const thread = 'rvcamp_01h47kvsy5fk1shh6w1g60eecf'
const mission = 'rvm_0123456789abcdef0123456789abcdef'
const uuid = '01234567-89ab-4cde-8fab-0123456789ab'

describe('桌面导航深链', () => {
  it.each([
    ['thread', thread], ['mission', mission], ['mission', uuid], ['canvas', uuid],
    ['settings', 'appearance'], ['tab', 'canvas']
  ])('按真实标识接收 %s', (kind, value) => {
    expect(parseDesktopDeepLink(`rovai://${kind}/${value}`)).toEqual({kind, value})
  })

  it.each([
    `rovai://thread/${uuid}`, `rovai://mission/${thread}`, `rovai://canvas/${mission}`,
    `rovai://user:secret@thread/${thread}`, `rovai://thread:80/${thread}`,
    `rovai://thread/${thread}?host=remote`, `rovai://thread/${thread}#run`,
    `rovai://thread/${thread}/extra`, `rovai://thread/${thread}%2fextra`,
    'rovai://settings/%0a', 'rovai://settings/%zz', 'rovai://settings/unknown',
    `https://thread/${thread}`, ` rovai://thread/${thread}`,
    `rovai://thread/${'a'.repeat(2048)}`
  ])('拒绝无效标识或夹带输入 %s', raw => {
    expect(parseDesktopDeepLink(raw)).toBeNull()
  })
})
