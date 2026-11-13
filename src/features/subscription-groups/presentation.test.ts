import { expect, test } from 'vitest'

import type { ProxyViewV1 } from '@/types/proxy-view'

import { createProxyPresentation } from './presentation'

const first = 'SG/one/Source/with slash/source-a/HK/01'
const second = 'SG/two/Other/source-b/HK/01'
const view = {
  subscriptionLabels: {
    [first]: { name: 'HK/01', source: 'Source/with slash' },
    [second]: { name: 'HK/01', source: 'Other' },
  },
  groups: [
    {
      name: 'Codex 专用',
      subscriptionGroup: { automaticName: 'Codex自动选择' },
    },
    {
      name: 'Codex自动选择',
      subscriptionGroup: { automaticName: 'Codex自动选择' },
    },
    {
      name: 'Claude',
      subscriptionGroup: { automaticName: 'Claude · 自动选择' },
    },
  ],
} as unknown as ProxyViewV1

test('display labels preserve slashes and distinguish duplicate nodes without changing identities', () => {
  const before = JSON.stringify(view)
  const display = createProxyPresentation(view)
  expect(display.label(first)).toEqual({
    name: 'HK/01',
    source: 'Source/with slash',
  })
  expect(display.summary(first)).toBe('HK/01 · Source/with slash')
  expect(display.summary(second)).toBe('HK/01 · Other')
  expect(JSON.stringify(view)).toBe(before)
})

test('automatic labels follow the parent group and stay unambiguous outside it', () => {
  const display = createProxyPresentation(view)
  expect(display.summary('Codex自动选择', 'Codex 专用')).toBe('自动选择')
  expect(display.summary('Codex自动选择')).toBe('Codex 专用 · 自动选择')
  expect(display.summary('Claude · 自动选择', 'Claude')).toBe('自动选择')
  expect(display.summary('Claude · 自动选择', 'Codex 专用')).toBe(
    'Claude · 自动选择',
  )
})

test('ordinary and unrecognized names are never parsed or rewritten', () => {
  const display = createProxyPresentation(view)
  expect(display.summary('自动选择')).toBe('自动选择')
  expect(display.summary('SG/not/an/imported/node')).toBe(
    'SG/not/an/imported/node',
  )
  expect(createProxyPresentation(undefined).summary(first)).toBe(first)
})
