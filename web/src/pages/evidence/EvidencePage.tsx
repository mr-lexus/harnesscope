import { useState } from 'react';
import { Alert, Badge, Button, Code, Group, Select, Stack, Tabs, Text, TextInput, Title } from '@mantine/core';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

type Row = { sequence:number; id:string; channel:string; kind:string; category:string; role?:string; tool?:string; session_id?:string; turn_id?:string; observed_at?:string; object_hash?:string; disposition:string; reason?:string };
type Page = {items:Row[];next:number|null;through:number};
type Task = {id:string;title:string;links:number;versions:number};
type Version = {cursor:number;id:string;root:string;path:string;observed_at:string;object_hash?:string;disposition:string;state:string};
async function api<T>(path:string, body?:unknown):Promise<T> {
  const response=await fetch(`/api/v1/${path}`,body===undefined ? undefined : {method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});
  if(!response.ok) throw new Error('Cannot load or save evidence. Check source diagnostics and retry.');
  return response.json();
}
function ObjectContent({hash}:{hash:string}) {
  const [offset,setOffset]=useState(0);
  const result=useQuery({queryKey:['object',hash,offset],queryFn:()=>api<{text:string;next:number|null;total_bytes:number}>(`evidence/objects/${hash}/pages?offset=${offset}`)});
  return <Stack gap={4}>
    {result.isPending && <Text size="xs">Loading evidence…</Text>}
    {result.isError && <Alert color="red">{result.error.message}</Alert>}
    {result.data && <><Text size="xs" c="dimmed">Sanitized evidence · {result.data.total_bytes.toLocaleString()} bytes · offset {offset}</Text><Code block style={{maxHeight:360,overflow:'auto',whiteSpace:'pre-wrap',overflowWrap:'anywhere'}}>{result.data.text}</Code><Group gap={4}><Button size="compact-xs" disabled={!offset} onClick={()=>setOffset(0)}>Start</Button><Button size="compact-xs" disabled={result.data.next===null} onClick={()=>setOffset(result.data!.next!)}>Next part</Button></Group></>}
  </Stack>;
}
function Observation({row}:{row:Row}) {
  const [open,setOpen]=useState(false);
  return <details className="evidence-row" onToggle={e=>setOpen(e.currentTarget.open)}>
    <summary aria-label={`${row.tool || row.kind}, ${row.observed_at || "time unavailable"}, ${row.disposition}`}><time>{row.observed_at?.replace('T',' ').slice(0,19) ?? 'Time unavailable'}</time><strong>{row.tool || row.kind}</strong><span>{row.role || row.category}</span><Badge size="xs" color={row.disposition==='excluded'?'orange':'gray'}>{row.disposition}</Badge></summary>
    {open && <Stack gap={6} p="xs"><Text size="xs" c="dimmed">{row.channel} · session {row.session_id || 'unavailable'} · turn {row.turn_id || 'unavailable'}</Text>{row.reason && <Alert color="orange">{row.reason}</Alert>}{row.object_hash && <ObjectContent hash={row.object_hash}/>}</Stack>}
  </details>;
}
function ArtifactVersion({hash,label}:{hash:string;label:string}) {
  const [open,setOpen]=useState(false);
  return <details className="evidence-row" onToggle={e=>setOpen(e.currentTarget.open)}><summary>{label}</summary>{open && <ObjectContent hash={hash}/>}</details>;
}
function VersionDiff({before,after}:{before:string;after:string}) {
  const result=useQuery({queryKey:['evidence-diff',before,after],queryFn:()=>api<{available:boolean;reason?:string;same?:boolean;removed?:string[];added?:string[]}>(`evidence/diff?before=${before}&after=${after}`)});
  if(result.isError)return <Alert color="red">{result.error.message}</Alert>;
  if(!result.data)return <Text size="xs">Comparing…</Text>;
  if(!result.data.available)return <Alert>Diff unavailable: {result.data.reason}. Open each version to compare paginated content.</Alert>;
  return result.data.same?<Text size="xs">Identical content</Text>:<Group align="start" grow><Code block style={{whiteSpace:'pre-wrap',overflowWrap:'anywhere',maxHeight:320,overflow:'auto'}}>{result.data.removed?.map(s=>`− ${s}`).join('\n')}</Code><Code block style={{whiteSpace:'pre-wrap',overflowWrap:'anywhere',maxHeight:320,overflow:'auto'}}>{result.data.added?.map(s=>`+ ${s}`).join('\n')}</Code></Group>;
}
export function EvidencePage() {
  const client=useQueryClient();
  const [task,setTask]=useState<string|null>(null),[session,setSession]=useState(''),[search,setSearch]=useState('');
  const [after,setAfter]=useState(0),[through,setThrough]=useState<number|undefined>();
  const [title,setTitle]=useState(''),[link,setLink]=useState(''),[mark,setMark]=useState<string|null>('accepted'),[note,setNote]=useState(''),[root,setRoot]=useState('');
  const [versionAfter,setVersionAfter]=useState(0);
  const [activeTab,setActiveTab]=useState<string|null>('history');
  const [contextAfter,setContextAfter]=useState(0),[contextThrough,setContextThrough]=useState<number|undefined>();
  const [before,setBefore]=useState<string|null>(null),[afterVersion,setAfterVersion]=useState<string|null>(null);
  const params=new URLSearchParams({after:String(after),limit:'50'});
  if(task)params.set('task',task);if(session)params.set('session',session);if(search)params.set('q',search);if(through!==undefined)params.set('through',String(through));
  const history=useQuery({queryKey:['evidence',params.toString()],queryFn:()=>api<Page>(`evidence?${params}`)});
  const context=useQuery({queryKey:['evidence-context',session,contextAfter,contextThrough],enabled:activeTab==='context',queryFn:()=>api<{items:(Row&{context:{window_limit:{value:number|null};turn_usage?:{total_tokens:number};thread_cumulative_usage?:{total_tokens:number}}})[];next:number|null;through:number}>(`evidence/context?limit=50&after=${contextAfter}${contextThrough===undefined?'':`&through=${contextThrough}`}${session?`&session=${encodeURIComponent(session)}`:''}`)});
  const tasks=useQuery({queryKey:['retro-tasks'],queryFn:()=>api<{items:Task[]}>('tasks?limit=200')});
  const detail=useQuery({queryKey:['retro-task',task],enabled:!!task,queryFn:()=>api<{marks:{kind:string;note:string;created_at:string}[];links:{session_id:string;status:string}[];versions:{id:string;object_hash:string;observed_at:string}[]}>(`tasks/${encodeURIComponent(task!)}`)});
  const workflow=useQuery({queryKey:['workflow',versionAfter],queryFn:()=>api<{items:Version[]}>(`workflow?after=${versionAfter}`)});
  const coverage=useQuery({queryKey:['evidence-coverage'],queryFn:()=>api<{channels:{channel:string;disposition:string;reason?:string;count:number;last_received:string}[];object_bytes:number;source_errors:number}>('evidence/coverage')});
  const save=useMutation({mutationFn:({path,body}:{path:string;body:unknown})=>api(path,body),onSuccess:()=>client.invalidateQueries()});
  const reset=()=>{setAfter(0);setThrough(undefined)};
  const exportParams=new URLSearchParams(params);exportParams.set('through',String(history.data?.through??0));
  return <Stack p="md" gap="xs" className="evidence-page">
    <Group justify="space-between"><Title order={3}>Evidence</Title><Text size="xs" c="dimmed">{coverage.data?.object_bytes.toLocaleString() ?? '…'} bytes · {coverage.data?.source_errors ?? '…'} source errors</Text></Group>
    <Text size="xs" c="dimmed">Observed data for retrospectives. A completed turn does not mean an accepted result.</Text>
    {save.isError && <Alert color="red">{save.error.message}</Alert>}
    {tasks.isError && <Alert color="red">{tasks.error.message}</Alert>}
    {detail.isError && <Alert color="red">{detail.error.message}</Alert>}
    <Tabs value={activeTab} onChange={setActiveTab}><Tabs.List><Tabs.Tab value="history">Timeline</Tabs.Tab><Tabs.Tab value="tasks">Tasks & outcomes</Tabs.Tab><Tabs.Tab value="context">Context</Tabs.Tab><Tabs.Tab value="workflow">Workflow versions</Tabs.Tab><Tabs.Tab value="coverage">Coverage</Tabs.Tab></Tabs.List>
      <Tabs.Panel value="context" pt="xs"><Stack gap="xs"><TextInput size="xs" label="Native session ID" value={session} onChange={e=>{setSession(e.target.value);setContextAfter(0);setContextThrough(undefined)}}/><Text size="xs">Observed window limits and usage. Active context size and per-role token counts remain unavailable.</Text>{context.isError && <Alert color="red">{context.error.message}</Alert>}{context.data?.items.map(row=><div key={row.id}><Text size="xs" c="dimmed">Window limit: {row.context.window_limit.value?.toLocaleString()??'Unavailable'} · Turn usage: {row.context.turn_usage?.total_tokens?.toLocaleString()??'Unavailable'} · Thread cumulative usage: {row.context.thread_cumulative_usage?.total_tokens?.toLocaleString()??'Unavailable'}</Text><Observation row={row}/></div>)}<Group><Button size="compact-xs" disabled={!contextAfter} onClick={()=>{setContextAfter(0);setContextThrough(undefined)}}>Start</Button><Button size="compact-xs" disabled={!context.data?.next} onClick={()=>{setContextAfter(context.data!.next!);setContextThrough(context.data!.through)}}>Next context events</Button></Group></Stack></Tabs.Panel>
      <Tabs.Panel value="history" pt="xs"><Stack gap="xs">
        <Group gap="xs" align="end"><Select size="xs" label="Task" placeholder="All tasks" clearable searchable data={(tasks.data?.items??[]).map(t=>({value:t.id,label:t.title}))} value={task} onChange={v=>{setTask(v);reset()}}/><TextInput size="xs" label="Native session ID" value={session} onChange={e=>{setSession(e.target.value);reset()}}/><TextInput size="xs" label="Find event or tool" value={search} onChange={e=>{setSearch(e.target.value);reset()}}/><Button size="xs" variant="default" onClick={reset}>Latest</Button></Group>
        {history.isError && <Alert color="red">{history.error.message}</Alert>}
        {history.isPending && <Text size="sm">Loading timeline…</Text>}
        {history.data?.items.length===0 && <Alert>No observations in this selection. Enable content collection in Sources or connect hooks.</Alert>}
        <div>{history.data?.items.map(row=><Observation key={row.id} row={row}/>)}</div>
        <Group justify="space-between"><Text size="xs" c="dimmed">{history.data?.items.length ?? 0} events · frozen boundary {history.data?.through ?? '…'}</Text><Group gap="xs"><Button component="a" size="xs" variant="default" href={`/api/v1/evidence/export?${exportParams}`} download>Export selection</Button><Button size="xs" disabled={!history.data?.next} onClick={()=>{setThrough(history.data!.through);setAfter(history.data!.next!)}}>Next 50</Button></Group></Group>
      </Stack></Tabs.Panel>
      <Tabs.Panel value="tasks" pt="xs"><Stack gap="xs">
        <Group align="end"><TextInput size="xs" label="New task title" value={title} onChange={e=>setTitle(e.target.value)}/><Button size="xs" disabled={!title || save.isPending} onClick={()=>save.mutate({path:'tasks',body:{title}})}>Create task</Button></Group>
        <Select size="xs" label="Task" clearable searchable data={(tasks.data?.items??[]).map(t=>({value:t.id,label:`${t.title} · ${t.links} links`}))} value={task} onChange={v=>{setTask(v);reset()}}/>
        {task && <><Group align="end"><TextInput size="xs" label="Native session ID to link" value={link} onChange={e=>setLink(e.target.value)}/><Button size="xs" disabled={!link || save.isPending} onClick={()=>save.mutate({path:`tasks/${encodeURIComponent(task)}/links`,body:{session_id:link,confirmed:true}})}>Confirm link</Button></Group>
        {detail.data?.links.map(l=><Text key={l.session_id} size="xs">{l.session_id} · {l.status}</Text>)}
        <Group align="end"><Select size="xs" label="Outcome" value={mark} onChange={setMark} data={['accepted','rework','repeated_mistake','misunderstood','successful_approach']}/><TextInput size="xs" label="Note" value={note} onChange={e=>setNote(e.target.value)}/><Button size="xs" disabled={!mark || save.isPending} onClick={()=>save.mutate({path:`tasks/${encodeURIComponent(task)}/marks`,body:{kind:mark,note}})}>Save mark</Button></Group>
        {detail.data?.marks.map((m,i)=><Text size="xs" key={i}>{m.created_at.slice(0,19)} · <strong>{m.kind}</strong> · {m.note}</Text>)}
        {detail.data?.versions.map(v=><ArtifactVersion key={v.id} hash={v.object_hash} label={`Observed external version · ${v.observed_at}`}/>)}</>}
      </Stack></Tabs.Panel>
      <Tabs.Panel value="workflow" pt="xs"><Stack gap="xs"><Text size="xs">These files were discovered at the recorded time. Discovery does not prove the agent loaded or used them.</Text>
        <Group align="end"><TextInput size="xs" label="Register workflow root (absolute path)" value={root} onChange={e=>setRoot(e.target.value)}/><Button size="xs" disabled={!root || save.isPending} onClick={()=>save.mutate({path:'workflow',body:{path:root}})}>Capture versions</Button></Group>
        {workflow.isError && <Alert color="red">{workflow.error.message}</Alert>}
        <Group grow><Select size="xs" label="Before" clearable searchable value={before} onChange={setBefore} data={Array.from(new Map((workflow.data?.items??[]).filter(v=>v.object_hash).map(v=>[v.object_hash!,{value:v.object_hash!,label:`${v.path} · ${v.observed_at.slice(0,19)}`}])).values())}/><Select size="xs" label="After" clearable searchable value={afterVersion} onChange={setAfterVersion} data={Array.from(new Map((workflow.data?.items??[]).filter(v=>v.object_hash).map(v=>[v.object_hash!,{value:v.object_hash!,label:`${v.path} · ${v.observed_at.slice(0,19)}`}])).values())}/></Group>
        {before && afterVersion && <VersionDiff before={before} after={afterVersion}/>}
        {workflow.data?.items.map(v=>v.object_hash ? <ArtifactVersion key={v.id} hash={v.object_hash} label={`${v.observed_at.slice(0,19)} · ${v.path} · ${v.state}`}/> : <Text size="xs" key={v.id}>{v.path} · {v.disposition}</Text>)}
        <Group><Button size="compact-xs" onClick={()=>setVersionAfter(0)}>Start</Button><Button size="compact-xs" disabled={workflow.data?.items.length!==100} onClick={()=>setVersionAfter(workflow.data!.items[workflow.data!.items.length-1].cursor)}>Next versions</Button></Group>
      </Stack></Tabs.Panel>
      <Tabs.Panel value="coverage" pt="xs"><Stack gap="xs"><Alert color="orange">Real Codex GUI acceptance on macOS is pending. Hidden system instructions, upstream truncation, and history lost before collection cannot be reconstructed.</Alert><Text size="sm">Full model requests and exact context size are not guaranteed by the sources. Cumulative token usage is not current context. Secret detection is conservative, with explicit exclusions.</Text>
        {coverage.isError && <Alert color="red">{coverage.error.message}</Alert>}
        {coverage.data?.channels.map((c,i)=><Group key={i} gap="xs"><Badge>{c.channel}</Badge><Text size="xs">{c.count} · {c.disposition} {c.reason && `· ${c.reason}`} · last received {c.last_received.slice(0,19)}</Text></Group>)}
        <Text size="xs">No automatic retention deletion. Detailed tool payloads are loaded on demand. Export a portable package with the evidence export command.</Text>
      </Stack></Tabs.Panel>
    </Tabs>
  </Stack>;
}
