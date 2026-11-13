import {
  Alert,
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Stack,
  Tab,
  Tabs,
  Typography,
} from '@mui/material'
import { useState } from 'react'

const fields = [
  [
    'concurrency',
    '同时检测的节点数，默认 16，可设置 1–32。每个节点的多个目标按顺序验证；较高并发可缩短整轮等待时间，也会增加网络占用。',
  ],
  [
    'checks',
    '检测目标列表。每项设置 url、expected-status 和 validation；所有目标通过后才接纳节点。空列表只运行节点筛选，不检测连通性。',
  ],
  [
    'interval',
    '完整扫描的间隔，单位为秒，范围 60–3600。上一轮结束后开始计时；下次扫描会重新读取来源节点和脚本。',
  ],
  [
    'interface-name',
    '网络检测时填写本机实际出口网卡名称，避免检测流量被虚拟网卡再次接管；仅合并或筛选节点时可留空。网卡名称因设备而异。',
  ],
  ['sort', 'delay：按检测延迟；name：按名称；original：按脚本返回的顺序。'],
  [
    'automatic',
    '启用后在组内加入“自动选择”。至少需要一个检测目标；通过首个目标定时测量并切换节点，完整扫描继续验证所有条件。',
  ],
] as const

export function GroupHelpDialog({
  open,
  onClose,
  configPath,
}: {
  open: boolean
  onClose: () => void
  configPath?: string
}) {
  const [tab, setTab] = useState(0)
  return (
    <Dialog
      open={open}
      onClose={onClose}
      maxWidth="sm"
      fullWidth
      scroll="paper"
    >
      <DialogTitle>订阅组使用指南</DialogTitle>
      <Tabs
        value={tab}
        onChange={(_, value: number) => setTab(value)}
        aria-label="使用指南章节"
        sx={{ px: 3, flexShrink: 0 }}
      >
        <Tab
          id="group-help-tab-0"
          aria-controls="group-help-panel-0"
          label="使用入门"
        />
        <Tab
          id="group-help-tab-1"
          aria-controls="group-help-panel-1"
          label="脚本参考"
        />
        <Tab
          id="group-help-tab-2"
          aria-controls="group-help-panel-2"
          label="本机存储"
        />
      </Tabs>
      <DialogContent
        dividers
        sx={{ minHeight: 0, height: 440, overflowY: 'auto' }}
      >
        <Stack
          role="tabpanel"
          id={`group-help-panel-${tab}`}
          aria-labelledby={`group-help-tab-${tab}`}
          spacing={2.5}
        >
          {tab === 0 && (
            <>
              <Typography variant="body2">
                订阅组将一个或多个订阅的节点组合成独立代理组。每组可使用自己的筛选条件和连通性检测，多个组可同时启用，也可以复用同一来源。
              </Typography>
              <Box
                component="ol"
                sx={{ m: 0, pl: 2.5, '& li': { mb: 1.5 }, typography: 'body2' }}
              >
                <li>在“订阅”页面导入远程订阅或本地节点文件。</li>
                <li>
                  在“订阅组”中新建组，填写名称并勾选来源。来源数量没有固定限制。
                </li>
                <li>
                  在“逻辑脚本”中选择模板并按需编辑。只需组合节点时，使用“合并全部节点”即可。
                </li>
                <li>
                  点击“保存并应用”，在“检测结果”查看进度、通过节点和排除原因。
                </li>
                <li>
                  到“代理”页面选择组内节点；在规则中指定这个组，让需要的流量通过它。
                </li>
              </Box>
              <Alert severity="info">
                来源订阅负责下载和更新节点；订阅组负责组合和筛选。创建分组不会自动添加分流规则。
              </Alert>
              <Typography variant="body2" color="text.secondary">
                网络检测模板中的 .example
                网址是占位示例，使用前请替换目标网址、预期响应，并填写本机出口网卡。检测通过表示符合所设条件，不代表目标服务的全部功能均可使用。
              </Typography>
              <Typography variant="body2" color="text.secondary">
                当前支持订阅文件中的 proxies 节点列表，不支持仅通过
                proxy-providers 提供节点的来源。检测无节点通过时会显示
                REJECT；订阅组不会自动改用其他来源。
              </Typography>
            </>
          )}
          {tab === 1 && (
            <>
              <Typography variant="body2">
                main(config, groupName) 接收该组来源节点和组名。修改
                config.proxies 可筛选或重排节点；保留节点的 name 字段，并返回
                config。config.screening
                声明检测方式，网络请求由应用执行，脚本本身不直接发起请求。
              </Typography>
              <Box
                component="pre"
                sx={{
                  m: 0,
                  p: 1.5,
                  borderRadius: 1,
                  bgcolor: 'action.hover',
                  fontSize: 12,
                  lineHeight: 1.7,
                  whiteSpace: 'pre-wrap',
                }}
              >{`function main(config) {
  config.screening = {
    checks: [],
    interval: 300,
    concurrency: 16,
    "interface-name": "",
    automatic: false,
    sort: "original"
  };
  return config;
}`}</Box>
              <Stack component="dl" spacing={1.5} sx={{ m: 0 }}>
                {fields.map(([name, description]) => (
                  <Box key={name}>
                    <Typography
                      component="dt"
                      variant="body2"
                      sx={{ fontFamily: 'monospace', fontWeight: 600 }}
                    >
                      {name}
                    </Typography>
                    <Typography
                      component="dd"
                      variant="body2"
                      color="text.secondary"
                      sx={{ m: 0, mt: 0.5 }}
                    >
                      {description}
                    </Typography>
                  </Box>
                ))}
              </Stack>
              <Box>
                <Typography variant="subtitle2" gutterBottom>
                  响应验证方式
                </Typography>
                <Typography variant="body2">
                  status：验证预期 HTTP 状态码。html：要求 HTTP 200 和 HTML
                  页面，并排除已识别的验证页。auth-json：要求 HTTP 401
                  和符合条件的认证错误 JSON。
                </Typography>
              </Box>
              <Typography variant="body2" color="text.secondary">
                检测使用 GET 请求，不跟随重定向；单次请求超时为 10
                秒。首个目标提供排序延迟。完整检测失败会移除节点，后续完整检测通过后可重新加入。
              </Typography>
            </>
          )}
          {tab === 2 && (
            <>
              <Typography variant="subtitle2">本机配置位置</Typography>
              <Typography variant="body2">
                组名、来源引用和脚本保存在当前用户的应用数据目录中。下面的路径由应用根据本机操作系统和用户动态生成，不是安装包中固定的个人路径。
              </Typography>
              <Box sx={{ p: 1.5, borderRadius: 1, bgcolor: 'action.hover' }}>
                <Typography
                  component="code"
                  variant="body2"
                  sx={{ overflowWrap: 'anywhere', userSelect: 'text' }}
                >
                  {configPath ?? '正在读取本机配置位置…'}
                </Typography>
              </Box>
              <Typography variant="body2">
                Windows 使用当前用户的应用数据目录，macOS 使用当前用户的
                Library/Application Support，Linux
                使用用户数据目录。更换用户或电脑后，应用会使用对应设备的目录。
              </Typography>
              <Typography variant="body2">
                订阅地址仍由“订阅”页面管理；订阅组保存来源引用，不内置任何订阅地址。复制订阅组配置到另一台设备时，需要先导入来源订阅，再重新选择来源。
              </Typography>
            </>
          )}
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>关闭</Button>
      </DialogActions>
    </Dialog>
  )
}
