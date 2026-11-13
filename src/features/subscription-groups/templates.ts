type Check = {
  url: string
  'expected-status': number
  validation: 'status' | 'auth-json' | 'html'
}

const makeScript = (
  checks: Check[],
  filter = false,
) => `// Each group receives nodes from its selected subscriptions.
// Keep node names unchanged so selection and health checks remain stable.
${checks.length ? '// Replace the .example URLs and expected responses before applying.\n// Set interface-name to a physical outbound adapter on this device.\n' : ''}function main(config) {
${
  filter
    ? `  // Edit this list to keep the proxy protocols you need.
  const allowedTypes = ["ss", "vmess", "vless", "trojan", "hysteria2"];
  config.proxies = config.proxies.filter(p => allowedTypes.includes(p.type));

`
    : ''
}  config.screening = ${JSON.stringify(
  {
    interval: 300,
    concurrency: 16,
    'interface-name': '',
    automatic: checks.length > 0,
    sort: checks.length > 0 ? 'delay' : 'original',
    checks,
  },
  null,
  2,
)
  .split('\n')
  .join('\n  ')};
  return config;
}
`

export const templates = {
  all: {
    label: '合并全部节点',
    description: '汇集所选来源的节点，不执行网络检测。',
    script: makeScript([]),
  },
  filter: {
    label: '按条件筛选',
    description: '演示按节点协议筛选，可自行修改条件。',
    script: makeScript([], true),
  },
  http: {
    label: 'HTTP 连通性检测',
    description: '使用自定义网址检测节点，按延迟排序并提供自动选择。',
    script: makeScript([
      {
        url: 'https://your-service.example/health',
        'expected-status': 200,
        validation: 'status',
      },
    ]),
  },
  multiple: {
    label: '多目标检测',
    description: '配置多个网址，全部通过的节点才会加入分组。',
    script: makeScript([
      {
        url: 'https://service-a.example/health',
        'expected-status': 200,
        validation: 'status',
      },
      {
        url: 'https://service-b.example/',
        'expected-status': 200,
        validation: 'html',
      },
    ]),
  },
}
