import type { Settings } from '@/services/bindings'
import {
  enhanceProfiles,
  getSubscriptionGroups,
  saveSubscriptionGroups,
  rescanSubscriptionGroup,
} from '@/services/cmds'
import { mutate } from '@/services/mutate'

export { getSubscriptionGroups }

export const saveAndApplyGroups = (settings: Settings) =>
  mutate(
    async () => {
      await saveSubscriptionGroups(settings)
      if (!(await enhanceProfiles())) {
        throw new Error(
          'Configuration saved, but runtime activation failed. Review the error and apply again.',
        )
      }
    },
    { id: 'subscription-groups', revalidate: [['subscription-groups']] },
  )

export const rescanGroup = (id: string) =>
  mutate(() => rescanSubscriptionGroup(id), {
    id: `subscription-group-scan:${id}`,
  })
