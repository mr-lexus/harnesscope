import React from 'react';
import { Card, Group, Select, TextInput, Button, SimpleGrid } from '@mantine/core';
import { ExecutionFilterParams } from '../shared/api/client';
import { IconFilter, IconRotateClockwise } from '@tabler/icons-react';

interface Props {
  filters: ExecutionFilterParams;
  onChange: (updated: ExecutionFilterParams) => void;
  onReset: () => void;
}

export const ExecutionFilters: React.FC<Props> = ({ filters, onChange, onReset }) => {
  return (
    <Card withBorder p="md" radius="md" mb="md">
      <Group justify="space-between" mb="xs">
        <Group gap="xs">
          <IconFilter size={18} />
          <strong>Filters</strong>
        </Group>
        <Button variant="subtle" color="gray" size="xs" leftSection={<IconRotateClockwise size={14} />} onClick={onReset}>
          Reset Filters
        </Button>
      </Group>

      <SimpleGrid cols={{ base: 1, sm: 2, md: 4 }} spacing="sm">
        <Select
          label="Period"
          placeholder="All time"
          data={[
            { value: 'all', label: 'All time' },
            { value: '1h', label: 'Last 1 hour' },
            { value: '24h', label: 'Last 24 hours' },
            { value: '7d', label: 'Last 7 days' },
            { value: '30d', label: 'Last 30 days' },
          ]}
          value={filters.period || 'all'}
          onChange={(val) => onChange({ ...filters, period: val || 'all', page: 1 })}
          size="xs"
        />

        <Select
          label="Runner"
          placeholder="All runners"
          data={[
            { value: 'ALL', label: 'All runners' },
            { value: 'codex', label: 'Codex' },
            { value: 'copilot', label: 'GitHub Copilot' },
            { value: 'opencode', label: 'OpenCode' },
          ]}
          value={filters.runner || 'ALL'}
          onChange={(val) => onChange({ ...filters, runner: val || 'ALL', page: 1 })}
          size="xs"
        />

        <Select
          label="Status"
          placeholder="All statuses"
          data={[
            { value: 'ALL', label: 'All statuses' },
            { value: 'COMPLETED', label: 'COMPLETED' },
            { value: 'RUNNING', label: 'RUNNING' },
            { value: 'FAILED', label: 'FAILED' },
            { value: 'CANCELLED', label: 'CANCELLED' },
          ]}
          value={filters.status || 'ALL'}
          onChange={(val) => onChange({ ...filters, status: val || 'ALL', page: 1 })}
          size="xs"
        />

        <TextInput
          label="Model"
          placeholder="e.g. gpt-4o, claude"
          value={filters.model || ''}
          onChange={(e) => onChange({ ...filters, model: e.currentTarget.value, page: 1 })}
          size="xs"
        />

        <TextInput
          label="Agent / Role"
          placeholder="e.g. coder, architect"
          value={filters.agent || ''}
          onChange={(e) => onChange({ ...filters, agent: e.currentTarget.value, page: 1 })}
          size="xs"
        />

        <TextInput
          label="Branch"
          placeholder="e.g. main, feature/auth"
          value={filters.branch || ''}
          onChange={(e) => onChange({ ...filters, branch: e.currentTarget.value, page: 1 })}
          size="xs"
        />

        <TextInput
          label="Session ID"
          placeholder="Native or session ID"
          value={filters.session || ''}
          onChange={(e) => onChange({ ...filters, session: e.currentTarget.value, page: 1 })}
          size="xs"
        />

        <TextInput
          label="Component (MCP / Skill / Plugin)"
          placeholder="e.g. filesystem, git"
          value={filters.component || ''}
          onChange={(e) => onChange({ ...filters, component: e.currentTarget.value, page: 1 })}
          size="xs"
        />
      </SimpleGrid>
    </Card>
  );
};
