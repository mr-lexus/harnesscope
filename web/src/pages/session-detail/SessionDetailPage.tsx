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

  const { session, bindings, executions } = data;

  return (
    <div style={{ padding: '24px', maxWidth: '1200px', margin: '0 auto' }}>
      <Button
        variant="subtle"
        leftSection={<IconArrowLeft size={16} />}
        onClick={() => navigate('/sessions')}
        mb="md"
      >
        Back to Sessions
      </Button>

      <Card withBorder radius="md" p="lg" mb="xl">
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
          </div>

          <Badge size="lg" variant="light" color="indigo" leftSection={<IconHistory size={16} />}>
            {bindings.length} Runtimes Bound
          </Badge>
        </Group>
      </Card>

      <Title order={4} mb="md">
        Session Lifecycle & Runtime Bindings
      </Title>
      <Text size="sm" c="dimmed" mb="lg">
        Illustrating session continuity across separate process executions (process stopped != session stopped).
      </Text>

      <Stack gap="lg">
        {bindings.map((b, idx) => {
          const boundExecutions = executions.filter((e) => e.runtime_id === b.runtime_id);

          return (
            <Card key={b.id} withBorder radius="md" p="md" style={{ position: 'relative' }}>
              <Group justify="space-between" mb="xs">
                <Group gap="sm">
                  <IconDeviceDesktop size={20} color="gray" />
                  <Title order={5}>
                    Runtime: <Code>{b.runtime_id}</Code>
                  </Title>
                  <Badge
                    color={b.reason === 'RESUME' ? 'grape' : 'blue'}
                    variant={b.reason === 'RESUME' ? 'filled' : 'light'}
                    leftSection={b.reason === 'RESUME' ? <IconHistory size={12} /> : undefined}
                  >
                    {b.reason === 'RESUME' ? 'RESUMED SESSION' : 'INITIAL RUNTIME'}
                  </Badge>
                </Group>
                <Text size="xs" c="dimmed">
                  Bound: {dayjs(b.bound_at).format('HH:mm:ss')} {b.unbound_at ? `— Stopped: ${dayjs(b.unbound_at).format('HH:mm:ss')}` : '— Active'}
                </Text>
              </Group>

              {b.unbound_at && (
                <Text size="xs" c="dimmed" mb="sm">
                  ✓ Process completed / stopped. Session was preserved and ready for resume.
                </Text>
              )}

              <Divider my="xs" label={`Executions in this runtime (${boundExecutions.length})`} labelPosition="left" />

              {boundExecutions.length === 0 ? (
                <Text size="xs" c="dimmed" fs="italic">No turns recorded for this runtime binding.</Text>
              ) : (
                <Stack gap="xs">
                  {boundExecutions.map((e) => (
                    <Box
                      key={e.id}
                      p="sm"
                      style={{
                        backgroundColor: 'var(--mantine-color-default-hover)',
                        borderRadius: '6px',
                        cursor: 'pointer',
                      }}
                      onClick={() => navigate(`/executions/${e.id}`)}
                    >
                      <Group justify="space-between">
                        <div>
                          <Group gap="xs">
                            <IconTerminal2 size={16} color="gray" />
                            <Text size="sm" fw={600}>
                              Turn #{e.turn_index + 1}: {e.prompt_summary || 'No prompt summary recorded'}
                            </Text>
                            <StatusBadge status={e.status} />
                          </Group>
                          <Group gap="md" mt={4}>
                            <Text size="xs" c="dimmed">Model: {e.model}</Text>
                            <Text size="xs" c="dimmed">Role: {e.selected_agent_role}</Text>
                            <Text size="xs" c="dimmed">Started: {dayjs(e.started_at).format('HH:mm:ss')}</Text>
                          </Group>
                        </div>
                        <Button variant="light" size="xs">View Turn</Button>
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
