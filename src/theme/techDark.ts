/**
 * 暗黑科技风主题 token（Ant Design ConfigProvider）
 */
export const techDarkTokens = {
  colorPrimary: '#00d4ff',
  colorInfo: '#00d4ff',
  colorSuccess: '#22d3a6',
  colorWarning: '#f0b429',
  colorError: '#ff5c7a',
  colorBgBase: '#070b14',
  colorTextBase: '#e8f1ff',
  colorBorder: '#1e2d4a',
  colorBorderSecondary: '#162238',
  borderRadius: 8,
  fontFamilyCode: "'JetBrains Mono', 'SF Mono', 'Cascadia Code', Consolas, monospace",
  wireframe: false,
} as const;

export const techDarkComponentTokens = {
  Layout: {
    siderBg: '#080d18',
    bodyBg: '#070b14',
    headerBg: '#080d18',
    triggerBg: '#0c1220',
  },
  Menu: {
    darkItemBg: '#080d18',
    darkSubMenuItemBg: '#080d18',
    darkItemSelectedBg: 'rgba(0, 212, 255, 0.14)',
    darkItemSelectedColor: '#00d4ff',
    darkItemHoverBg: 'rgba(0, 212, 255, 0.08)',
    darkItemColor: '#9db0cc',
    itemBorderRadius: 6,
  },
  Card: {
    colorBgContainer: '#0f1628',
  },
  Table: {
    colorBgContainer: '#0f1628',
    headerBg: '#0c1220',
    headerColor: '#c5d4eb',
    borderColor: '#1e2d4a',
    rowHoverBg: 'rgba(0, 212, 255, 0.06)',
  },
  Modal: {
    contentBg: '#0f1628',
    headerBg: '#0f1628',
  },
  Drawer: {
    colorBgElevated: '#0f1628',
  },
  Button: {
    primaryShadow: '0 0 12px rgba(0, 212, 255, 0.35)',
  },
  Segmented: {
    trackBg: '#0a101c',
    itemSelectedBg: 'rgba(0, 212, 255, 0.16)',
    itemSelectedColor: '#00d4ff',
  },
  Input: {
    colorBgContainer: '#0a101c',
    activeBorderColor: '#00d4ff',
    hoverBorderColor: '#3adfff',
  },
  Select: {
    colorBgContainer: '#0a101c',
  },
  Tag: {
    defaultBg: 'rgba(0, 212, 255, 0.1)',
    defaultColor: '#7ddfff',
  },
} as const;

export const lightThemeTokens = {
  // 品牌青绿（与 Logo W1x-a #00D9A0 同色相，深一档保证白字可读）
  colorPrimary: '#00a67e',
  colorInfo: '#00a67e',
  colorSuccess: '#22d3a6',
  colorWarning: '#f0b429',
  colorError: '#ff5c7a',
  colorPrimaryBg: '#e6f9f3',
  colorBgBase: '#f4faf7',
  colorTextBase: '#000000',
  colorBorder: '#d9e6e2',
  borderRadius: 8,
} as const;
