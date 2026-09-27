import React, { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import {
  Table,
  Card,
  Title,
  Text,
  Group,
  Pagination,
  Badge,
  Loader,
  Center,
  Code,
  Tooltip,
} from '@mantine/core';
import { useNavigate } from 'react-router-dom';
import dayjs from 'dayjs';
import { fetchExecutions, ExecutionFilterParams } from '../../shared/api/client';
import { StatusBadge, AttributionBadge, UnknownText } from '../../shared/ui/Badges';
import { ExecutionFilters } from '../../features/ExecutionFilters';
import { IconCpu, IconGitBranch, IconFolder } from '@tabler/icons-react';

export const ExecutionsPage: React.FC = () => {
  const navigate = useNavigate();
  const [filters, setFilters] = useState<ExecutionFilterParams>({
    page: 1,
    page_size: 15,
  });

  const { data, isLoading, error } = useQuery({
    queryKey: ['executions', filters],
    queryFn: () => fetchExecutions(filters),
    refetchInterval: 5000,
  });

  const formatDuration = (ms: number | null) => {
    if (ms === null || ms === undefined) return <UnknownText value={null} />;
    const seconds = Math.floor(ms / 1000);
    if (seconds < 60) return `${seconds}s`;
    const minutes = Math.floor(seconds / 60);
    const remSec = seconds % 60;
    return `${minutes}m ${remSec}s`;
  };

  return (
    <div style={{ padding: '24px', maxWidth: '1440px', margin: '0 auto' }}>
      <Group justify="space-between" mb="lg">
        <div>
          <Title order={2}>Agent Executions</Title>
          <Text c="dimmed" size="sm">
            Deterministic turn-level telemetry from wrapped AI coding agents.
          </Text>
        </div>
        {data && (
          <Badge size="lg" variant="light" color="blue">
            Total: {data.total} executions
          </Badge>
        )}
      </Group>

      <ExecutionFilters
        filters={filters}
        onChange={setFilters}
        onReset={() => setFilters({ page: 1, page_size: 15 })}
      />

      <Card withBorder radius="md" p={0} style={{ overflow: 'hidden' }}>
        {isLoading ? (
          <Center p="xl">
            <Loader size="lg" />
          </Center>
        ) : error ? (
          <Center p="xl">
            <Text c="red">Error loading executions: {(error as Error).message}</Text>
          </Center>
        ) : !data || data.items.length === 0 ? (
          <Center p="xl">
            <Text c="dimmed">No executions found matching your filters.</Text>
          </Center>
        ) : (
          <Table.ScrollContainer minWidth={1000}>
            <Table highlightOnHover verticalSpacing="sm">
              <Table.Thead>
                <Table.Tr>
                  <Table.Th>Time</Table.Th>
                  <Table.Th>Runner</Table.Th>
                  <Table.Th>Model</Table.Th>
                  <Table.Th>Reasoning</Table.Th>
                  <Table.Th>Agent / Role</Table.Th>
                  <Table.Th>Session</Table.Th>
                  <Table.Th>Repo / Branch</Table.Th>
                  <Table.Th>Worktree & Attribution</Table.Th>
                  <Table.Th>Duration</Table.Th>
                  <Table.Th>Status</Table.Th>
                </Table.Tr>
              </Table.Thead>
              <Table.Tbody>
                {data.items.map((exec) => (
                  <Table.Tr
                    key={exec.id}
                    style={{ cursor: 'pointer' }}
                    onClick={() => navigate(`/executions/${exec.id}`)}
                  >
                    <Table.Td>
                      <Tooltip label={exec.started_at}>
                        <Text size="xs" fw={500}>
                          {dayjs(exec.started_at).format('YYYY-MM-DD HH:mm:ss')}
                        </Text>
                      </Tooltip>
                    </Table.Td>

                    <Table.Td>
                      <Badge variant="outline" color="cyan" size="sm">
                        {exec.runtime_id.startsWith('run_') && exec.native_execution_id ? 'CLI' : 'UNKNOWN'}
                      </Badge>
                    </Table.Td>

                    <Table.Td>
                      <Group gap={4}>
                        <IconCpu size={14} color="gray" />
                        <UnknownText value={exec.model} />
                      </Group>
                    </Table.Td>

                    <Table.Td>
                      <UnknownText value={exec.reasoning_effort} />
                    </Table.Td>

                    <Table.Td>
                      <Badge color="violet" variant="light" size="sm">
                        {exec.selected_agent_role || 'main'}
                      </Badge>
                    </Table.Td>

                    <Table.Td>
                      <Code style={{ fontSize: '11px' }}>
                        {exec.native_execution_id !== 'UNKNOWN'
                          ? exec.native_execution_id
                          : exec.session_id.substring(0, 14)}
                      </Code>
                    </Table.Td>

                    <Table.Td>
                      <div>
                        <Text size="xs" fw={500} lineClamp={1}>
                          {exec.repo_root ? exec.repo_root.split('/').pop()?.split('\\').pop() : 'UNKNOWN'}
                        </Text>
                        {exec.branch && (
                          <Group gap={4} mt={2}>
                            <IconGitBranch size={12} color="gray" />
                            <Text size="xs" c="dimmed">
                              {exec.branch}
                            </Text>
                          </Group>
                        )}
                      </div>
                    </Table.Td>

                    <Table.Td>
                      <div>
                        {exec.worktree_path ? (
                          <Group gap={4}>
                            <IconFolder size={12} color="gray" />
                            <Text size="xs" lineClamp={1}>
                              {exec.worktree_path.split('/').pop()?.split('\\').pop()}
                            </Text>
                          </Group>
                        ) : (
                          <UnknownText value={null} />
                        )}
                        <AttributionBadge attribution={exec.git_attribution} />
                      </div>
                    </Table.Td>

                    <Table.Td>
                      <Text size="xs">{formatDuration(exec.duration_ms)}</Text>
                    </Table.Td>

                    <Table.Td>
                      <StatusBadge status={exec.status} />
                    </Table.Td>
                  </Table.Tr>
                ))}
              </Table.Tbody>
            </Table>
          </Table.ScrollContainer>
        )}

        {data && data.total_pages > 1 && (
          <Group justify="center" p="md" style={{ borderTop: '1px solid var(--mantine-color-default-border)' }}>
            <Pagination
              total={data.total_pages}
              value={filters.page || 1}
              onChange={(page) => setFilters({ ...filters, page })}
            />
          </Group>
        )}
      </Card>
    </div>
  );
};
