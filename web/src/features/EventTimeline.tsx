import { useState } from 'react';
import { useDebouncedValue } from '@mantine/hooks';
import { Alert, Button, Card, Code, Group, Loader, Stack, Text, TextInput } from '@mantine/core';
import { useQuery } from '@tanstack/react-query';
import dayjs from 'dayjs';
import { EventItem } from '../entities/execution/types';
import { useRefreshInterval } from '../shared/preferences';

interface EventPage { items: EventItem[]; next_cursor: number | null; total: number }

export function EventTimeline({ id, enabled }: { id: string; enabled: boolean }) {
  const [search, setSearch] = useState('');
  const [debounced] = useDebouncedValue(search, 250);
  // A filter gets its own navigation state; changing it always starts at the newest page.
  return <Card withBorder p="sm">
    <TextInput size="xs" aria-label="Search events" placeholder="Search all events by type or source…"
      maxLength={50} value={search} onChange={e => setSearch(e.currentTarget.value)} mb="xs" />
    <EventPages key={`${id}:${debounced}`} id={id} search={debounced} enabled={enabled} />
  </Card>;
}

function EventPages({ id, search, enabled }: { id: string; search: string; enabled: boolean }) {
  const [cursors, setCursors] = useState<(number | null)[]>([null]);
  const cursor = cursors[cursors.length - 1];
  const interval = useRefreshInterval();
  const { data, error, isLoading, isFetching, refetch } = useQuery<EventPage>({
    queryKey: ['execution-events', id, search, cursor],
    queryFn: async ({ signal }) => {
      const params = new URLSearchParams({ limit: '50', search });
      if (cursor !== null) params.set('before_id', String(cursor));
      const response = await fetch(`/api/v1/executions/${encodeURIComponent(id)}/events?${params}`, { signal });
      if (!response.ok) throw new Error('Could not load events');
      return response.json();
    },
    enabled,
    refetchInterval: enabled && cursor === null ? interval : false,
  });
  const navigation = <Group justify="space-between" gap="xs" wrap="wrap">
    <Text size="xs" c="dimmed">{data ? `${data.total.toLocaleString()} matches · page ${cursors.length}` : 'Event history'}</Text>
    <Group gap={4}>
      <Button size="compact-xs" variant="subtle" disabled={cursor === null} onClick={() => setCursors([null])}>Latest</Button>
      <Button size="compact-xs" variant="default" disabled={cursors.length === 1 || isFetching} onClick={() => setCursors(c => c.slice(0, -1))}>Newer</Button>
      <Button size="compact-xs" variant="default" disabled={!data?.next_cursor || isFetching} onClick={() => { if (data?.next_cursor) setCursors(c => [...c, data.next_cursor]); }}>Older</Button>
    </Group>
  </Group>;
  return <Stack gap="xs">
    {navigation}
    <Text size="xs" c="dimmed">Newest arrivals first. Delayed events retain their original timestamps.</Text>
    {isLoading && <Loader size="sm" />}
    {error && <Alert color="red">Unable to load this page. <Button size="compact-xs" onClick={() => refetch()}>Retry</Button></Alert>}
    <div className="event-list">{data?.items.map(event => <details className="compact-details" key={event.event_id}>
      <summary><strong>{event.event_type}</strong> · {dayjs(event.timestamp).format('MMM D HH:mm:ss.SSS')} · {event.source}</summary>
      <Text size="xs" c="dimmed">{event.event_id}</Text>
      <Code block>{event.payload_json || '{}'}</Code>
    </details>)}</div>
    {data?.items.length === 0 && <Text size="sm" c="dimmed">{search ? 'No events match this search.' : 'No events recorded.'}</Text>}
    {(data?.items.length ?? 0) > 10 && navigation}
  </Stack>;
}
