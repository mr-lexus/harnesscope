import React from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import {
  Card,
  Title,
  Text,
  Group,
  Stack,
  Badge,
  Loader,
  Center,
  Button,
  Tabs,
  Timeline,
  Code,
  Table,
  SimpleGrid,
  Box,
} from '@mantine/core';
import dayjs from 'dayjs';
import { fetchExecutionDetail } from '../../shared/api/client';
import { StatusBadge, AttributionBadge, ComponentStateBadge, UnknownText } from '../../shared/ui/Badges';
import {
  IconArrowLeft,
  IconCpu,
  IconGitBranch,
  IconClock,
  IconFolder,
  IconServer,
  IconRobot,
  IconFileDiff,
  IconListDetails,
} from '@tabler/icons-react';

export const ExecutionDetailPage: React.FC = () => {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();

  const { data, isLoading, error } = useQuery({
    queryKey: ['execution', id],
    queryFn: () => fetchExecutionDetail(id!),
    enabled: Boolean(id),
  });

  if (isLoading) {
    return (
      <Center style={{ height: '70vh' }}>
        <Loader size="xl" />
      </Center>
    );
  }

  if (error || !data) {
    return (
      <Center style={{ height: '70vh' }}>
        <Stack align="center">
          <Text c="red" size="lg">Execution not found or failed to load.</Text>
          <Button variant="outline" onClick={() => navigate('/executions')}>
            Back to Executions
          </Button>
        </Stack>
      </Center>
    );
  }

  const { execution, runtime, session, agents, components, git_snapshots, events } = data;

  const mcpComponents = components.filter((c) => c.component_type === 'MCP');
  const skillComponents = components.filter((c) => c.component_type === 'SKILL');
  const pluginComponents = components.filter((c) => c.component_type === 'PLUGIN' || c.component_type === 'TOOL');

  const beforeSnap = git_snapshots.find((s) => s.snapshot_type === 'BEFORE');
  const afterSnap = git_snapshots.find((s) => s.snapshot_type === 'AFTER');

  return (
    <div style={{ padding: '24px', maxWidth: '1440px', margin: '0 auto' }}>
      <Button
        variant="subtle"
        leftSection={<IconArrowLeft size={16} />}
        onClick={() => navigate('/executions')}
        mb="md"
      >
        Back to Executions
      </Button>

      {/* Top Header Card */}
      <Card withBorder radius="md" p="lg" mb="lg">
        <Group justify="space-between" align="flex-start">
          <div>
            <Group gap="sm">
              <Title order={3}>Execution: {execution.id}</Title>
              <StatusBadge status={execution.status} />
              <AttributionBadge attribution={execution.git_attribution} />
            </Group>
            <Text c="dimmed" size="sm" mt={4}>
              Started: {dayjs(execution.started_at).format('YYYY-MM-DD HH:mm:ss')}
            </Text>
            {execution.prompt_summary && (
              <Text size="sm" mt="xs" fw={500}>
                Prompt: "{execution.prompt_summary}"
              </Text>
            )}
          </div>

          <Group gap="xs">
            <Badge size="lg" variant="light" color="blue" leftSection={<IconClock size={14} />}>
              Duration: {execution.duration_ms ? `${(execution.duration_ms / 1000).toFixed(1)}s` : 'In progress'}
            </Badge>
            {execution.exit_code !== null && (
              <Badge size="lg" variant="outline" color={execution.exit_code === 0 ? 'green' : 'red'}>
                Exit code: {execution.exit_code}
              </Badge>
            )}
          </Group>
        </Group>

        {execution.error_message && (
          <Box mt="md" p="sm" style={{ backgroundColor: 'rgba(255,0,0,0.05)', borderRadius: '6px' }}>
            <Text size="sm" c="red" fw={600}>
              Error: {execution.error_message}
            </Text>
          </Box>
        )}

        <SimpleGrid cols={{ base: 1, sm: 2, md: 4 }} spacing="md" mt="lg">
          <div>
            <Text size="xs" c="dimmed">Model & Reasoning</Text>
            <Group gap={6} mt={2}>
              <IconCpu size={16} color="gray" />
              <UnknownText value={execution.model} />
              <Badge size="xs" variant="outline">
                effort: {execution.reasoning_effort}
              </Badge>
            </Group>
          </div>

          <div>
            <Text size="xs" c="dimmed">Runner & Runtime</Text>
            <Text size="sm" fw={500} mt={2}>
              {runtime ? `${runtime.runner_name} (v${runtime.runner_version})` : 'UNKNOWN'}
            </Text>
            <Text size="xs" c="dimmed">
              PID: {runtime?.pid ?? 'UNKNOWN'} | OS: {runtime?.os ?? 'UNKNOWN'}
            </Text>
          </div>

          <div>
            <Text size="xs" c="dimmed">Session</Text>
            <Text size="sm" fw={500} mt={2} lineClamp={1}>
              {session?.title || session?.native_session_id || 'UNKNOWN'}
            </Text>
            <Button
              variant="subtle"
              size="compact-xs"
              p={0}
              onClick={() => navigate(`/sessions/${execution.session_id}`)}
            >
              View Session Details
            </Button>
          </div>

          <div>
            <Text size="xs" c="dimmed">Git Worktree</Text>
            <Group gap={4} mt={2}>
              <IconFolder size={14} color="gray" />
              <Text size="xs" lineClamp={1}>
                {execution.worktree_path || 'UNKNOWN'}
              </Text>
            </Group>
            {execution.branch && (
              <Group gap={4}>
                <IconGitBranch size={12} color="gray" />
                <Text size="xs" c="dimmed">{execution.branch}</Text>
              </Group>
            )}
          </div>
        </SimpleGrid>
      </Card>

      {/* Tabs Section */}
      <Tabs defaultValue="components">
        <Tabs.List mb="md">
          <Tabs.Tab value="components" leftSection={<IconServer size={16} />}>
            Components (MCP, Skills, Plugins) ({components.length})
          </Tabs.Tab>
          <Tabs.Tab value="git" leftSection={<IconFileDiff size={16} />}>
            Git Context & Changes ({git_snapshots.length})
          </Tabs.Tab>
          <Tabs.Tab value="agents" leftSection={<IconRobot size={16} />}>
            Agents & Subagents ({agents.length})
          </Tabs.Tab>
          <Tabs.Tab value="events" leftSection={<IconListDetails size={16} />}>
            Event Timeline ({events.length})
          </Tabs.Tab>
        </Tabs.List>

        {/* 1. Components Tab */}
        <Tabs.Panel value="components">
          <SimpleGrid cols={{ base: 1, md: 3 }} spacing="lg">
            {/* MCP Servers */}
            <Card withBorder radius="md" p="md">
              <Title order={5} mb="sm">
                Model Context Protocol (MCP) ({mcpComponents.length})
              </Title>
              {mcpComponents.length === 0 ? (
                <Text size="sm" c="dimmed">No MCP servers recorded.</Text>
              ) : (
                <Stack gap="xs">
                  {mcpComponents.map((c) => (
                    <Box key={c.component_id} p="xs" style={{ border: '1px solid var(--mantine-color-default-border)', borderRadius: '6px' }}>
                      <Group justify="space-between">
                        <Text size="sm" fw={600}>
                          {c.component_name || c.component_id}
                        </Text>
                        <ComponentStateBadge state={c.state} count={c.invocations_count} />
                      </Group>
                      {c.details_json && (
                        <Code block mt="xs" style={{ fontSize: '11px' }}>
                          {c.details_json}
                        </Code>
                      )}
                    </Box>
                  ))}
                </Stack>
              )}
            </Card>

            {/* Skills */}
            <Card withBorder radius="md" p="md">
              <Title order={5} mb="sm">
                Skills ({skillComponents.length})
              </Title>
              {skillComponents.length === 0 ? (
                <Text size="sm" c="dimmed">No skills discovered.</Text>
              ) : (
                <Stack gap="xs">
                  {skillComponents.map((c) => (
                    <Box key={c.component_id} p="xs" style={{ border: '1px solid var(--mantine-color-default-border)', borderRadius: '6px' }}>
                      <Group justify="space-between">
                        <Text size="sm" fw={600}>
                          {c.component_name || c.component_id}
                        </Text>
                        <ComponentStateBadge state={c.state} count={c.invocations_count} />
                      </Group>
                    </Box>
                  ))}
                </Stack>
              )}
            </Card>

            {/* Plugins & Tools */}
            <Card withBorder radius="md" p="md">
              <Title order={5} mb="sm">
                Plugins & Tools ({pluginComponents.length})
              </Title>
              {pluginComponents.length === 0 ? (
                <Text size="sm" c="dimmed">No plugins recorded.</Text>
              ) : (
                <Stack gap="xs">
                  {pluginComponents.map((c) => (
                    <Box key={c.component_id} p="xs" style={{ border: '1px solid var(--mantine-color-default-border)', borderRadius: '6px' }}>
                      <Group justify="space-between">
                        <Text size="sm" fw={600}>
                          {c.component_name || c.component_id}
                        </Text>
                        <ComponentStateBadge state={c.state} count={c.invocations_count} />
                      </Group>
                    </Box>
                  ))}
                </Stack>
              )}
            </Card>
          </SimpleGrid>
        </Tabs.Panel>

        {/* 2. Git Context Tab */}
        <Tabs.Panel value="git">
          <SimpleGrid cols={{ base: 1, md: 2 }} spacing="lg">
            {/* Before Snapshot */}
            <Card withBorder radius="md" p="md">
              <Group justify="space-between" mb="xs">
                <Title order={5}>Snapshot: BEFORE Execution</Title>
                <Badge color="gray">BEFORE</Badge>
              </Group>
              {beforeSnap ? (
                <Stack gap="xs">
                  <Text size="xs">
                    <strong>Commit:</strong> <Code>{beforeSnap.head_commit}</Code>
                  </Text>
                  <Text size="xs">
                    <strong>Branch:</strong> {beforeSnap.branch}
                  </Text>
                  <Text size="xs">
                    <strong>Dirty state:</strong> {beforeSnap.is_dirty ? 'Dirty (uncommitted changes)' : 'Clean'}
                  </Text>
                  <Text size="xs">
                    <strong>Changed files before:</strong> {beforeSnap.changed_files_count}
                  </Text>
                  <Text size="xs">
                    <strong>Attribution:</strong> <AttributionBadge attribution={beforeSnap.attribution} />
                  </Text>
                </Stack>
              ) : (
                <Text size="sm" c="dimmed">No before snapshot captured.</Text>
              )}
            </Card>

            {/* After Snapshot */}
            <Card withBorder radius="md" p="md">
              <Group justify="space-between" mb="xs">
                <Title order={5}>Snapshot: AFTER Execution</Title>
                <Badge color="blue">AFTER</Badge>
              </Group>
              {afterSnap ? (
                <Stack gap="xs">
                  <Text size="xs">
                    <strong>Commit:</strong> <Code>{afterSnap.head_commit}</Code>
                  </Text>
                  <Text size="xs">
                    <strong>Branch:</strong> {afterSnap.branch}
                  </Text>
                  <Text size="xs">
                    <strong>Diff stat:</strong> {afterSnap.diff_stat || 'No diff reported'}
                  </Text>
                  <Text size="xs">
                    <strong>Changed files:</strong> {afterSnap.changed_files_count}
                  </Text>
                  <Text size="xs">
                    <strong>Attribution:</strong> <AttributionBadge attribution={afterSnap.attribution} />
                  </Text>
                  {afterSnap.changed_files_json && (
                    <Box mt="xs">
                      <Text size="xs" fw={600} mb={4}>Modified Files:</Text>
                      <Code block style={{ fontSize: '11px' }}>
                        {afterSnap.changed_files_json}
                      </Code>
                    </Box>
                  )}
                </Stack>
              ) : (
                <Text size="sm" c="dimmed">No after snapshot captured (execution might be in progress).</Text>
              )}
            </Card>
          </SimpleGrid>
        </Tabs.Panel>

        {/* 3. Agents Tab */}
        <Tabs.Panel value="agents">
          <Card withBorder radius="md" p="md">
            <Title order={5} mb="sm">Agent Instances</Title>
            {agents.length === 0 ? (
              <Text size="sm" c="dimmed">No agent instances recorded.</Text>
            ) : (
              <Table>
                <Table.Thead>
                  <Table.Tr>
                    <Table.Th>Name</Table.Th>
                    <Table.Th>Role</Table.Th>
                    <Table.Th>Model</Table.Th>
                    <Table.Th>Parent Agent</Table.Th>
                    <Table.Th>Started</Table.Th>
                    <Table.Th>Status</Table.Th>
                  </Table.Tr>
                </Table.Thead>
                <Table.Tbody>
                  {agents.map((ag) => (
                    <Table.Tr key={ag.id}>
                      <Table.Td fw={600}>{ag.agent_name}</Table.Td>
                      <Table.Td>
                        <Badge variant="light" color="violet">{ag.agent_role}</Badge>
                      </Table.Td>
                      <Table.Td><UnknownText value={ag.model} /></Table.Td>
                      <Table.Td>{ag.parent_agent_id || 'None (Main)'}</Table.Td>
                      <Table.Td>{dayjs(ag.started_at).format('HH:mm:ss')}</Table.Td>
                      <Table.Td><StatusBadge status={ag.status} /></Table.Td>
                    </Table.Tr>
                  ))}
                </Table.Tbody>
              </Table>
            )}
          </Card>
        </Tabs.Panel>

        {/* 4. Events Timeline Tab */}
        <Tabs.Panel value="events">
          <Card withBorder radius="md" p="md">
            <Title order={5} mb="md">Event Timeline</Title>
            {events.length === 0 ? (
              <Text size="sm" c="dimmed">No timeline events recorded.</Text>
            ) : (
              <Timeline active={events.length - 1} bulletSize={22} lineWidth={2}>
                {events.map((ev, index) => (
                  <Timeline.Item
                    key={ev.id || index}
                    title={
                      <Group gap="xs">
                        <Text size="sm" fw={600}>
                          {ev.event_type}
                        </Text>
                        <Badge size="xs" variant="outline" color="gray">
                          {ev.source}
                        </Badge>
                      </Group>
                    }
                  >
                    <Text c="dimmed" size="xs">
                      {dayjs(ev.timestamp).format('YYYY-MM-DD HH:mm:ss.SSS')}
                    </Text>
                    {ev.payload_json && ev.payload_json !== '{}' && (
                      <Code block mt={4} style={{ fontSize: '11px', maxHeight: '180px', overflowY: 'auto' }}>
                        {ev.payload_json}
                      </Code>
                    )}
                  </Timeline.Item>
                ))}
              </Timeline>
            )}
          </Card>
        </Tabs.Panel>
      </Tabs>
    </div>
  );
};
