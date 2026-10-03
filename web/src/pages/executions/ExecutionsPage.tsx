import { useQuery, keepPreviousData } from '@tanstack/react-query';
import { Link, useLocation, useSearchParams } from 'react-router-dom';
import { Alert, Badge, Button, Card, Group, Loader, Pagination, Select, Table, Text, Title } from '@mantine/core';
import dayjs from 'dayjs';
import { fetchExecutions, ExecutionFilterParams } from '../../shared/api/client';
import { StatusBadge, AttributionBadge } from '../../shared/ui/Badges';
import { ExecutionFilters } from '../../features/ExecutionFilters';
import { useRefreshInterval } from '../../shared/preferences';
const duration = (ms: number | null) => ms == null ? '—' : ms < 60000 ? `${(ms/1000).toFixed(1)}s` : `${(ms/60000).toFixed(1)}m`;
const leaf = (s: string | null) => s?.split(/[\\/]/).pop() || 'Unknown project';
export function ExecutionsPage() {
  const [params,setParams] = useSearchParams();
  const location = useLocation();
  const filters: ExecutionFilterParams = Object.fromEntries(params);
  filters.page = Math.max(1,Number(params.get('page')) || 1);
  filters.page_size = [15,30,50].includes(Number(params.get('page_size'))) ? Number(params.get('page_size')) : 15;
  const setFilters = (value:ExecutionFilterParams) => { const p = new URLSearchParams(); Object.entries(value).forEach(([k,v])=>{if(v!==undefined && v!=='' && v!=='ALL') p.set(k,String(v));}); setParams(p,{replace:true}); };
  const query = useQuery({queryKey:['executions',filters],queryFn:()=>fetchExecutions(filters),refetchInterval:useRefreshInterval(),placeholderData:keepPreviousData});
  const data = query.data;
  const linkState = {from:location.pathname+location.search};
  return <div className="panel-page">
    <div className="page-heading"><div><Title order={2}>Executions <Text component="span" size="sm" c="dimmed">{data?.total ?? '…'}</Text></Title><Text size="sm" c="dimmed">Process observations and native turns. Open a record to review its outcome.</Text></div><Button variant="default" size="xs" onClick={()=>query.refetch()} loading={query.isFetching}>Refresh</Button></div>
    <ExecutionFilters filters={filters} onChange={setFilters} onReset={()=>setFilters({page:1,page_size:filters.page_size})}/>
    {query.error && <Alert color="red" mb="sm">{query.error.message}</Alert>}
    <Card withBorder p={0} className="records-shell" aria-busy={query.isFetching}>
      {query.isLoading ? <Loader m="md" aria-label="Loading executions"/> : !data?.items.length ? <Text p="lg" c="dimmed">No executions match these filters. Widen the period or reset filters.</Text> : <>
        <div className="desktop-records"><Table.ScrollContainer minWidth={780}><Table highlightOnHover><Table.Thead><Table.Tr>{['Execution / model','Scope','Project / branch','Conversation','Duration','Status'].map(h=><Table.Th key={h}>{h}</Table.Th>)}</Table.Tr></Table.Thead><Table.Tbody>
          {data.items.map(e=><Table.Tr key={e.id}><Table.Td><Link className="record-title" to={`/executions/${encodeURIComponent(e.id)}`} state={linkState}>{e.prompt_summary || e.model || 'Unknown model'}</Link><Text size="xs" c="dimmed">{dayjs(e.started_at).format('MMM D, HH:mm:ss')} · {e.reasoning_effort === 'UNKNOWN' ? 'Effort unknown' : e.reasoning_effort}{e.prompt_summary && ` · ${e.model}`}</Text></Table.Td>
            <Table.Td><Badge size="xs" variant="outline" color="cyan">{e.capture_scope}</Badge></Table.Td>
            <Table.Td><Text size="xs" title={e.repo_root || undefined}>{leaf(e.repo_root)}</Text><Text size="xs" c="dimmed" lineClamp={1} maw={180} title={e.branch || undefined}>{e.branch || 'No branch'}</Text>{e.git_attribution==='AMBIGUOUS' && <AttributionBadge attribution={e.git_attribution}/>}</Table.Td>
            <Table.Td><Link className="record-title" to={`/sessions/${encodeURIComponent(e.session_id)}`} title={e.session_id}>…{e.session_id.slice(-10)}</Link>{e.native_execution_id!=='UNKNOWN' && <Text size="xs" c="dimmed" maw={140} truncate title={e.native_execution_id}>{e.native_execution_id}</Text>}</Table.Td>
            <Table.Td>{duration(e.duration_ms)}</Table.Td><Table.Td><StatusBadge status={e.status}/></Table.Td></Table.Tr>)}
        </Table.Tbody></Table></Table.ScrollContainer></div>
        <div className="mobile-records">{data.items.map(e=><article key={e.id} className="record-card"><Group justify="space-between" mb={4}><Text size="xs" c="dimmed">{dayjs(e.started_at).format('MMM D, HH:mm')} · {duration(e.duration_ms)}</Text><StatusBadge status={e.status}/></Group><Link className="record-title" to={`/executions/${encodeURIComponent(e.id)}`} state={linkState}>{e.prompt_summary || e.model || 'Unknown model'}</Link><Group gap={6} mt={6}><Badge size="xs" variant="outline">{e.capture_scope}</Badge><Text size="xs" c="dimmed" lineClamp={1}>{leaf(e.repo_root)}{e.branch && ` · ${e.branch}`}</Text></Group></article>)}</div>
      </>}
    </Card>
    {data && <Group justify="space-between" mt="sm"><Group gap="xs"><Text size="xs" c="dimmed">{data.total ? `${(data.page-1)*data.page_size+1}–${Math.min(data.page*data.page_size,data.total)} of ${data.total}` : '0 records'}</Text><Select aria-label="Rows per page" size="xs" w={85} allowDeselect={false} data={['15','30','50']} value={String(filters.page_size)} onChange={v=>setFilters({...filters,page_size:Number(v),page:1})}/></Group>{data.total_pages>1 && <Pagination size="sm" siblings={0} total={data.total_pages} value={filters.page} onChange={page=>setFilters({...filters,page})}/>}</Group>}
  </div>;
}
