import { Card, Input, Select, Space, Button, Switch } from 'antd';
import {
  SearchOutlined,
  ExportOutlined,
  ClearOutlined,
  ReloadOutlined,
} from '@ant-design/icons';
import type { LogFilter } from '../../types/index.js';

const { Option } = Select;

export function LogToolbar({
  filter,
  setFilter,
  autoScroll,
  toggleAutoScroll,
  exporting,
  onRefresh,
  onExport,
  onClear,
}: {
  filter: LogFilter;
  setFilter: (patch: Partial<LogFilter>) => void;
  autoScroll: boolean;
  toggleAutoScroll: (v: boolean) => void;
  exporting: boolean;
  onRefresh: () => void;
  onExport: () => void;
  onClear: () => void;
}) {
  return (
    <Card variant="outlined" style={{ marginBottom: 24 }}>
      <Space direction="vertical" style={{ width: '100%' }} size={16}>
        <Space wrap>
          <Input
            placeholder="关键字搜索"
            prefix={<SearchOutlined />}
            value={filter.keyword ?? ''}
            onChange={(e) =>
              setFilter({ keyword: e.target.value || undefined })
            }
            style={{ width: 220 }}
            allowClear
          />
          <Select
            placeholder="日志等级"
            value={filter.level}
            onChange={(v) => setFilter({ level: v })}
            style={{ width: 130 }}
            allowClear
          >
            <Option value="DEBUG">DEBUG</Option>
            <Option value="INFO">INFO</Option>
            <Option value="WARN">WARN</Option>
            <Option value="ERROR">ERROR</Option>
          </Select>
          <Input
            placeholder="服务ID过滤"
            value={filter.serverId ?? ''}
            onChange={(e) =>
              setFilter({ serverId: e.target.value || undefined })
            }
            style={{ width: 150 }}
            allowClear
          />
          <Button icon={<ReloadOutlined />} onClick={onRefresh}>
            刷新
          </Button>
        </Space>

        <Space>
          <Switch checked={autoScroll} onChange={toggleAutoScroll} /> 自动滚动
          <Button
            icon={<ExportOutlined />}
            loading={exporting}
            onClick={onExport}
          >
            导出日志
          </Button>
          <Button danger icon={<ClearOutlined />} onClick={onClear}>
            清空日志
          </Button>
        </Space>
      </Space>
    </Card>
  );
}
