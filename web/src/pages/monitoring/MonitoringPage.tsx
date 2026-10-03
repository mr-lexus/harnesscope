import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Anchor, Badge, Button, Card, Group, Loader, SimpleGrid, Stack, Switch, Text, Title } from '@mantine/core';
import { fetchMonitoring, retryDelivery } from '../../shared/api/monitoring';
import { useRefreshInterval } from '../../shared/preferences';
const stamp = (v: string | number | null) => v ? new Date(v).toLocaleString() : 'Not observed';
export function MonitoringPage() {
  const [all,setAll] = useState(false);
  const client = useQueryClient();
  const query = useQuery({queryKey:['monitoring'],queryFn:fetchMonitoring,refetchInterval:useRefreshInterval()});
  const retry = useMutation({mutationFn:retryDelivery,onSuccess:()=>client.invalidateQueries({queryKey:['monitoring']})});
  const data = query.data;
  const active = data?.runtimes.filter(r=>r.status==='RUNNING') ?? [];
  return <div className="panel-page">
    <div className="page-heading"><div><Title order={2}>Monitor</Title><Text size="sm" c="dimmed">Process observations and durable event delivery.</Text></div><Button size="xs" variant="default" loading={query.isFetching} onClick={()=>query.refetch()}>Refresh</Button></div>
    {(query.error || retry.error) && <Alert color="red" mb="sm">{(query.error ?? retry.error)?.message}</Alert>}
    {query.isLoading && <Loader aria-label="Loading monitoring"/>}
    {data && <>
      <SimpleGrid cols={{base:2,sm:4}} spacing="sm" mb="md">
        {[[active.filter(r=>r.freshness==='FRESH').length,'Fresh processes','teal'],[active.filter(r=>r.freshness==='STALE').length,'Stale observations','orange'],[data.pending_events,'Queued events',data.pending_events ? 'orange':'teal'],[data.blocked_batches,'Blocked batches',data.blocked_batches ? 'red':'gray']].map(([value,label,color])=><Card withBorder p="sm" key={label}><Text size="xs" c="dimmed">{label}</Text><Text size="xl" fw={650} c={String(color)}>{value}</Text></Card>)}
      </SimpleGrid>
      <Card withBorder p="sm" mb="md"><Group justify="space-between" mb="xs"><Title order={3}>Processes</Title><Switch size="sm" label="Include ended" checked={all} onChange={e=>setAll(e.currentTarget.checked)}/></Group>
        <Text size="xs" c="dimmed" mb="sm">Heartbeat every {data.heartbeat_seconds}s · stale after {data.stale_after_seconds}s. A stale signal means observation was lost; the task result stays unchanged.</Text>
        <Stack gap="xs">{(all ? data.runtimes : active).map(r=><div className="record-card" key={r.runtime_id}>
          <Group justify="space-between"><Group gap="xs"><Text fw={600} size="sm">{r.runner}</Text><Text size="xs" c="dimmed">PID {r.pid ?? 'unknown'}</Text></Group><Badge variant="light" color={r.freshness==='FRESH' ? 'teal':r.freshness==='STALE' || r.freshness==='CLOCK_SKEW' ? 'orange':'gray'}>{r.freshness.replace('_',' ')}</Badge></Group>
          <Text size="xs" c="dimmed" style={{overflowWrap:'anywhere'}}>{r.cwd || 'Directory unknown'}</Text>
          <Group justify="space-between" gap="xs"><Text size="xs">Last observed: {stamp(r.observed_at)}</Text>{r.execution_id && <Anchor size="xs" component={Link} to={`/executions/${encodeURIComponent(r.execution_id)}`}>Open execution</Anchor>}</Group>
          <details className="compact-details"><summary>Runtime details</summary><Text size="xs" style={{overflowWrap:'anywhere'}}>{r.runtime_id} · {r.status}</Text><Text size="xs">Received: {stamp(r.received_at)}</Text></details>
        </div>)}{(all ? data.runtimes : active).length===0 && <Text size="sm" c="dimmed" py="sm">{all ? 'No process observations yet. Run an agent through a Harnesscope wrapper.' : 'No processes awaiting completion.'}</Text>}</Stack>
        <Text size="xs" c="dimmed" mt="sm">Up to 200 processes, unfinished first. Native transcripts appear in Sources and do not imply a live process.</Text>
      </Card>
      <Card withBorder p="sm"><Group justify="space-between" mb="xs"><Title order={3}>Delivery queue · {data.pending_batches} batches</Title><Button size="xs" variant="light" disabled={!data.queue_available || !data.pending_batches} loading={retry.isPending} onClick={()=>retry.mutate()}>Retry now</Button></Group>
        {!data.queue_available ? <Alert color="orange">Durable queue is unavailable.</Alert> : !data.pending_batches ? <Text size="sm" c="teal">All queued events delivered.</Text> : <Stack gap="xs">{data.batches.map(b=><div className="record-card" key={b.id}>
          <Group justify="space-between"><Text size="sm" fw={600}>{b.events} events · batch {b.id}</Text><Badge color={b.blocked ? 'red':'orange'} variant="light">{b.blocked ? 'Blocked':'Waiting'}</Badge></Group>
          <Text size="xs" c="dimmed">Queued {stamp(b.created_at)} · {b.attempts} attempts</Text>{b.last_error && <Text size="xs" c="orange">{b.last_error}</Text>}
          {b.next_attempt>0 && !b.blocked && <Text size="xs">Next attempt: {stamp(b.next_attempt)}</Text>}
        </div>)}</Stack>}
        <Text size="xs" c="dimmed" mt="sm">Events stay on disk until confirmed. Retries preserve order within each process. Showing the first 100 pending batches for this database and server address.</Text>
      </Card>
    </>}
  </div>;
}
