import { useRefreshInterval } from '../../shared/preferences';
import { EventTimeline } from '../../features/EventTimeline';
import { ExecutionReview } from '../../features/ExecutionReview';
import React, { useState } from 'react';
import { useParams, useNavigate, useLocation } from 'react-router-dom';
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
  const location = useLocation();
  const backTo = typeof location.state?.from === "string" && location.state.from.startsWith("/executions?") ? location.state.from : "/executions";
  const [activeTab,setActiveTab] = useState<string | null>('review');

  const { data, isLoading, error } = useQuery({
    queryKey: ['execution', id],
    queryFn: () => fetchExecutionDetail(id!),
    enabled: Boolean(id),
    refetchInterval: useRefreshInterval(),
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
          <Button variant="outline" onClick={() => navigate(backTo)}>
            Back to Executions
          </Button>
        </Stack>
      </Center>
    );
  }

  const { execution, runtime, session, agents, components, git_snapshots, events_total } = data;

  const mcpComponents = components.filter((c) => c.component_type === 'MCP');
  const skillComponents = components.filter((c) => c.component_type === 'SKILL');
  const pluginComponents = components.filter((c) => c.component_type === 'PLUGIN' || c.component_type === 'TOOL');

  const beforeSnap = git_snapshots.find((s) => s.snapshot_type === 'BEFORE');
  const afterSnap = git_snapshots.find((s) => s.snapshot_type === 'AFTER');

  return (
    <div className="panel-page">
      <Button
        variant="subtle"
        leftSection={<IconArrowLeft size={16} />}
        onClick={() => navigate(backTo)}
        mb="md"
      >
        Back to Executions
      </Button>

      {/* Top Header Card */}
      <Card withBorder radius="md" p="sm" mb="md">
        <Group justify="space-between" align="flex-start">
          <div>
            <Group gap="sm">
              <Title order={2}>{execution.prompt_summary || `${runtime?.runner_name ?? "Agent"} execution`}</Title>
              <StatusBadge status={execution.status} />
              {execution.git_attribution !== 'UNKNOWN' && <AttributionBadge attribution={execution.git_attribution} />}
              <Badge variant="outline">{execution.capture_scope} observation</Badge>
            </Group>
            <Text c="dimmed" size="sm" mt={4}>
              Started: {dayjs(execution.started_at).format('YYYY-MM-DD HH:mm:ss')}
            </Text>
          </div>

          <Group gap="xs">
            <Badge size="lg" variant="light" color="blue" leftSection={<IconClock size={14} />}>
              Duration: {execution.duration_ms !== null ? `${(execution.duration_ms / 1000).toFixed(1)}s` : execution.status === 'RUNNING' ? 'Awaiting completion' : 'Unknown'}
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

        <details className="compact-details"><summary>Execution ID · …{execution.id.slice(-12)}</summary><Code block>{execution.id}</Code></details>
        <SimpleGrid cols={{ base: 2, md: 4 }} spacing="sm" mt="sm">
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
      {runtime?.surface === 'transcript' && <Text size="xs" c="dimmed" mb="sm">Native transcript · turn boundaries from the rollout · process liveness unknown</Text>}
      {data.usage && <Card withBorder mb="md">
        <details className="compact-details"><summary>Token usage · {data.usage.total_tokens.toLocaleString()} total · {data.usage.input_tokens.toLocaleString()} in / {data.usage.output_tokens.toLocaleString()} out</summary>
        <SimpleGrid cols={{base:2,md:5}}>
          {[
            ['Input', data.usage.input_tokens], ['Cached input', data.usage.cached_input_tokens],
            ['Output', data.usage.output_tokens], ['Reasoning output', data.usage.reasoning_output_tokens],
            ['Total', data.usage.total_tokens],
          ].map(([label,value]) => <div key={label}><Text size="xs" c="dimmed">{label}</Text><Text fw={600}>{Number(value).toLocaleString()}</Text></div>)}
        </SimpleGrid>
        <Text size="xs" c="dimmed" mt="sm">Observed counters may be incomplete. Cached input is part of input; reasoning output is part of output. These numbers are not a cost estimate.</Text></details>
      </Card>}
      <Tabs value={activeTab} onChange={setActiveTab} keepMounted>

        <Tabs.List mb="sm">
          <Tabs.Tab value="review">Review</Tabs.Tab>
          <Tabs.Tab value="components" leftSection={<IconServer size={16} />}>
            Components ({components.length})
          </Tabs.Tab>
          <Tabs.Tab value="git" leftSection={<IconFileDiff size={16} />}>
            Git ({git_snapshots.length})
          </Tabs.Tab>
          <Tabs.Tab value="agents" leftSection={<IconRobot size={16} />}>
            Agents ({agents.length})
          </Tabs.Tab>
          <Tabs.Tab value="events" leftSection={<IconListDetails size={16} />}>
            Events ({events_total.toLocaleString()})
          </Tabs.Tab>
        </Tabs.List>

        <Tabs.Panel value="review"><ExecutionReview key={execution.id} id={execution.id} /></Tabs.Panel>
        {/* 1. Components Tab */}
        <Tabs.Panel value="components">
          <SimpleGrid cols={{ base: 1, md: 3 }} spacing="sm">
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
          <SimpleGrid cols={{ base: 1, md: 2 }} spacing="sm">
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
              <Table.ScrollContainer minWidth={650}><Table>
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
              </Table></Table.ScrollContainer>
            )}
          </Card>
        </Tabs.Panel>

        {/* 4. Events Timeline Tab */}
        <Tabs.Panel value="events">
          <EventTimeline key={execution.id} id={execution.id} enabled={activeTab === "events"} />
        </Tabs.Panel>
      </Tabs>
    </div>
  );
};
