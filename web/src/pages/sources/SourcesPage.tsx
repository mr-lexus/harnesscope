import { useRefreshInterval } from '../../shared/preferences';
import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Badge, Button, Card, Checkbox, Code, Container, Drawer, Group, Loader, Progress, Stack, Table, Text, TextInput, Title } from '@mantine/core';
import { addSource, fetchSources, fetchCollection, collectionLabel, removeSource, setSourceEnabled } from '../../shared/api/sources';

const date = (value: string | null) => value ? new Date(value).toLocaleString() : 'Not yet';
const sourcePath = (value: string) => value.replace(/^\\\\\?\\/, '');

export function SourcesPage() {
  const [adding, setAdding] = useState(false);
  const [search,setSearch] = useState('');
  const [path, setPath] = useState('');
  const [content, setContent] = useState(false);
  const client = useQueryClient();
  const query = useQuery({ queryKey: ['sources'], queryFn: fetchSources, refetchInterval: useRefreshInterval() });
  const collection = useQuery({ queryKey: ['collection'], queryFn: fetchCollection, refetchInterval: useRefreshInterval() });
  const refresh = () => { client.invalidateQueries({ queryKey: ['sources'] }); client.invalidateQueries({ queryKey: ['collection'] }); };
  const add = useMutation({
    mutationFn: addSource,
    onSuccess: () => { setPath(''); setContent(false); setAdding(false); refresh(); },
  });
  const change = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) => setSourceEnabled(id, enabled),
    onSuccess: refresh,
  });
  const remove = useMutation({ mutationFn: removeSource, onSuccess: refresh });
  const error = query.error ?? add.error ?? change.error ?? remove.error;

  return <Container fluid className="panel-page">
    <div className="page-heading"><div><Title order={2}>Sources</Title><Text size="sm" c="dimmed">Native turns, tool calls and token usage. Checkpoints survive server restarts.</Text></div><Button size="xs" onClick={()=>setAdding(true)}>Add source</Button></div>
    {collection.data && <Card withBorder mb="sm" p="sm"><Group justify="space-between"><Text fw={600} size="sm">{collectionLabel[collection.data.status]}</Text><Text size="xs" c="dimmed">{collection.data.enabled_sources} active sources · {collection.data.files_inspected} files inspected · {collection.data.converted_events.toLocaleString()} converted events</Text></Group><Text size="xs" c="dimmed" mt={4}>Collection runs in the background while the server is running. Closing this tab or pausing panel refresh does not stop it.</Text></Card>}
    <TextInput size="xs" aria-label="Filter sources by path" placeholder="Filter sources by path…" value={search} onChange={e=>setSearch(e.currentTarget.value)} mb="sm"/>
    {error && <Alert color="red" title="Source operation failed" mb="md">{error.message}</Alert>}
    <Drawer closeButtonProps={{'aria-label':'Close source form'}} classNames={{body:'panel-drawer'}} opened={adding} onClose={()=>setAdding(false)} position="right" title="Connect Codex rollouts" size="md">
      <form onSubmit={event => { event.preventDefault(); add.mutate({ path: path.trim(), include_content: content }); }}>
        <Stack>
          <Title order={3}>Add a Codex source</Title>
          <Text size="sm" c="dimmed">
            Choose one rollout file or a directory containing rollout-*.jsonl files. Use an absolute path on this computer.
            A typical directory is <Code>~/.codex/sessions</Code>; expand ~ to your home directory.
            Existing history and future appended records will be imported.
          </Text>
          {collection.data?.detected_codex_path && <Button variant="light" size="xs" onClick={()=>setPath(collection.data!.detected_codex_path!)}>Use detected Codex folder</Button>}
          <TextInput label="File or directory path" placeholder="Absolute path to a Codex rollout or sessions directory"
            value={path} onChange={event => setPath(event.currentTarget.value)} required />
          <Checkbox label="Include redacted prompt summaries and error messages"
            description="Off by default. Tool arguments, tool outputs, instructions and reasoning text are never copied."
            checked={content} onChange={event => setContent(event.currentTarget.checked)} />
          <Group><Button type="submit" loading={add.isPending} disabled={!path.trim()}>Connect source</Button>
            <Text size="xs" c="dimmed">Local files only · no model calls</Text></Group>
        </Stack>
      </form>
      {add.error && <Alert color="red" mt="sm">{add.error.message}</Alert>}
    </Drawer>
    {query.isLoading && <Loader aria-label="Loading sources" />}
    {query.data?.items.length === 0 && <Alert title="Connect Codex once to start collecting" mb="sm">
      <Text size="sm">Harnesscope can read existing and future local Codex rollouts. Messages, tool arguments and outputs stay out of the database by default.</Text>
      <Button mt="sm" size="xs" onClick={()=>{setPath(collection.data?.detected_codex_path ?? '');setAdding(true);}}>Set up collection</Button>
    </Alert>}
    {query.data && query.data.items.length>0 && !query.data.items.some(({source})=>source.path.toLowerCase().includes(search.toLowerCase())) && <Text size="sm" c="dimmed">No sources match this path.</Text>}
    <Stack gap="sm">
      {query.data?.items.filter(({source})=>source.path.toLowerCase().includes(search.toLowerCase())).map(({ source, files }) => {
        const ordered = [...files].sort((a, b) => Number(b.status === 'ERROR') - Number(a.status === 'ERROR'));
        const unmapped = files.reduce((sum, file) => sum + file.ignored_count, 0);
        const pending = files.filter(file => file.status === 'BACKLOG').length;
        const stale = source.enabled && source.last_scan_at && Date.now() - new Date(source.last_scan_at).getTime() > 30_000;
        return <Card withBorder radius="md" key={source.id}>
          <Group justify="space-between" align="flex-start" wrap="wrap">
            <Stack gap={4} style={{ minWidth: 'min(100%, 320px)', flex: 1 }}>
              <Group><Title order={3}>Codex rollouts</Title>
                <Badge color={!source.enabled ? 'gray' : source.last_error ? 'red' : stale ? 'orange' : 'teal'}>
                  {!source.enabled ? 'Paused' : source.last_error ? 'Needs attention' : stale ? 'Scan overdue' : !source.last_scan_at ? 'Starting' : pending ? 'Importing history' : 'Collecting'}
                </Badge>
                <Badge variant="outline">{source.include_content ? 'Summaries included' : 'Metadata only'}</Badge>
              </Group>
              <Text size="sm" style={{ overflowWrap: 'anywhere' }}>{sourcePath(source.path)}</Text>
              <Text size="xs" c="dimmed">Last scan: {date(source.last_scan_at)} · Last successful scan: {date(source.last_success_at)}</Text>
            </Stack>
            <Group gap="xs">
              <Button size="xs" variant="light" disabled={change.isPending}
                onClick={() => change.mutate({ id: source.id, enabled: !source.enabled })}>{source.enabled ? 'Pause' : 'Resume'}</Button>
              <Button size="xs" color="gray" variant="subtle" disabled={remove.isPending}
                onClick={() => remove.mutate(source.id)}>Disconnect</Button>
            </Group>
          </Group>
          {source.last_error && <Alert color="red" mt="md">{source.last_error}</Alert>}
          <Text size="sm" mt="md">{files.length} files inspected · {pending} with backlog · {unmapped} unmapped records</Text>
          {unmapped > 0 && <Text size="xs" c="orange" mt={4}>Some records could not be mapped to supported evidence. Counts may be incomplete; unmapped records are not silently turned into successful tasks.</Text>}
          <details className="compact-details"><summary>File checkpoints and diagnostics ({files.length})</summary>
          {files.length > 0 && <Table.ScrollContainer minWidth={760} mt="md">
            <Table verticalSpacing="sm"><Table.Thead><Table.Tr>
              <Table.Th>Rollout</Table.Th><Table.Th>Checkpoint</Table.Th><Table.Th>Status</Table.Th><Table.Th>Converted events</Table.Th>
            </Table.Tr></Table.Thead><Table.Tbody>
              {ordered.slice(0, 50).map(file => <Table.Tr key={file.path}>
                <Table.Td maw={480}><Text size="xs" style={{ overflowWrap: 'anywhere' }}>{sourcePath(file.path)}</Text>
                  {file.last_error && <Text size="xs" c="red" mt={4}>{file.last_error}</Text>}</Table.Td>
                <Table.Td><Text size="xs">Line {file.line_number} · {file.byte_offset.toLocaleString()} bytes</Text>
                  <Progress mt={4} value={file.file_size ? Math.min(100, file.byte_offset / file.file_size * 100) : 0} />
                </Table.Td>
                <Table.Td><Badge color={file.status === 'ERROR' ? 'red' : file.status === 'READY' ? 'teal' : 'gray'} variant="light">{file.status}</Badge></Table.Td>
                <Table.Td>{file.events_count}</Table.Td>
              </Table.Tr>)}
            </Table.Tbody></Table>
          </Table.ScrollContainer>}
          {files.length > 50 && <Text size="xs" mt="sm">Showing 50 files, with errors first. Use <Code>harnesscope sources list</Code> for all checkpoints.</Text>}
          <Text size="xs" c="dimmed" mt="md">READY means caught up with the file, not that the agent is alive. WAITING means the last line is unfinished. Pausing finishes the current batch. Disconnecting retains imported telemetry.</Text>
          </details>
        </Card>;
      })}
    </Stack>
  </Container>;
}
