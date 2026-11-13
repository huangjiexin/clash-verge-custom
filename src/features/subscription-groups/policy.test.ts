import { expect, test } from 'vitest'

import type { ResolvedMemberOccurrence } from '@/components/proxy/use-render-list'

import { newerGroupMeasurement } from './history'
import { applyGroupPolicy } from './policy'

const node = (name: string, delay: number) =>
  ({
    memberIndex: delay,
    member: { kind: 'node', ref: { kind: 'node', name, recordId: name } },
  }) as ResolvedMemberOccurrence

test('arbitrary groups hide failed nodes, sort latency and keep auto first', () => {
  const auto = node('Company auto', 0)
  const slow = node('slow', 500)
  const fast = node('fast', 50)
  const failed = node('failed', 1e6)
  const unknown = node('unknown', -1)
  const input = [slow, failed, unknown, auto, fast]
  expect(
    applyGroupPolicy(
      input,
      { sort: 'delay', hideFailed: true, automaticName: 'Company auto' },
      (item) => item.memberIndex,
    ),
  ).toEqual([auto, fast, slow, unknown])
  expect(
    applyGroupPolicy(
      input,
      { sort: 'original', hideFailed: false, automaticName: null },
      (item) => item.memberIndex,
    ),
  ).toEqual(input)
  expect(
    applyGroupPolicy(
      [node('REJECT', 0)],
      { sort: 'delay', hideFailed: true, automaticName: null },
      (item) => item.memberIndex,
    ),
  ).toHaveLength(1)
})

test('recovered group node supersedes cached error without affecting upstream nodes', () => {
  const sample = { time: new Date(2000).toISOString(), delay: 50 }
  const error = { delay: 1e6, updatedAt: 1000 }
  expect(newerGroupMeasurement('SG/group/node', sample, error)).toBe(50)
  expect(newerGroupMeasurement('ordinary node', sample, error)).toBeUndefined()
  expect(
    newerGroupMeasurement('SG/group/node', sample, {
      delay: -2,
      updatedAt: 1000,
    }),
  ).toBeUndefined()
})
