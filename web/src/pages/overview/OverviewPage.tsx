import { useRefreshInterval } from '../../shared/preferences';
import { Link, useSearchParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { Alert, Anchor, Badge, Button, Card, Code, Container, Group, Loader, Progress, Select, SimpleGrid, Stack, Switch, Table, Text, Title } from '@mantine/core';
import { IconArrowUpRight, IconDownload, IconRefresh } from '@tabler/icons-react';
import { downloadReport, fetchRetrospective, RetrospectiveFilter } from '../../shared/api/retrospective';

const percentage = (part: number, whole: number) => whole ? `${Math.round(part / whole * 100)}%` : '—';
const duration = (ms: number | null) => ms === null ? 'Unknown' : ms < 60_000 ? `${(ms / 1000).toFixed(1)}s` : `${(ms / 60_000).toFixed(1)}m`;

export function OverviewPage() {
  const [params, setParams] = useSearchParams();
  const days = Number(params.get('days') ?? 30);
  const filters: RetrospectiveFilter = {
    days: [7,30,90,3650].includes(days) ? days : 30,
    include_demo: params.get('include_demo') === 'true',
    runner: params.get('runner') || undefined,
    project: params.get('project') || undefined,
    scope: params.get('scope') || undefined,
  };
  const setFilters = (update: (current: RetrospectiveFilter) => RetrospectiveFilter) => {
    const next = new URLSearchParams();
    Object.entries(update(filters)).forEach(([key,value]) => { if (value !== undefined) next.set(key,String(value)); });
    setParams(next, {replace:true});
  };
  const query = useQuery({ queryKey: ['retrospective', filters], queryFn: () => fetchRetrospective(filters), refetchInterval: useRefreshInterval() });
  const data = query.data;
  return <Container fluid className="panel-page">
    <Group justify="space-between" align="flex-start" mb="md">
      <div><Title order={2}>Overview</Title><Text size="sm" c="dimmed">Review outcomes. Compare experiments. Improve the next run.</Text></div>
      <Button variant="default" leftSection={<IconDownload size={16} />} disabled={!data || query.isError} onClick={() => data && downloadReport(data, filters)}>Export report</Button>
    </Group>
    <Card withBorder radius="md" mb="md">
      <div className="filter-grid">
        <Select size="xs" label="Period" value={String(filters.days)} data={[{value:'7',label:'Last 7 days'},{value:'30',label:'Last 30 days'},{value:'90',label:'Last 90 days'},{value:'3650',label:'Last 10 years'}]} onChange={v => setFilters(f => ({...f,days:Number(v ?? 30)}))} />
        <Select size="xs" label="Runner" placeholder="All runners" clearable searchable data={data?.runners ?? []} value={filters.runner ?? null} onChange={v => setFilters(f => ({...f,runner:v ?? undefined}))} />
        <Select size="xs" label="Project / launch directory" placeholder="All projects" clearable searchable data={data?.projects ?? []} value={filters.project ?? null} onChange={v => setFilters(f => ({...f,project:v ?? undefined}))} />
        <Select size="xs" label="Observation scope" placeholder="All scopes" clearable data={['PROCESS','TURN','UNKNOWN']} value={filters.scope ?? null} onChange={v => setFilters(f => ({...f,scope:v ?? undefined}))} />
      </div><Group mt="sm" justify="space-between">
        <Switch size="xs" label="Include demo" checked={filters.include_demo} onChange={e => { const checked = e.currentTarget.checked; setFilters(f => ({...f,include_demo:checked})); }} />
      </Group>
    </Card>
    {query.isError && <Alert color="red" title="Could not refresh the overview" mb="md">{query.error.message} <Button size="xs" variant="subtle" onClick={() => query.refetch()}>Retry</Button></Alert>}
    {query.isLoading && <Group justify="center" p="xl"><Loader aria-label="Loading retrospective" /></Group>}
    {data && <>
      <SimpleGrid cols={{base:2,md:4}} mb="md">
        {[
          ['Recorded executions',data.total,`${data.process_captures} process · ${data.turn_captures} turn · ${data.unknown_scope} unknown${filters.include_demo ? ` · ${data.total-data.process_captures-data.turn_captures-data.unknown_scope} demo` : ''}`],
          ['Awaiting completion',data.running,'A running record is not a heartbeat'],
          ['Outcome coverage',percentage(data.reviewed,data.total),`${data.reviewed} reviewed of ${data.total} recorded`],
          ['Accepted by you',percentage(data.accepted,data.reviewed),`${data.accepted} accepted of ${data.reviewed} reviewed`],
        ].map(([label,value,caption]) => <Card key={label} withBorder radius="md" p="sm"><Text size="sm" c="dimmed">{label}</Text><Title order={2} my={4}>{value}</Title><Text size="xs" c="dimmed">{caption}</Text></Card>)}
      </SimpleGrid>
      {data.total === 0 ? <Card withBorder radius="md" p="xl" mb="md">
        <Title order={3}>Connect Codex to start your history.</Title>
        <Text c="dimmed" mt="sm" mb="md">Connect your local Codex sessions folder once. Existing history and new turns will appear automatically while Harnesscope is running. If you already have records, widen the filters.</Text>
        <Button component={Link} to="/sources" mb="md">Set up collection</Button><Text size="xs" c="dimmed" mb="xs">Optional: observe process lifetimes for other tools through the wrapper.</Text><Stack gap="xs"><Code block>harnesscope codex</Code><Code block>harnesscope run --runner claude claude</Code><Code block>harnesscope ingest --file events.jsonl</Code></Stack>
        <Text size="sm" c="dimmed" mt="md">Wrappers observe process lifetimes. Turn-level records require an adapter that emits actual turn boundaries. Imports use the Harnesscope event format.</Text>
      </Card> : <Card withBorder radius="md" mb="md">
        <Group justify="space-between" mb="xs"><Title order={3}>Experiments</Title><Badge variant="light">{data.cohorts.length} groups</Badge></Group>
        <Text size="sm" c="dimmed" mb="md">Grouped by runner, model, scope and experiment. Acceptance comes from your review.</Text>
        <div className="desktop-records"><Table.ScrollContainer minWidth={900}><Table verticalSpacing="sm" highlightOnHover><Table.Thead><Table.Tr>
          {['Runner / model','Scope','Experiment','Runs','Failed','Reviewed','Accepted','Avg duration'].map(h => <Table.Th key={h}>{h}</Table.Th>)}
        </Table.Tr></Table.Thead><Table.Tbody>{data.cohorts.map((c,i) => <Table.Tr key={i}>
          <Table.Td><Text fw={600} size="sm">{c.runner}</Text><Text size="xs" c="dimmed">{c.model}</Text></Table.Td>
          <Table.Td><Badge variant="outline" color={c.scope === 'TURN' ? 'teal' : 'gray'}>{c.scope}</Badge></Table.Td>
          <Table.Td>{c.experiment || <Text size="sm" c="dimmed">Unlabelled</Text>}</Table.Td>
          <Table.Td>{c.executions}</Table.Td><Table.Td>{c.failed}</Table.Td><Table.Td>{c.reviewed} / {c.executions}</Table.Td>
          <Table.Td>{percentage(c.accepted,c.reviewed)}<Text size="xs" c="dimmed">{c.rework} rework · {c.rejected} rejected</Text></Table.Td><Table.Td>{duration(c.avg_duration_ms)}</Table.Td>
        </Table.Tr>)}</Table.Tbody></Table></Table.ScrollContainer></div>
        <div className="mobile-records">{data.cohorts.map((c,i)=><div className="record-card" key={i}><Group justify="space-between"><Text fw={600} size="sm">{c.runner} · {c.model}</Text><Badge size="xs" variant="outline">{c.scope}</Badge></Group><Text size="xs" c="dimmed">{c.experiment || 'Unlabelled'} · {c.executions} runs · {duration(c.avg_duration_ms)} avg</Text><Group justify="space-between" mt={4}><Text size="xs">{c.reviewed} reviewed · {c.failed} failed</Text><Text size="xs" fw={600}>{percentage(c.accepted,c.reviewed)} accepted</Text></Group><Text size="xs" c="dimmed">{c.rework} rework · {c.rejected} rejected</Text></div>)}</div>
        <Text size="xs" c="dimmed" mt="sm">Small or different task samples are not a model ranking. Compare similar tasks and review more than one run before changing your workflow.</Text>
      </Card>}
      <SimpleGrid cols={{base:1,md:2}}>
        <Card withBorder radius="md"><Title order={3} mb="xs">Review queue</Title><Text c="dimmed" size="sm" mb="md">Up to 20 running, failed, overlapping or unreviewed executions.</Text>
          <Stack gap="xs">{data.attention.length === 0 ? <Text c="dimmed" size="sm">Nothing needs review in this selection.</Text> : data.attention.map(item => <Group key={item.id} justify="space-between" wrap="nowrap">
            <div style={{minWidth:0}}><Anchor component={Link} to={`/executions/${encodeURIComponent(item.id)}`} size="sm" fw={600} lineClamp={1}>{item.prompt_summary || `${item.runner} · …${item.id.slice(-10)}`}</Anchor><Text size="xs" c="dimmed">{item.reason}</Text></div>
            <Badge color={item.status === 'FAILED' ? 'red' : 'gray'} variant="light">{item.status}</Badge>
          </Group>)}</Stack>
        </Card>
        <Card withBorder radius="md"><Title order={3} mb="xs">Evidence coverage</Title><Text size="sm" c="dimmed" mb="md">Missing evidence remains unknown.</Text>
          <Stack gap="sm">{[['Model identified',data.total-data.unknown_model],['Native conversation identified',data.total-data.unknown_session],['Observation scope identified',data.total-data.unknown_scope]].map(([label,value]) => <div key={label}><Group justify="space-between" mb={6}><Text size="sm">{label}</Text><Text size="sm" fw={600}>{percentage(Number(value),data.total)}</Text></Group><Progress value={data.total ? Number(value)/data.total*100 : 0} color="teal" /></div>)}</Stack>
          <Text size="sm" mt="sm">{data.ambiguous_git} executions have ambiguous Git attribution.</Text>
          <Text size="xs" c="dimmed" mt="xs">Worktree snapshots show observed changes, not proof that an agent authored them.</Text>
          <Button component={Link} to="/sessions" variant="subtle" rightSection={<IconArrowUpRight size={16}/>} mt="md">Explore conversations</Button>
        </Card>
      </SimpleGrid>
      <Group justify="space-between" mt="sm"><Text size="xs" c="dimmed">Last event in database: {data.last_event_at ? new Date(data.last_event_at).toLocaleString() : 'none'}</Text><Button size="xs" variant="subtle" leftSection={<IconRefresh size={14}/>} loading={query.isFetching} onClick={() => query.refetch()}>Refresh</Button></Group>
    </>}
  </Container>;
}
