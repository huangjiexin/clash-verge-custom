export function newerGroupMeasurement(
  name: string,
  latest: { time: string; delay: number } | undefined,
  cached: { updatedAt: number; delay: number } | undefined,
): number | undefined {
  if (
    name.startsWith('SG/') &&
    latest &&
    cached?.delay !== -2 &&
    (!cached || Date.parse(latest.time) > cached.updatedAt)
  ) {
    return latest.delay || 1e6
  }
  return undefined
}
