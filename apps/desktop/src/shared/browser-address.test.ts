import {expect,it} from 'vitest'
import {resolveBrowserAddress} from './browser-address'

it.each([
  ['bbs.125.la','https://bbs.125.la/'],
  ['example.com/a?x=1#next','https://example.com/a?x=1#next'],
  [' https://example.com ','https://example.com/'],
  ['localhost:3000','http://localhost:3000/'],
  ['127.0.0.1:4173/demo','http://127.0.0.1:4173/demo'],
  ['[::1]:8080','http://[::1]:8080/'],
  ['明朝','https://www.baidu.com/s?wd=%E6%98%8E%E6%9C%9D'],
  ['hello world','https://www.baidu.com/s?wd=hello%20world'],
  ['','' ],
])('解析地址栏 %s',(value,expected)=>expect(resolveBrowserAddress(value)).toBe(expected||null))
it.each(['javascript:alert(1)','file:///C:/data','data:text/html,hello','https://user:password@example.com'])('拒绝非网页协议与 URL 凭据 %s',value=>expect(()=>resolveBrowserAddress(value)).toThrow())
