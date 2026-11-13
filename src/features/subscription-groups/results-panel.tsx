import { PlayArrowRounded } from '@mui/icons-material'
import {
  Alert,
  Box,
  Button,
  Chip,
  Collapse,
  LinearProgress,
  MenuItem,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TablePagination,
  TableRow,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material'
import { useState } from 'react'

import type { Report } from '@/services/bindings'

function displayName(name: string) {
  const parts = name.split('/')
  return name.startsWith('SG/') && parts.length >= 5
    ? { name: parts.slice(4).join('/'), source: parts[2] }
    : { name, source: '' }
}

function displayReason(reason: string) {
  if (reason === 'Passed all checks') return '全部检查通过'
  if (reason === 'Latest health check failed') return '最新连通性检查失败'
  if (reason === 'Selected by script; connectivity not tested')
    return '脚本选中 · 未测试连通性'
  if (reason.startsWith('Not currently admitted;')) return '等待重新检测'
  return reason
    .replace(/^Check (\d+): /, '第 $1 项：')
    .replace('connection or response error', '连接或响应异常')
    .replace('timeout', '超时')
    .replace(/HTTP (\d+), expected (\d+)/, 'HTTP $1（预期 $2）')
}

export function ResultsPanel({
  report,
  disabled,
  onScan,
}: {
  report?: Report
  disabled: boolean
  onScan: () => void
}) {
  const [filter, setFilter] = useState('all')
  const [search, setSearch] = useState('')
  const [page, setPage] = useState(0)
  const [details, setDetails] = useState(false)
  const [logs, setLogs] = useState(false)
  const nodes = report?.nodes ?? []
  const passed = nodes.filter((node) => node.passed).length
  const visible = nodes
    .filter(
      (node) =>
        (filter === 'all' || node.passed === (filter === 'passed')) &&
        node.name.toLowerCase().includes(search.toLowerCase()),
    )
    .sort(
      (a, b) =>
        Number(b.passed) - Number(a.passed) ||
        (a.delay ?? Infinity) - (b.delay ?? Infinity),
    )
  const currentPage = Math.min(
    page,
    Math.max(0, Math.ceil(visible.length / 20) - 1),
  )
  return (
    <Box
      role="tabpanel"
      aria-label="检测结果"
      sx={{ display: 'flex', flexDirection: 'column', flex: 1, minHeight: 0 }}
    >
      <Box sx={{ px: 2, pt: 1.5, pb: 1, flexShrink: 0 }}>
        <Stack
          direction="row"
          sx={{ gap: 1, alignItems: 'center', justifyContent: 'space-between' }}
        >
          <Box>
            <Typography variant="body2" sx={{ fontWeight: 600 }}>
              {report?.running
                ? `检测中 · ${report.completed} / ${report.total}`
                : report?.lastScan
                  ? '检测完成'
                  : '等待检测'}
            </Typography>
            <Typography variant="caption" color="text.secondary">
              {report?.lastScan
                ? `最近完成 ${new Date(report.lastScan).toLocaleTimeString()}`
                : '检测结果将在这里更新'}
            </Typography>
          </Box>
          <Button
            size="small"
            variant="outlined"
            startIcon={<PlayArrowRounded />}
            disabled={disabled || report?.running}
            onClick={onScan}
            sx={{ whiteSpace: 'nowrap' }}
          >
            立即检测
          </Button>
        </Stack>
        <Stack
          direction="row"
          sx={{ gap: 1, mt: 1.25, alignItems: 'center', flexWrap: 'wrap' }}
        >
          <Chip
            size="small"
            variant="outlined"
            label={`总计 ${report?.total ?? 0}`}
          />
          <Chip
            size="small"
            color="success"
            variant="outlined"
            label={`保留 ${passed}`}
          />
          <Chip
            size="small"
            variant="outlined"
            label={`排除 ${nodes.length - passed}`}
          />
          {!!report?.options.checks?.length && (
            <Chip
              size="small"
              variant="outlined"
              label={`并发 ${report.options.concurrency ?? 16}`}
            />
          )}
          <Button
            size="small"
            onClick={() => setDetails(!details)}
            sx={{ ml: 'auto' }}
          >
            {details ? '收起检测条件' : '检测条件'}
          </Button>
        </Stack>
        {report?.running && (
          <LinearProgress
            variant={report.total ? 'determinate' : 'indeterminate'}
            value={report.total ? (report.completed / report.total) * 100 : 0}
            sx={{ mt: 1.25, borderRadius: 1 }}
          />
        )}
        {report?.error && (
          <Alert
            severity="error"
            sx={{ mt: 1, maxHeight: 90, overflow: 'auto' }}
          >
            {report.error}
          </Alert>
        )}
        <Collapse in={details}>
          <Box
            sx={{
              mt: 1,
              p: 1,
              borderRadius: 1,
              bgcolor: 'action.hover',
              maxHeight: 130,
              overflow: 'auto',
            }}
          >
            {report?.options.checks?.length ? (
              report.options.checks.map((check, index) => (
                <Typography
                  variant="caption"
                  key={`${check.url}:${check['expected-status']}:${check.validation}`}
                  component="div"
                  sx={{ overflowWrap: 'anywhere', mb: 0.75 }}
                >
                  {index + 1}. GET {check.url}
                  <br />
                  预期 {check['expected-status']} · {check.validation}
                </Typography>
              ))
            ) : (
              <Typography variant="caption">
                未配置连通性检查，仅使用脚本筛选。
              </Typography>
            )}
          </Box>
        </Collapse>
        <Stack direction="row" spacing={1} sx={{ mt: 1.5 }}>
          <TextField
            size="small"
            placeholder="搜索节点或订阅"
            value={search}
            onChange={(e) => {
              setSearch(e.target.value)
              setPage(0)
            }}
            fullWidth
            slotProps={{ htmlInput: { 'aria-label': '搜索检测结果' } }}
          />
          <TextField
            select
            size="small"
            value={filter}
            onChange={(e) => {
              setFilter(e.target.value)
              setPage(0)
            }}
            sx={{ width: 110, flexShrink: 0 }}
            slotProps={{ select: { 'aria-label': '结果状态' } }}
          >
            <MenuItem value="all">全部结果</MenuItem>
            <MenuItem value="passed">仅保留</MenuItem>
            <MenuItem value="failed">仅排除</MenuItem>
          </TextField>
        </Stack>
      </Box>
      <TableContainer
        sx={{
          flex: 1,
          minHeight: 0,
          overflow: 'auto',
          overscrollBehavior: 'contain',
          scrollbarGutter: 'stable',
        }}
        aria-label="节点检测结果列表"
        tabIndex={0}
      >
        <Table
          stickyHeader
          size="small"
          sx={{
            tableLayout: 'fixed',
            '& .MuiTableCell-root': { px: 1.5 },
            '& .MuiTableCell-head': { fontSize: 12, color: 'text.secondary' },
          }}
        >
          <TableHead>
            <TableRow>
              <TableCell>节点 / 订阅</TableCell>
              <TableCell sx={{ width: 58 }}>状态</TableCell>
              <TableCell align="right" sx={{ width: 68 }}>
                延迟 ↑
              </TableCell>
              <TableCell sx={{ width: '26%' }}>结果说明</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {visible
              .slice(currentPage * 20, (currentPage + 1) * 20)
              .map((node) => {
                const label = displayName(node.name)
                return (
                  <TableRow key={node.name} hover>
                    <TableCell>
                      <Tooltip title={node.name}>
                        <Typography
                          variant="body2"
                          noWrap
                          sx={{ fontSize: 12, fontWeight: 500 }}
                        >
                          {label.name}
                        </Typography>
                      </Tooltip>
                      <Typography
                        variant="caption"
                        color="text.secondary"
                        noWrap
                        component="div"
                      >
                        {label.source}
                      </Typography>
                    </TableCell>
                    <TableCell>
                      <Box
                        component="span"
                        sx={{
                          fontSize: 12,
                          color: node.passed
                            ? 'success.main'
                            : 'text.secondary',
                          whiteSpace: 'nowrap',
                        }}
                      >
                        {node.passed ? '保留' : '排除'}
                      </Box>
                    </TableCell>
                    <TableCell
                      align="right"
                      sx={{
                        fontSize: 12,
                        fontVariantNumeric: 'tabular-nums',
                        whiteSpace: 'nowrap',
                      }}
                    >
                      {node.delay == null ? '—' : `${node.delay} ms`}
                    </TableCell>
                    <TableCell>
                      <Tooltip title={node.reason}>
                        <Typography
                          variant="caption"
                          sx={{ overflowWrap: 'anywhere', lineHeight: 1.5 }}
                        >
                          {displayReason(node.reason)}
                        </Typography>
                      </Tooltip>
                    </TableCell>
                  </TableRow>
                )
              })}
            {!visible.length && (
              <TableRow>
                <TableCell
                  colSpan={4}
                  align="center"
                  sx={{ py: 6, color: 'text.secondary' }}
                >
                  {nodes.length ? '没有匹配的节点' : '暂无检测结果'}
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </TableContainer>
      <TablePagination
        component="div"
        count={visible.length}
        page={currentPage}
        rowsPerPage={20}
        rowsPerPageOptions={[20]}
        onPageChange={(_, value) => setPage(value)}
        labelDisplayedRows={({ from, to, count }) => `${from}–${to} / ${count}`}
        getItemAriaLabel={(type) => (type === 'next' ? '下一页' : '上一页')}
        sx={{
          flexShrink: 0,
          borderTop: 1,
          borderColor: 'divider',
          '& .MuiTablePagination-toolbar': { minHeight: 44, px: 1 },
        }}
      />
      {!!report?.logs.length && (
        <Box sx={{ px: 1.5, pb: 0.5, flexShrink: 0 }}>
          <Button size="small" onClick={() => setLogs(!logs)}>
            脚本输出 ({report.logs.length})
          </Button>
          <Collapse in={logs}>
            <Box
              component="pre"
              sx={{
                maxHeight: 100,
                overflow: 'auto',
                fontSize: 12,
                whiteSpace: 'pre-wrap',
                my: 0,
              }}
            >
              {report.logs.join('\n')}
            </Box>
          </Collapse>
        </Box>
      )}
    </Box>
  )
}
