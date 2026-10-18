import { expect, it } from 'vitest'
import { prepareDelegationViewerLeave } from './DelegationViewer'

it('后层准备失败会回滚已准备的输入；再次准备成功后按逆序且只完成一次', () => {
  const events: string[] = []
  let blocked = true
  const leaf = () => { events.push('leaf:prepare'); return { complete: (left: boolean) => { events.push('leaf:' + left) } } }
  const nested = () => { if (blocked) throw Error('nested input pending'); events.push('nested:prepare'); return { complete: (left: boolean) => { events.push('nested:' + left) } } }
  expect(() => prepareDelegationViewerLeave(leaf, nested)).toThrow('nested input pending')
  expect(events).toEqual(['leaf:prepare', 'leaf:false'])
  events.length = 0; blocked = false
  const prepared = prepareDelegationViewerLeave(leaf, null, nested)
  prepared.complete(true); prepared.complete(false)
  expect(events).toEqual(['leaf:prepare', 'nested:prepare', 'nested:true', 'leaf:true'])
})

it('取消页面切换会释放全部准备项，不留下已离开的状态', () => {
  let leases = 0
  const guard = () => { leases++; return { complete: () => { leases-- } } }
  const prepared = prepareDelegationViewerLeave(guard, guard)
  expect(leases).toBe(2)
  prepared.complete(false); prepared.complete(false)
  expect(leases).toBe(0)
  prepareDelegationViewerLeave(guard).complete(false)
  expect(leases).toBe(0)
})
