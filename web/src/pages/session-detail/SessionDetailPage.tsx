import { useRefreshInterval } from '../../shared/preferences';
import React from 'react';
import { useParams, useNavigate, Link } from 'react-router-dom';
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
  Code,
  Box,
  Divider,
} from '@mantine/core';
import dayjs from 'dayjs';
import { fetchSessionDetail } from '../../shared/api/client';
import { StatusBadge, UnknownText } from '../../shared/ui/Badges';
import {
  IconArrowLeft,
  IconHistory,
  IconDeviceDesktop,
  IconTerminal2,
  IconPlayerPlay,
} from '@tabler/icons-react';

export const SessionDetailPage: React.FC = () => {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();

  const { data, isLoading, error } = useQuery({
    queryKey: ['session', id],
    queryFn: () => fetchSessionDetail(id!),
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
          <Text c="red" size="lg">Session not found or failed to load.</Text>
          <Button variant="outline" onClick={() => navigate('/sessions')}>
            Back to Sessions
          </Button>
        </Stack>
      </Center>
    );
  }

  const { session, bindings, executions, child_forks = [], conflicts = [] } = data;

  return (
    <div className="panel-page">
      <Button
        variant="subtle"
        leftSection={<IconArrowLeft size={16} />}
        onClick={() => navigate('/sessions')}
        mb="md"
      >
        Back to Sessions
      </Button>

      {/* Conflicts Alert Card */}
      {conflicts.length > 0 && (
        <Card withBorder radius="md" p="md" mb="md" style={{ borderColor: 'var(--mantine-color-red-6)', backgroundColor: 'rgba(255, 0, 0, 0.05)' }}>
          <Group justify="space-between" mb="xs">
            <Group gap="xs">
              <span style={{ fontSize: '20px' }}>⚠️</span>
              <Title order={4} c="red">
                Session Conflicts Detected ({conflicts.length})
              </Title>
            </Group>
            <Badge color="red" variant="filled">
              Review overlaps
            </Badge>
          </Group>
          <Text size="xs" c="dimmed" mb="sm">
            Harnesscope monitors session-level access across all agent instances, detecting concurrent access and divergence.
          </Text>
          <Stack gap="xs">
            {conflicts.map((c) => (
              <Box key={c.id} p="sm" style={{ border: '1px solid var(--mantine-color-default-border)', borderRadius: '6px', backgroundColor: 'var(--mantine-color-body)' }}>
                <Group justify="space-between">
                  <Group gap="xs">
                    <Badge color={c.severity === 'CRITICAL' ? 'red' : 'yellow'} variant="light">
                      {c.conflict_type}
                    </Badge>
                    <Badge variant="outline" color={c.resolved_at ? 'gray' : 'orange'}>{c.resolved_at ? 'Overlap ended' : 'Open'}</Badge>
                    <Text size="xs" c="dimmed">Detected: {dayjs(c.detected_at).format('YYYY-MM-DD HH:mm:ss')}</Text>
                  </Group>
                  {c.conflicting_session_id && (
                    <Button variant="subtle" size="xs" onClick={() => navigate(`/sessions/${c.conflicting_session_id}`)}>
                      View Conflicting Session
                    </Button>
                  )}
                </Group>
                {c.details_json && (
                  <Code block mt="xs" style={{ fontSize: '11px' }}>
                    {c.details_json}
                  </Code>
                )}
              </Box>
            ))}
          </Stack>
        </Card>
      )}

      {/* Session Info Card */}
      <Card withBorder radius="md" p="sm" mb="md">
        <Group justify="space-between" align="flex-start">
          <div>
            <Group gap="sm">
              <Title order={3}>{session.title || 'Untitled Session'}</Title>
              <StatusBadge status={session.status} />
              <Badge color="cyan" variant="outline">
                {session.runner_name}
              </Badge>
            </Group>
            <Group gap="xs" mt="xs">
              <Text size="sm" c="dimmed">Native Session ID:</Text>
              <Code style={{ fontSize: '12px' }}>
                <UnknownText value={session.native_session_id} />
              </Code>
            </Group>
            <Text size="xs" c="dimmed" mt={4}>
              Internal ID: {session.id} | Created: {dayjs(session.created_at).format('YYYY-MM-DD HH:mm:ss')}
            </Text>

            {/* Parent Fork Info */}
            {session.parent_session_id && (
              <Box mt="sm" p="xs" style={{ border: '1px dashed var(--mantine-color-blue-4)', borderRadius: '6px' }}>
                <Group justify="space-between">
                  <Group gap="xs">
                    <Text size="xs" fw={600} c="blue">
                      🔀 Forked from Parent Session:
                    </Text>
                    <Code style={{ fontSize: '11px' }}>{session.parent_session_id}</Code>
                    {session.fork_reason && (
                      <Badge size="xs" variant="light" color="indigo">
                        Reason: {session.fork_reason}
                      </Badge>
                    )}
                  </Group>
                  <Button
                    size="compact-xs"
                    variant="light"
                    onClick={() => navigate(`/sessions/${session.parent_session_id}`)}
                  >
                    Open Parent Session
                  </Button>
                </Group>
              </Box>
            )}
          </div>

          <Group gap="xs">
            <Badge size="lg" variant="light" color="indigo" leftSection={<IconHistory size={16} />}>
              {bindings.length} Observation Bindings
            </Badge>
            {child_forks.length > 0 && (
              <Badge size="lg" variant="outline" color="grape">
                {child_forks.length} Child Forks
              </Badge>
            )}
          </Group>
        </Group>

        {/* Child Forks Section */}
        {child_forks.length > 0 && (
          <Box mt="md" pt="sm" style={{ borderTop: '1px solid var(--mantine-color-default-border)' }}>
            <Text size="xs" fw={700} tt="uppercase" c="dimmed" mb="xs">
              Branch Lineage / Child Forks ({child_forks.length}):
            </Text>
            <Group gap="xs">
              {child_forks.map((cf) => (
                <Button
                  key={cf.id}
                  variant="default"
                  size="xs"
                  onClick={() => navigate(`/sessions/${cf.id}`)}
                >
                  🌱 {cf.title || cf.native_session_id || cf.id}
                </Button>
              ))}
            </Group>
          </Box>
        )}
      </Card>

      <Title order={4} mb="md">
        Observation history
      </Title>
      <Text size="sm" c="dimmed" mb="md">
        Bindings connect process observations or imported transcripts to this conversation. IMPORT is historical evidence, not a running process.
      </Text>

      <Stack gap="sm">
        {bindings.map((b) => {
          const boundExecutions = executions.filter((e) => e.runtime_id === b.runtime_id);

          return (
            <Card key={b.id} withBorder radius="md" p="md" style={{ position: 'relative' }}>
              <Group justify="space-between" mb="xs">
                <Group gap="sm">
                  <IconDeviceDesktop size={20} color="gray" />
                  <Title order={5}>
                    {b.reason === 'IMPORT' ? 'Transcript' : 'Runtime'} <Text component="span" size="xs" title={b.runtime_id}>…{b.runtime_id.slice(-12)}</Text>
                  </Title>
                  <Badge
                    color={b.reason === 'RESUME' ? 'grape' : 'blue'}
                    variant={b.reason === 'RESUME' ? 'filled' : 'light'}
                    leftSection={b.reason === 'RESUME' ? <IconHistory size={12} /> : undefined}
                  >
                    {b.reason === 'IMPORT' ? 'IMPORTED TRANSCRIPT' : b.reason === 'RESUME' ? 'RESUMED SESSION' : 'INITIAL RUNTIME'}
                  </Badge>
                </Group>
                <Text size="xs" c="dimmed">
                  Bound: {dayjs(b.bound_at).format('HH:mm:ss')} {b.unbound_at ? `— Stopped: ${dayjs(b.unbound_at).format('HH:mm:ss')}` : b.reason === 'IMPORT' ? '— Process status unknown' : '— No stop event'}
                </Text>
              </Group>

              {b.unbound_at && (
                <Text size="xs" c="dimmed" mb="sm">
                  ✓ Process completed / stopped. Session was preserved and ready for resume.
                </Text>
              )}

              <Divider my="xs" label={`Executions in this observation (${boundExecutions.length})`} labelPosition="left" />

              {boundExecutions.length === 0 ? (
                <Text size="xs" c="dimmed" fs="italic">No executions recorded for this binding.</Text>
              ) : (
                <Stack gap="xs">
                  {boundExecutions.map((e) => (
                    <Box
                      key={e.id}
                      p="sm"
                      style={{
                        backgroundColor: 'var(--mantine-color-default-hover)',
                        borderRadius: '6px',
                      }}

                    >
                      <Group justify="space-between">
                        <div>
                          <Group gap="xs">
                            <IconTerminal2 size={16} color="gray" />
                            <Link className="record-title" to={`/executions/${encodeURIComponent(e.id)}`}>{e.capture_scope === 'TURN' ? `Turn #${e.turn_index + 1}` : e.capture_scope} · {e.prompt_summary || e.model}</Link>
                            <StatusBadge status={e.status} />
                          </Group>
                          <Group gap="md" mt={4}>
                            <Text size="xs" c="dimmed">Model: {e.model}</Text>
                            <Text size="xs" c="dimmed">Role: {e.selected_agent_role}</Text>
                            <Text size="xs" c="dimmed">Started: {dayjs(e.started_at).format('HH:mm:ss')}</Text>
                          </Group>
                        </div>
                        <Button component={Link} to={`/executions/${encodeURIComponent(e.id)}`} variant="subtle" size="xs">Open</Button>
                      </Group>
                    </Box>
                  ))}
                </Stack>
              )}
            </Card>
          );
        })}
      </Stack>
    </div>
  );
};
