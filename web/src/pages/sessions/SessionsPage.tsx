import { useRefreshInterval } from '../../shared/preferences';
import React from 'react';
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
  Select, Pagination, TextInput,
  Code,
} from '@mantine/core';
import { Link, useSearchParams } from 'react-router-dom';
import dayjs from 'dayjs';
import { fetchSessions, SessionFilterParams } from '../../shared/api/client';
import { StatusBadge, UnknownText } from '../../shared/ui/Badges';
import { IconHistory, IconDeviceDesktop, IconPlayerPlay } from '@tabler/icons-react';

export const SessionsPage: React.FC = () => {
  const [params,setParams] = useSearchParams();
  const filters: SessionFilterParams = { runner:params.get('runner') || undefined,status:params.get('status') || undefined,page:Math.max(1,Number(params.get('page')) || 1),page_size:20 };
  const setFilters = (value:SessionFilterParams) => { const p = new URLSearchParams(); Object.entries(value).forEach(([k,v])=>{if(v!==undefined && v!=='' && v!=='ALL') p.set(k,String(v));}); setParams(p,{replace:true}); };

  const { data, isLoading, error } = useQuery({
    queryKey: ['sessions', filters],
    queryFn: () => fetchSessions(filters),
    refetchInterval: useRefreshInterval(),
  });

  return (
    <div className="panel-page">
      <Group justify="space-between" mb="md">
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
          <TextInput label="Runner" placeholder="Any runner (exact name)" size="xs" value={filters.runner || ''} onChange={e=>setFilters({...filters,runner:e.currentTarget.value,page:1})}/>
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

      <Card className="records-shell" withBorder radius="md" p={0} style={{ overflow: 'hidden' }}>
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
          <>
          <div className="desktop-records"><Table.ScrollContainer minWidth={800}>
            <Table highlightOnHover verticalSpacing="sm">
              <Table.Thead>
                <Table.Tr>
                  <Table.Th>Session Title / Native ID</Table.Th>
                  <Table.Th>Runner</Table.Th>
                  <Table.Th>Executions</Table.Th>
                  <Table.Th>Bindings & Resumes</Table.Th>
                  <Table.Th>Started</Table.Th>
                  <Table.Th>Last Active</Table.Th>
                  <Table.Th>Status</Table.Th>
                </Table.Tr>
              </Table.Thead>
              <Table.Tbody>
                {data.items.map((sess) => (
                  <Table.Tr
                    key={sess.id}
                  >
                    <Table.Td>
                      <Group gap="xs" align="center">
                        <Link className="record-title" to={`/sessions/${encodeURIComponent(sess.id)}`}>{sess.title || 'Untitled conversation'}</Link>
                        {sess.parent_session_id && (
                          <Badge size="xs" variant="outline" color="orange">
                            Fork of {sess.parent_session_id.substring(0, 14)}...
                          </Badge>
                        )}
                        {sess.conflicts_count && sess.conflicts_count > 0 ? (
                          <Badge size="xs" variant="filled" color="red">
                            ⚠️ {sess.conflicts_count} Conflict{sess.conflicts_count > 1 ? 's' : ''}
                          </Badge>
                        ) : null}
                      </Group>
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
                        {sess.execution_count} executions
                      </Badge>
                    </Table.Td>

                    <Table.Td>
                      <Group gap="xs">
                        <Badge variant="light" color="gray" leftSection={<IconDeviceDesktop size={12} />}>
                          {sess.runtime_count} bindings
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
          </Table.ScrollContainer></div>
          <div className="mobile-records">{data.items.map(sess=><article className="record-card" key={sess.id}>
            <Group justify="space-between" mb={4}><Badge size="xs" variant="outline">{sess.runner_name}</Badge><StatusBadge status={sess.status}/></Group>
            <Link className="record-title" to={`/sessions/${encodeURIComponent(sess.id)}`}>{sess.title || (sess.native_session_id!=='UNKNOWN' ? sess.native_session_id : 'Untitled conversation')}</Link>
            <Text size="xs" c="dimmed" mt={4}>{sess.execution_count} executions · {sess.runtime_count} bindings · {sess.resume_count} resumes</Text>
            <Text size="xs" c="dimmed">Last active {dayjs(sess.last_active_at).format('MMM D, HH:mm')}</Text>
          </article>)}</div></>
        )}
      </Card>
      {data && data.total_pages>1 && <Group justify="center" mt="sm"><Pagination size="sm" siblings={0} total={data.total_pages} value={filters.page} onChange={page=>setFilters({...filters,page})}/></Group>}
    </div>
  );
};
