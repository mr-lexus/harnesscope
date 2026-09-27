import React from 'react';
import { useQuery } from '@tanstack/react-query';
import {
  Card,
  Title,
  Text,
  Group,
  SimpleGrid,
  Badge,
  Loader,
  Center,
  Progress,
  Stack,
  Table,
} from '@mantine/core';
import { fetchStats } from '../../shared/api/client';
import { IconCpu, IconActivity, IconTerminal2, IconTool } from '@tabler/icons-react';

export const StatsPage: React.FC = () => {
  const { data: stats, isLoading, error } = useQuery({
    queryKey: ['stats'],
    queryFn: fetchStats,
    refetchInterval: 5000,
  });

  if (isLoading) {
    return (
      <Center style={{ height: '70vh' }}>
        <Loader size="xl" />
      </Center>
    );
  }

  if (error || !stats) {
    return (
      <Center style={{ height: '70vh' }}>
        <Text c="red">Failed to load telemetry stats.</Text>
      </Center>
    );
  }

  const formatDuration = (ms: number) => {
    const s = Math.round(ms / 1000);
    if (s < 60) return `${s}s`;
    const m = Math.floor(s / 60);
    return `${m}m ${s % 60}s`;
  };

  return (
    <div style={{ padding: '24px', maxWidth: '1440px', margin: '0 auto' }}>
      <Group justify="space-between" mb="lg">
        <div>
          <Title order={2}>Deterministic Telemetry Stats</Title>
          <Text c="dimmed" size="sm">
            Pure SQL aggregations directly from local SQLite database (Zero LLM / Zero Heuristics).
          </Text>
        </div>
      </Group>

      {/* Overview Cards */}
      <SimpleGrid cols={{ base: 1, sm: 2, md: 4 }} spacing="md" mb="xl">
        <Card withBorder radius="md" p="md">
          <Group justify="space-between">
            <Text size="xs" c="dimmed" fw={700} tt="uppercase">Total Executions</Text>
            <IconTerminal2 size={20} color="gray" />
          </Group>
          <Title order={2} mt="xs">{stats.total_executions}</Title>
          <Text size="xs" c="dimmed" mt={4}>Turn-level agent jobs</Text>
        </Card>

        <Card withBorder radius="md" p="md">
          <Group justify="space-between">
            <Text size="xs" c="dimmed" fw={700} tt="uppercase">Total Sessions</Text>
            <IconActivity size={20} color="gray" />
          </Group>
          <Title order={2} mt="xs">{stats.total_sessions}</Title>
          <Text size="xs" c="dimmed" mt={4}>Native agent conversations</Text>
        </Card>

        <Card withBorder radius="md" p="md">
          <Group justify="space-between">
            <Text size="xs" c="dimmed" fw={700} tt="uppercase">Runtime Instances</Text>
            <IconCpu size={20} color="gray" />
          </Group>
          <Title order={2} mt="xs">{stats.total_runtimes}</Title>
          <Text size="xs" c="dimmed" mt={4}>Spanned process instances</Text>
        </Card>

        <Card withBorder radius="md" p="md">
          <Group justify="space-between">
            <Text size="xs" c="dimmed" fw={700} tt="uppercase">Avg Turn Duration</Text>
            <IconTool size={20} color="gray" />
          </Group>
          <Title order={2} mt="xs">{formatDuration(stats.avg_duration_ms)}</Title>
          <Text size="xs" c="dimmed" mt={4}>Wall-clock duration</Text>
        </Card>
      </SimpleGrid>

      {/* Breakdowns Grid */}
      <SimpleGrid cols={{ base: 1, md: 2 }} spacing="lg" mb="xl">
        {/* By Runner */}
        <Card withBorder radius="md" p="md">
          <Title order={4} mb="md">Executions by Runner</Title>
          <Stack gap="sm">
            {stats.by_runner.map((item) => {
              const pct = stats.total_executions > 0 ? (item.count / stats.total_executions) * 100 : 0;
              return (
                <div key={item.key}>
                  <Group justify="space-between" mb={4}>
                    <Text size="sm" fw={500}>{item.key}</Text>
                    <Badge variant="light">{item.count} ({pct.toFixed(0)}%)</Badge>
                  </Group>
                  <Progress value={pct} color="blue" size="sm" />
                </div>
              );
            })}
          </Stack>
        </Card>

        {/* By Model */}
        <Card withBorder radius="md" p="md">
          <Title order={4} mb="md">Executions by Model</Title>
          <Stack gap="sm">
            {stats.by_model.map((item) => {
              const pct = stats.total_executions > 0 ? (item.count / stats.total_executions) * 100 : 0;
              return (
                <div key={item.key}>
                  <Group justify="space-between" mb={4}>
                    <Text size="sm" fw={500}>{item.key}</Text>
                    <Badge variant="light">{item.count} ({pct.toFixed(0)}%)</Badge>
                  </Group>
                  <Progress value={pct} color="teal" size="sm" />
                </div>
              );
            })}
          </Stack>
        </Card>

        {/* By Reasoning Effort */}
        <Card withBorder radius="md" p="md">
          <Title order={4} mb="md">Executions by Reasoning Effort</Title>
          <Stack gap="sm">
            {stats.by_reasoning.map((item) => {
              const pct = stats.total_executions > 0 ? (item.count / stats.total_executions) * 100 : 0;
              return (
                <div key={item.key}>
                  <Group justify="space-between" mb={4}>
                    <Text size="sm" fw={500}>{item.key}</Text>
                    <Badge variant="light">{item.count} ({pct.toFixed(0)}%)</Badge>
                  </Group>
                  <Progress value={pct} color="indigo" size="sm" />
                </div>
              );
            })}
          </Stack>
        </Card>

        {/* By Status */}
        <Card withBorder radius="md" p="md">
          <Title order={4} mb="md">Status Distribution</Title>
          <Stack gap="sm">
            {stats.by_status.map((item) => {
              const pct = stats.total_executions > 0 ? (item.count / stats.total_executions) * 100 : 0;
              const color = item.key === 'COMPLETED' ? 'green' : item.key === 'FAILED' ? 'red' : 'blue';
              return (
                <div key={item.key}>
                  <Group justify="space-between" mb={4}>
                    <Text size="sm" fw={500}>{item.key}</Text>
                    <Badge color={color} variant="light">{item.count} ({pct.toFixed(0)}%)</Badge>
                  </Group>
                  <Progress value={pct} color={color} size="sm" />
                </div>
              );
            })}
          </Stack>
        </Card>
      </SimpleGrid>

      {/* Components Statistics */}
      <SimpleGrid cols={{ base: 1, md: 2 }} spacing="lg">
        {/* MCP Configured vs Invoked */}
        <Card withBorder radius="md" p="md">
          <Title order={4} mb="sm">MCP Servers: Configured vs Invoked</Title>
          <Text size="xs" c="dimmed" mb="md">
            Distinguishing servers discovered in configs vs servers proven to be invoked.
          </Text>
          {stats.mcp_stats.length === 0 ? (
            <Text size="sm" c="dimmed">No MCP data available.</Text>
          ) : (
            <Table>
              <Table.Thead>
                <Table.Tr>
                  <Table.Th>Server Name</Table.Th>
                  <Table.Th>Configured in Executions</Table.Th>
                  <Table.Th>Invoked in Executions</Table.Th>
                </Table.Tr>
              </Table.Thead>
              <Table.Tbody>
                {stats.mcp_stats.map((mcp) => (
                  <Table.Tr key={mcp.name}>
                    <Table.Td fw={600}>{mcp.name}</Table.Td>
                    <Table.Td>
                      <Badge variant="light" color="blue">{mcp.configured}</Badge>
                    </Table.Td>
                    <Table.Td>
                      <Badge variant="filled" color={mcp.invoked > 0 ? 'green' : 'gray'}>
                        {mcp.invoked}
                      </Badge>
                    </Table.Td>
                  </Table.Tr>
                ))}
              </Table.Tbody>
            </Table>
          )}
        </Card>

        {/* Skills & Plugins */}
        <Card withBorder radius="md" p="md">
          <Title order={4} mb="sm">Skills & Plugins Usage</Title>
          <Text size="xs" c="dimmed" mb="md">
            Discovered/invoked skills and plugins across all executions.
          </Text>
          <Stack gap="md">
            <div>
              <Text size="xs" fw={700} tt="uppercase" c="dimmed" mb="xs">Skills</Text>
              {stats.skill_stats.length === 0 ? (
                <Text size="xs" c="dimmed">No skills discovered.</Text>
              ) : (
                <Group gap="xs">
                  {stats.skill_stats.map((s) => (
                    <Badge key={s.key} variant="outline" color="cyan" size="md">
                      {s.key}: {s.count}
                    </Badge>
                  ))}
                </Group>
              )}
            </div>

            <div>
              <Text size="xs" fw={700} tt="uppercase" c="dimmed" mb="xs">Plugins</Text>
              {stats.plugin_stats.length === 0 ? (
                <Text size="xs" c="dimmed">No plugins discovered.</Text>
              ) : (
                <Group gap="xs">
                  {stats.plugin_stats.map((p) => (
                    <Badge key={p.key} variant="outline" color="grape" size="md">
                      {p.key}: {p.count}
                    </Badge>
                  ))}
                </Group>
              )}
            </div>
          </Stack>
        </Card>
      </SimpleGrid>
    </div>
  );
};
