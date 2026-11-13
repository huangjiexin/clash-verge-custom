import type { ResolvedMemberOccurrence } from '@/components/proxy/use-render-list'
import type { GroupPolicy } from '@/services/bindings'
import {
  classifyDelay,
  compareByDelay,
  DEFAULT_DELAY_TIMEOUT,
} from '@/utils/delay'

export function applyGroupPolicy(
  members: ResolvedMemberOccurrence[],
  policy: GroupPolicy,
  delayOf: (member: ResolvedMemberOccurrence) => number,
  timeout = DEFAULT_DELAY_TIMEOUT,
) {
  const effectiveTimeout = timeout > 0 ? timeout : DEFAULT_DELAY_TIMEOUT
  const automatic = members.filter(
    (item) => item.member.ref.name === policy.automaticName,
  )
  const nodes = members
    .filter((item) => item.member.ref.name !== policy.automaticName)
    .map((item) => ({ item, delay: delayOf(item) }))
    .filter(({ item, delay }) => {
      if (!policy.hideFailed || item.member.ref.name === 'REJECT') return true
      const state = classifyDelay(delay, effectiveTimeout)
      return state !== 'error' && state !== 'timeout'
    })
  if (policy.sort === 'delay')
    nodes.sort((a, b) => compareByDelay(a.delay, b.delay, effectiveTimeout))
  if (policy.sort === 'name')
    nodes.sort((a, b) =>
      a.item.member.ref.name.localeCompare(b.item.member.ref.name),
    )
  return [...automatic, ...nodes.map(({ item }) => item)]
}
