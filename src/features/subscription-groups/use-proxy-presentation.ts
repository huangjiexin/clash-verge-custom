import { useMemo } from 'react'

import { useProxiesData } from '@/providers/app-data-context'

import { createProxyPresentation } from './presentation'

export function useProxyPresentation() {
  const { proxyView } = useProxiesData()
  return useMemo(() => createProxyPresentation(proxyView), [proxyView])
}
