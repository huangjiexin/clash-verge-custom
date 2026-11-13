import {
  AddRounded,
  DeleteOutlineRounded,
  MoreHorizRounded,
  SaveOutlined,
} from '@mui/icons-material'
import {
  Alert,
  Box,
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
  FormControlLabel,
  IconButton,
  List,
  ListItemButton,
  ListItemText,
  Menu,
  MenuItem,
  Paper,
  Stack,
  Switch,
  Tab,
  Tabs,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material'
import { useState } from 'react'

import { BasePage, MonacoEditor } from '@/components/base'
import { useProfiles } from '@/hooks/use-profiles'
import ProfilePage from '@/pages/profiles'
import type { Group, Settings } from '@/services/bindings'
import { errorDetail } from '@/services/notice-service'
import { useQuery } from '@/services/query-client'
import { useThemeMode } from '@/services/states'

import { GroupHelpDialog } from './help-dialog'
import { ResultsPanel } from './results-panel'
import {
  getSubscriptionGroups,
  rescanGroup,
  saveAndApplyGroups,
} from './service'
import { templates } from './templates'

const tabsSx = {
  minHeight: 42,
  '& .MuiTab-root': {
    minHeight: 42,
    minWidth: 78,
    px: 1.5,
    fontSize: 16,
    textTransform: 'none',
  },
  '& .MuiTabs-indicator': { height: 3, borderRadius: '3px 3px 0 0' },
}
const scrollSx = {
  minHeight: 0,
  overflowY: 'auto',
  overscrollBehavior: 'contain',
  scrollbarGutter: 'stable',
} as const

export default function SubscriptionPage() {
  const [tab, setTab] = useState(0)
  const [dirty, setDirty] = useState(false)
  const [leaveOpen, setLeaveOpen] = useState(false)
  const tabs = (
    <Tabs
      value={tab}
      onChange={(_, next: number) => {
        if (next === 0 && dirty) setLeaveOpen(true)
        else setTab(next)
      }}
      aria-label="订阅管理"
      sx={tabsSx}
    >
      <Tab label="订阅" />
      <Tab label="订阅组" />
    </Tabs>
  )
  return (
    <>
      {tab === 0 ? (
        <ProfilePage titleOverride={tabs} />
      ) : (
        <GroupsPage title={tabs} onDirty={setDirty} />
      )}
      <Dialog open={leaveOpen} onClose={() => setLeaveOpen(false)}>
        <DialogTitle>放弃未保存的修改？</DialogTitle>
        <DialogContent>切换到订阅页面将丢弃当前草稿。</DialogContent>
        <DialogActions>
          <Button onClick={() => setLeaveOpen(false)}>继续编辑</Button>
          <Button
            color="error"
            onClick={() => {
              setLeaveOpen(false)
              setDirty(false)
              setTab(0)
            }}
          >
            放弃修改
          </Button>
        </DialogActions>
      </Dialog>
    </>
  )
}

function GroupsPage({
  title,
  onDirty,
}: {
  title: React.ReactNode
  onDirty: (dirty: boolean) => void
}) {
  const { profiles } = useProfiles()
  const themeMode = useThemeMode()
  const { data, error, refetch } = useQuery({
    queryKey: ['subscription-groups'],
    queryFn: getSubscriptionGroups,
    refetchInterval: 3000,
  })
  const [draft, setDraft] = useState<Settings | null>(null)
  const [selected, setSelected] = useState<string | null>(null)
  const [section, setSection] = useState(0)
  const [busy, setBusy] = useState(false)
  const [feedback, setFeedback] = useState<{
    error: boolean
    message: string
  } | null>(null)
  const [sourceSearch, setSourceSearch] = useState('')
  const [templateAnchor, setTemplateAnchor] = useState<HTMLElement | null>(null)
  const [templateKey, setTemplateKey] = useState<keyof typeof templates | null>(
    null,
  )
  const [deleteOpen, setDeleteOpen] = useState(false)
  const [helpOpen, setHelpOpen] = useState(false)
  const settings = draft ?? data?.settings ?? { groups: [] }
  const group =
    settings.groups.find((item) => item.id === selected) ?? settings.groups[0]
  const report = data?.reports.find((item) => item.groupId === group?.id)
  const sources = (profiles?.items ?? []).filter(
    (item) => item.type === 'remote' || item.type === 'local',
  )
  const edit = (settings: Settings) => {
    setDraft(settings)
    onDirty(true)
    setFeedback(null)
  }
  const patch = (value: Partial<Group>) =>
    edit({
      groups: settings.groups.map((item) =>
        item.id === group?.id ? { ...item, ...value } : item,
      ),
    })
  const add = () => {
    const id = crypto.randomUUID()
    edit({
      groups: [
        ...settings.groups,
        {
          id,
          name: `新订阅组 ${settings.groups.length + 1}`,
          enabled: true,
          sources: [],
          script: templates.all.script,
        },
      ],
    })
    setSelected(id)
    setSection(0)
  }
  const save = async () => {
    setBusy(true)
    try {
      await saveAndApplyGroups(settings)
      await refetch()
      setDraft(null)
      onDirty(false)
      setFeedback({ error: false, message: '已保存并应用' })
    } catch (error) {
      setFeedback({ error: true, message: errorDetail(error) })
    } finally {
      setBusy(false)
    }
  }
  const sourceLabel = (source: IProfileItem) => {
    if (source.url) {
      try {
        return new URL(source.url).host
      } catch {
        /* Display the local source type instead. */
      }
    }
    return source.type === 'local' ? '本地订阅' : '远程订阅'
  }
  return (
    <Box
      sx={{
        height: '100%',
        minHeight: 0,
        '& .base-container': { flex: '1 1 0', minHeight: 0, height: 'auto' },
      }}
    >
      <BasePage
        full
        title={title}
        contentStyle={{ height: '100%', minHeight: 0, overflow: 'hidden' }}
        header={
          <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
            {draft && (
              <Typography
                variant="caption"
                color="text.secondary"
                sx={{ whiteSpace: 'nowrap' }}
              >
                未保存
              </Typography>
            )}
            <Button
              size="small"
              variant="contained"
              startIcon={<SaveOutlined />}
              onClick={() => void save()}
              disabled={!draft || busy}
              sx={{ whiteSpace: 'nowrap', textTransform: 'none' }}
            >
              保存并应用
            </Button>
          </Stack>
        }
      >
        <Box
          sx={{
            display: 'flex',
            height: '100%',
            minHeight: 0,
            p: 1.5,
            gap: 1.5,
            boxSizing: 'border-box',
            '@media (max-width: 820px)': { flexDirection: 'column' },
          }}
        >
          <Paper
            variant="outlined"
            sx={{
              width: 190,
              flexShrink: 0,
              display: 'flex',
              flexDirection: 'column',
              minHeight: 0,
              overflow: 'hidden',
              borderRadius: 2,
              '@media (max-width: 820px)': { width: '100%', maxHeight: 145 },
            }}
          >
            <Stack
              direction="row"
              sx={{
                alignItems: 'center',
                justifyContent: 'space-between',
                px: 1.5,
                py: 1,
                flexShrink: 0,
              }}
            >
              <Typography variant="body2" sx={{ fontWeight: 600 }}>
                我的订阅组 · {settings.groups.length}
              </Typography>
              <Tooltip title="新建订阅组">
                <span>
                  <IconButton
                    size="small"
                    aria-label="新建订阅组"
                    onClick={add}
                    disabled={!data || busy}
                  >
                    <AddRounded fontSize="small" />
                  </IconButton>
                </span>
              </Tooltip>
            </Stack>
            <Divider />
            <List
              aria-label="订阅组列表"
              sx={{ ...scrollSx, flex: 1, p: 0.75 }}
            >
              {settings.groups.map((item) => (
                <ListItemButton
                  key={item.id}
                  selected={item.id === group?.id}
                  onClick={() => {
                    setSelected(item.id)
                    setSourceSearch('')
                  }}
                  sx={{ borderRadius: 1.5, mb: 0.5, px: 1.25 }}
                >
                  <ListItemText
                    primary={item.name}
                    secondary={`${item.sources.length} 个订阅 · ${item.enabled ? '已启用' : '已停用'}`}
                    slotProps={{
                      primary: {
                        noWrap: true,
                        sx: {
                          fontSize: 14,
                          fontWeight: 600,
                          textTransform: 'none',
                        },
                      },
                      secondary: { sx: { fontSize: 12, mt: 0.5 } },
                    }}
                  />
                </ListItemButton>
              ))}
              {!settings.groups.length && (
                <Typography
                  variant="body2"
                  color="text.secondary"
                  sx={{ p: 1.5 }}
                >
                  尚无订阅组
                </Typography>
              )}
            </List>
            <Divider />
            <Button
              size="small"
              onClick={() => setHelpOpen(true)}
              sx={{ m: 0.5, textTransform: 'none' }}
            >
              使用说明与配置位置
            </Button>
          </Paper>
          <Paper
            variant="outlined"
            sx={{
              flex: 1,
              minWidth: 0,
              minHeight: 0,
              display: 'flex',
              flexDirection: 'column',
              borderRadius: 2,
              overflow: 'hidden',
            }}
          >
            {(error || feedback) && (
              <Alert
                severity={error || feedback?.error ? 'error' : 'success'}
                onClose={() => setFeedback(null)}
                sx={{ borderRadius: 0, flexShrink: 0 }}
              >
                {error ? errorDetail(error) : feedback?.message}
              </Alert>
            )}
            {!group ? (
              <Stack
                spacing={1.5}
                sx={{
                  alignItems: 'center',
                  justifyContent: 'center',
                  height: '100%',
                  p: 3,
                }}
              >
                <Typography sx={{ fontWeight: 600 }}>
                  组合订阅，按用途管理节点
                </Typography>
                <Typography variant="body2" color="text.secondary">
                  选择来源订阅，为每个组配置独立脚本。
                </Typography>
                <Button
                  variant="contained"
                  startIcon={<AddRounded />}
                  onClick={add}
                  disabled={!data}
                >
                  新建订阅组
                </Button>
              </Stack>
            ) : (
              <>
                <Stack
                  direction="row"
                  sx={{
                    px: 2,
                    py: 1.5,
                    gap: 1,
                    alignItems: 'center',
                    flexShrink: 0,
                  }}
                >
                  <Box sx={{ flex: 1, minWidth: 0 }}>
                    <Typography noWrap sx={{ fontSize: 17, fontWeight: 600 }}>
                      {group.name}
                    </Typography>
                    <Typography variant="caption" color="text.secondary">
                      {group.sources.length} 个来源 · 独立脚本
                    </Typography>
                  </Box>
                  <FormControlLabel
                    sx={{
                      m: 0,
                      flexShrink: 0,
                      '& .MuiFormControlLabel-label': {
                        fontSize: 13,
                        whiteSpace: 'nowrap',
                      },
                    }}
                    control={
                      <Switch
                        size="small"
                        checked={group.enabled}
                        onChange={(_, enabled) => patch({ enabled })}
                        disabled={busy}
                      />
                    }
                    label={group.enabled ? '启用' : '停用'}
                  />
                  <Tooltip title="删除订阅组">
                    <IconButton
                      size="small"
                      aria-label="删除订阅组"
                      onClick={() => setDeleteOpen(true)}
                      disabled={busy}
                    >
                      <DeleteOutlineRounded fontSize="small" />
                    </IconButton>
                  </Tooltip>
                </Stack>
                <Tabs
                  value={section}
                  onChange={(_, value: number) => setSection(value)}
                  aria-label="订阅组详情"
                  sx={{
                    px: 1,
                    minHeight: 40,
                    borderBottom: 1,
                    borderColor: 'divider',
                    flexShrink: 0,
                    '& .MuiTab-root': {
                      minHeight: 40,
                      minWidth: 82,
                      fontSize: 13,
                    },
                  }}
                >
                  <Tab label="订阅来源" />
                  <Tab label="逻辑脚本" />
                  <Tab label="检测结果" />
                </Tabs>
                {section === 0 && (
                  <Box
                    role="tabpanel"
                    aria-label="订阅来源"
                    sx={{ ...scrollSx, flex: 1, p: 2 }}
                  >
                    <TextField
                      size="small"
                      label="组名"
                      value={group.name}
                      onChange={(e) => patch({ name: e.target.value })}
                      fullWidth
                      disabled={busy}
                    />
                    <Stack
                      direction="row"
                      sx={{
                        mt: 3,
                        mb: 1.5,
                        alignItems: 'center',
                        justifyContent: 'space-between',
                        gap: 1,
                      }}
                    >
                      <Typography
                        variant="body2"
                        sx={{ fontWeight: 600, whiteSpace: 'nowrap' }}
                      >
                        来源订阅{' '}
                        <Box
                          component="span"
                          sx={{ color: 'text.secondary', fontWeight: 400 }}
                        >
                          已选 {group.sources.length}
                        </Box>
                      </Typography>
                      <TextField
                        size="small"
                        placeholder="搜索订阅"
                        value={sourceSearch}
                        onChange={(e) => setSourceSearch(e.target.value)}
                        slotProps={{ htmlInput: { 'aria-label': '搜索订阅' } }}
                        sx={{ width: 180 }}
                      />
                    </Stack>
                    <Stack spacing={1}>
                      {sources
                        .filter((source) =>
                          `${source.name} ${sourceLabel(source)}`
                            .toLowerCase()
                            .includes(sourceSearch.toLowerCase()),
                        )
                        .map((source) => {
                          const checked = group.sources.includes(source.uid!)
                          return (
                            <Paper
                              variant="outlined"
                              key={source.uid}
                              sx={{
                                borderColor: checked
                                  ? 'primary.main'
                                  : 'divider',
                                borderRadius: 1.5,
                              }}
                            >
                              <FormControlLabel
                                sx={{
                                  m: 0,
                                  width: '100%',
                                  p: 0.5,
                                  boxSizing: 'border-box',
                                  '& .MuiFormControlLabel-label': {
                                    minWidth: 0,
                                    flex: 1,
                                  },
                                }}
                                control={
                                  <Checkbox
                                    checked={checked}
                                    disabled={busy}
                                    onChange={(_, value) =>
                                      patch({
                                        sources: value
                                          ? [...group.sources, source.uid!]
                                          : group.sources.filter(
                                              (id) => id !== source.uid,
                                            ),
                                      })
                                    }
                                  />
                                }
                                label={
                                  <Box sx={{ pr: 1 }}>
                                    <Typography
                                      variant="body2"
                                      noWrap
                                      sx={{ fontWeight: 500 }}
                                    >
                                      {source.name || source.uid}
                                    </Typography>
                                    <Typography
                                      variant="caption"
                                      noWrap
                                      color="text.secondary"
                                    >
                                      {sourceLabel(source)}
                                    </Typography>
                                  </Box>
                                }
                              />
                            </Paper>
                          )
                        })}
                    </Stack>
                    {group.sources
                      .filter(
                        (id) => !sources.some((source) => source.uid === id),
                      )
                      .map((id) => (
                        <Alert
                          sx={{ mt: 1 }}
                          key={id}
                          severity="error"
                          action={
                            <Button
                              onClick={() =>
                                patch({
                                  sources: group.sources.filter(
                                    (source) => source !== id,
                                  ),
                                })
                              }
                            >
                              移除
                            </Button>
                          }
                        >
                          来源已删除：{id}
                        </Alert>
                      ))}
                    <Typography
                      variant="caption"
                      color="text.secondary"
                      sx={{ display: 'block', mt: 2, lineHeight: 1.7 }}
                    >
                      来源可被多个组复用，节点随订阅更新。流量分配请在规则中选择对应组。
                    </Typography>
                  </Box>
                )}
                {section === 1 && (
                  <Box
                    role="tabpanel"
                    aria-label="逻辑脚本"
                    sx={{
                      flex: 1,
                      minHeight: 0,
                      display: 'flex',
                      flexDirection: 'column',
                    }}
                  >
                    <Stack
                      direction="row"
                      sx={{
                        px: 2,
                        py: 1,
                        alignItems: 'center',
                        justifyContent: 'space-between',
                        flexShrink: 0,
                      }}
                    >
                      <Typography variant="caption" color="text.secondary">
                        JavaScript · main(config)
                      </Typography>
                      <Stack direction="row" spacing={1}>
                        <Button size="small" onClick={() => setHelpOpen(true)}>
                          脚本说明
                        </Button>
                        <Button
                          size="small"
                          endIcon={<MoreHorizRounded />}
                          onClick={(e) => setTemplateAnchor(e.currentTarget)}
                          disabled={busy}
                        >
                          使用模板
                        </Button>
                      </Stack>
                    </Stack>
                    <Box sx={{ flex: 1, minHeight: 0, overflow: 'hidden' }}>
                      <MonacoEditor
                        height="100%"
                        path={`subscription-group:${group.id}.js`}
                        value={group.script}
                        language="javascript"
                        theme={themeMode === 'light' ? 'light' : 'vs-dark'}
                        saveViewState
                        onChange={(value) => patch({ script: value ?? '' })}
                        options={{
                          automaticLayout: true,
                          minimap: { enabled: false },
                          fontSize: 13,
                          lineHeight: 21,
                          tabSize: 2,
                          wordWrap: 'on',
                          scrollBeyondLastLine: false,
                          padding: { top: 12, bottom: 12 },
                          readOnly: busy,
                          scrollbar: { alwaysConsumeMouseWheel: true },
                          ariaLabel: '本组逻辑脚本',
                        }}
                      />
                    </Box>
                    <Typography
                      variant="caption"
                      color="text.secondary"
                      sx={{
                        px: 2,
                        py: 1,
                        borderTop: 1,
                        borderColor: 'divider',
                        flexShrink: 0,
                      }}
                    >
                      修改脚本后，点击右上角「保存并应用」。
                    </Typography>
                  </Box>
                )}
                {section === 2 && (
                  <ResultsPanel
                    key={group.id}
                    report={report}
                    disabled={busy || !!draft || !group.enabled}
                    onScan={() =>
                      void rescanGroup(group.id)
                        .then(() => refetch())
                        .catch((error) =>
                          setFeedback({
                            error: true,
                            message: errorDetail(error),
                          }),
                        )
                    }
                  />
                )}
              </>
            )}
          </Paper>
        </Box>
      </BasePage>
      <Menu
        anchorEl={templateAnchor}
        open={!!templateAnchor}
        onClose={() => setTemplateAnchor(null)}
      >
        {(Object.keys(templates) as Array<keyof typeof templates>).map(
          (key) => (
            <MenuItem
              key={key}
              onClick={() => {
                setTemplateAnchor(null)
                setTemplateKey(key)
              }}
            >
              <ListItemText
                primary={templates[key].label}
                secondary={templates[key].description}
              />
            </MenuItem>
          ),
        )}
      </Menu>
      <Dialog open={!!templateKey} onClose={() => setTemplateKey(null)}>
        <DialogTitle>替换当前脚本？</DialogTitle>
        <DialogContent>
          所选模板将替换本组的脚本，保存并应用后生效。网络检测模板中的 .example
          网址需要替换为实际检测目标，并填写本机出口网卡。
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setTemplateKey(null)}>取消</Button>
          <Button
            onClick={() => {
              if (templateKey) patch({ script: templates[templateKey].script })
              setTemplateKey(null)
            }}
          >
            替换脚本
          </Button>
        </DialogActions>
      </Dialog>
      <Dialog open={deleteOpen} onClose={() => setDeleteOpen(false)}>
        <DialogTitle>删除「{group?.name}」？</DialogTitle>
        <DialogContent>
          仅移除这个组合，来源订阅仍然保留。保存并应用后生效。
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setDeleteOpen(false)}>取消</Button>
          <Button
            color="error"
            onClick={() => {
              edit({
                groups: settings.groups.filter((item) => item.id !== group?.id),
              })
              setSelected(null)
              setDeleteOpen(false)
            }}
          >
            删除组
          </Button>
        </DialogActions>
      </Dialog>
      <GroupHelpDialog
        open={helpOpen}
        onClose={() => setHelpOpen(false)}
        configPath={data?.config_path}
      />
    </Box>
  )
}
