// Streamdown 2.2 memoizes containers by source position, ignoring changed
// children. A late math plugin therefore needs an inert class revision on the
// affected ancestors. Element types/keys and source positions stay unchanged,
// preserving interactive card state and the original DOM/style composition.
import type { ComponentProps } from 'react'
import type { Streamdown } from 'streamdown'
type MathPlugin = NonNullable<NonNullable<ComponentProps<typeof Streamdown>['plugins']>['math']>
type Node = { type: string; properties?: Record<string, unknown>; children?: Node[] }
let revision = 0
function refresh(node: Node, token: string): boolean {
  const classes = node.properties?.className
  const names = Array.isArray(classes) ? classes : typeof classes === 'string' ? classes.split(/\s+/) : []
  let containsMath = names.includes('katex') || names.includes('katex-error')
  for (const child of node.children ?? []) if (refresh(child, token)) containsMath = true
  if (containsMath && node.type === 'element') node.properties = { ...node.properties, className: [...names, token] }
  return containsMath
}
export function hydrateMathContainers(declared: MathPlugin['rehypePlugin']): MathPlugin['rehypePlugin'] {
  const [plugin, options] = (Array.isArray(declared) ? declared : [declared, undefined]) as [
    (this: unknown, options: unknown) => (tree: Node, file: unknown) => Node | void, unknown
  ]
  function rehypeMathWithContainerUpdates(this: unknown) {
    const transform = plugin.call(this, options)
    return (tree: Node, file: unknown) => {
      const result = transform(tree, file)
      refresh(result ?? tree, `coaxi-math-pass-${++revision}`)
      return result
    }
  }
  return rehypeMathWithContainerUpdates as MathPlugin['rehypePlugin']
}
