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
    expect(parseDesktopDeepLink(`coaxi://${kind}/${value}`)).toEqual({kind, value})
  })

  it.each([
    `coaxi://thread/${uuid}`, `coaxi://mission/${thread}`, `coaxi://canvas/${mission}`,
    `coaxi://user:secret@thread/${thread}`, `coaxi://thread:80/${thread}`,
    `coaxi://thread/${thread}?host=remote`, `coaxi://thread/${thread}#run`,
    `coaxi://thread/${thread}/extra`, `coaxi://thread/${thread}%2fextra`,
    'coaxi://settings/%0a', 'coaxi://settings/%zz', 'coaxi://settings/unknown',
    `https://thread/${thread}`, ` coaxi://thread/${thread}`,
    `coaxi://thread/${'a'.repeat(2048)}`
  ])('拒绝无效标识或夹带输入 %s', raw => {
    expect(parseDesktopDeepLink(raw)).toBeNull()
  })
})
