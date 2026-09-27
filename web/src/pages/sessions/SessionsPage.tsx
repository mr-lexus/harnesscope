import React, { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import {
  Table,
  Card,
  Title,
  Text,
  Group,
  Badge,
  Loader,
  Center,
  Select,
  Code,
} from '@mantine/core';
import { useNavigate } from 'react-router-dom';
import dayjs from 'dayjs';
import { fetchSessions, SessionFilterParams } from '../../shared/api/client';
import { StatusBadge, UnknownText } from '../../shared/ui/Badges';
import { IconHistory, IconDeviceDesktop, IconPlayerPlay } from '@tabler/icons-react';

export const SessionsPage: React.FC = () => {
  const navigate = useNavigate();
  const [filters, setFilters] = useState<SessionFilterParams>({
    page: 1,
    page_size: 20,
  });

  const { data, isLoading, error } = useQuery({
    queryKey: ['sessions', filters],
    queryFn: () => fetchSessions(filters),
    refetchInterval: 5000,
  });

  return (
    <div style={{ padding: '24px', maxWidth: '1440px', margin: '0 auto' }}>
      <Group justify="space-between" mb="lg">
        <div>
          <Title order={2}>Sessions</Title>
          <Text c="dimmed" size="sm">
            Logical agent conversations surviving process restarts (Session ≠ Process).
          </Text>
        </div>
        {data && (
          <Badge size="lg" variant="light" color="blue">
            Total: {data.total} sessions
          </Badge>
        )}
      </Group>

      <Card withBorder p="sm" radius="md" mb="md">
        <Group>
          <Select
            label="Filter Runner"
            placeholder="All runners"
            data={[
              { value: 'ALL', label: 'All runners' },
              { value: 'codex', label: 'Codex' },
              { value: 'copilot', label: 'Copilot' },
              { value: 'opencode', label: 'OpenCode' },
            ]}
            value={filters.runner || 'ALL'}
            onChange={(val) => setFilters({ ...filters, runner: val || 'ALL', page: 1 })}
            size="xs"
          />
          <Select
            label="Filter Status"
            placeholder="All statuses"
            data={[
              { value: 'ALL', label: 'All statuses' },
              { value: 'ACTIVE', label: 'ACTIVE' },
              { value: 'COMPLETED', label: 'COMPLETED' },
            ]}
            value={filters.status || 'ALL'}
            onChange={(val) => setFilters({ ...filters, status: val || 'ALL', page: 1 })}
            size="xs"
          />
        </Group>
      </Card>

      <Card withBorder radius="md" p={0} style={{ overflow: 'hidden' }}>
        {isLoading ? (
          <Center p="xl">
            <Loader size="lg" />
          </Center>
        ) : error ? (
          <Center p="xl">
            <Text c="red">Error loading sessions: {(error as Error).message}</Text>
          </Center>
        ) : !data || data.items.length === 0 ? (
          <Center p="xl">
            <Text c="dimmed">No sessions found.</Text>
          </Center>
        ) : (
          <Table.ScrollContainer minWidth={800}>
            <Table highlightOnHover verticalSpacing="sm">
              <Table.Thead>
                <Table.Tr>
                  <Table.Th>Session Title / Native ID</Table.Th>
                  <Table.Th>Runner</Table.Th>
                  <Table.Th>Executions</Table.Th>
                  <Table.Th>Runtimes & Resumes</Table.Th>
                  <Table.Th>Started</Table.Th>
                  <Table.Th>Last Active</Table.Th>
                  <Table.Th>Status</Table.Th>
                </Table.Tr>
              </Table.Thead>
              <Table.Tbody>
                {data.items.map((sess) => (
                  <Table.Tr
                    key={sess.id}
                    style={{ cursor: 'pointer' }}
                    onClick={() => navigate(`/sessions/${sess.id}`)}
                  >
                    <Table.Td>
                      <Text fw={600} size="sm">
                        {sess.title || 'Untitled Conversation'}
                      </Text>
                      <Group gap={6} mt={2}>
                        <Text size="xs" c="dimmed">Native ID:</Text>
                        <Code style={{ fontSize: '11px' }}>
                          <UnknownText value={sess.native_session_id} />
                        </Code>
                      </Group>
                    </Table.Td>

                    <Table.Td>
                      <Badge variant="outline" color="cyan" size="sm">
                        {sess.runner_name}
                      </Badge>
                    </Table.Td>

                    <Table.Td>
                      <Badge variant="light" color="blue" leftSection={<IconPlayerPlay size={12} />}>
                        {sess.execution_count} turns
                      </Badge>
                    </Table.Td>

                    <Table.Td>
                      <Group gap="xs">
                        <Badge variant="light" color="gray" leftSection={<IconDeviceDesktop size={12} />}>
                          {sess.runtime_count} runtimes
                        </Badge>
                        {sess.resume_count > 0 && (
                          <Badge variant="filled" color="grape" size="sm" leftSection={<IconHistory size={12} />}>
                            {sess.resume_count} resumed
                          </Badge>
                        )}
                      </Group>
                    </Table.Td>

                    <Table.Td>
                      <Text size="xs">
                        {dayjs(sess.started_at).format('YYYY-MM-DD HH:mm')}
                      </Text>
                    </Table.Td>

                    <Table.Td>
                      <Text size="xs">
                        {dayjs(sess.last_active_at).format('YYYY-MM-DD HH:mm')}
                      </Text>
                    </Table.Td>

                    <Table.Td>
                      <StatusBadge status={sess.status} />
                    </Table.Td>
                  </Table.Tr>
                ))}
              </Table.Tbody>
            </Table>
          </Table.ScrollContainer>
        )}
      </Card>
    </div>
  );
};
