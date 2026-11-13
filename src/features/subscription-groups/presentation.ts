import type { ProxyViewV1 } from '@/types/proxy-view'

export function createProxyPresentation(view: ProxyViewV1 | null | undefined) {
  const automatic = new Map(
    (view?.groups ?? []).flatMap((group) => {
      const name = group.subscriptionGroup?.automaticName
      return name && name !== group.name ? [[name, group.name] as const] : []
    }),
  )
  const label = (name: string, owner?: string) => {
    const parent = automatic.get(name)
    if (parent) {
      return {
        name: owner === parent ? '自动选择' : `${parent} · 自动选择`,
        source: '',
      }
    }
    return view?.subscriptionLabels?.[name] ?? { name, source: '' }
  }
  const summary = (name: string, owner?: string) => {
    const item = label(name, owner)
    return item.source ? `${item.name} · ${item.source}` : item.name
  }
  return { label, summary }
}
