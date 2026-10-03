import { useState } from 'react';
import { Button, Card, Collapse, Group, Select, TextInput } from '@mantine/core';
import { ExecutionFilterParams } from '../shared/api/client';
interface Props { filters: ExecutionFilterParams; onChange: (updated: ExecutionFilterParams) => void; onReset: () => void }
export function ExecutionFilters({filters,onChange,onReset}: Props) {
  const advanced = ['runner','agent','branch','session','component'] as const;
  const count = advanced.filter(k=>filters[k] && filters[k]!=='ALL').length;
  const [open,setOpen] = useState(count>0);
  const change = (key: keyof ExecutionFilterParams, value: string | null) => onChange({...filters,[key]:value || undefined,page:1});
  return <Card withBorder p="sm" mb="sm">
    <div className="filter-grid">
      <Select size="xs" label="Period" allowDeselect={false} value={filters.period || 'all'} data={[{value:'all',label:'All time'},{value:'1h',label:'Last hour'},{value:'24h',label:'Last 24 hours'},{value:'7d',label:'Last 7 days'},{value:'30d',label:'Last 30 days'}]} onChange={v=>change('period',v)}/>
      <Select size="xs" label="Scope" allowDeselect={false} value={filters.scope || 'ALL'} data={[{value:'ALL',label:'All scopes'},'PROCESS','TURN','UNKNOWN','DEMO']} onChange={v=>change('scope',v)}/>
      <Select size="xs" label="Status" allowDeselect={false} value={filters.status || 'ALL'} data={[{value:'ALL',label:'All statuses'},'RUNNING','COMPLETED','FAILED','CANCELLED','UNKNOWN']} onChange={v=>change('status',v)}/>
      <TextInput size="xs" label="Model" placeholder="Any model" value={filters.model || ''} onChange={e=>change('model',e.currentTarget.value)}/>
    </div>
    <Group justify="space-between" mt={6}><Button size="compact-xs" variant="subtle" onClick={()=>setOpen(!open)} aria-expanded={open} aria-controls="advanced-filters">{open ? 'Hide':'More'} filters{count ? ` (${count} active)` : ''}</Button><Button size="compact-xs" variant="subtle" color="gray" onClick={onReset}>Reset</Button></Group>
    <Collapse in={open} id="advanced-filters"><div className="filter-grid" style={{marginTop:8}}>{advanced.map(k=><TextInput key={k} size="xs" label={{runner:'Runner (exact name)',agent:'Agent / role',branch:'Branch',session:'Session ID',component:'Component'}[k]} value={filters[k] === 'ALL' ? '' : filters[k] || ''} onChange={e=>change(k,e.currentTarget.value)}/>)}</div></Collapse>
  </Card>;
}
