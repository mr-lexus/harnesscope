import { ActionIcon, Badge, Button, Group, Menu, Text, Tooltip } from '@mantine/core';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { NavLink } from 'react-router-dom';
import { IconAdjustments, IconLayoutRows, IconPlayerPause, IconPlayerPlay, IconRefresh, IconTelescope } from '@tabler/icons-react';
import { fetchCollection, collectionLabel } from '../shared/api/sources';
import { fetchHealth, seedDemoData } from '../shared/api/client';
import { usePreferences, useRefreshInterval, toggleDensity, toggleLive } from '../shared/preferences';
export function Header() {
  const client = useQueryClient();
  const { compact, live } = usePreferences();
  const health = useQuery({queryKey:['health'],queryFn:fetchHealth,refetchInterval:useRefreshInterval()});
  const collection = useQuery({queryKey:['collection'],queryFn:fetchCollection,refetchInterval:useRefreshInterval()});
  const state = collection.data?.status;
  const stateLabel = state ? collectionLabel[state] : 'Checking collection';
  const needsAttention = collection.isError || state === 'NEEDS_ATTENTION' || state === 'OVERDUE';
  const seed = useMutation({mutationFn:seedDemoData,onSuccess:()=>client.invalidateQueries()});
  return <header className="app-header">
    <a className="skip-link" href="#main">Skip to content</a>
    <NavLink to="/" className="brand"><IconTelescope size={22}/><span>Harnesscope</span></NavLink>
    <nav className="primary-nav" aria-label="Main navigation">
      {[['/','Overview'],['/executions','Executions'],['/sessions','Sessions'],['/monitoring','Monitor'],['/sources','Sources'],['/stats','Stats']].map(([to,label]) =>
        <NavLink key={to} to={to} end={to==='/'}>{label}</NavLink>)}
    </nav>
    <Group className="header-tools" gap={6} wrap="nowrap">
      <Tooltip label={health.isError ? 'Server offline. Start Harnesscope to resume collection.' : stateLabel}><NavLink to="/sources" aria-label={stateLabel} style={{textDecoration:'none'}}><Badge variant="dot" size="sm" color={health.isError || needsAttention ? 'red' : state === 'COLLECTING' ? 'teal' : 'orange'}>{health.isError ? 'Offline' : needsAttention ? 'Check' : state === 'COLLECTING' ? 'Active' : state === 'CATCHING_UP' ? 'Backlog' : state === 'PAUSED' ? 'Paused' : state === 'STARTING' ? 'Starting' : 'Setup'}</Badge></NavLink></Tooltip>
      <Tooltip label={live ? 'Pause automatic refresh' : 'Resume automatic refresh'}><ActionIcon variant="subtle" aria-label={live ? 'Pause automatic refresh' : 'Resume automatic refresh'} onClick={toggleLive} color={live ? 'gray':'orange'}>{live ? <IconPlayerPause size={17}/> : <IconPlayerPlay size={17}/>}</ActionIcon></Tooltip>
      <Menu position="bottom-end" withinPortal><Menu.Target><ActionIcon variant="default" aria-label="Panel settings"><IconAdjustments size={17}/></ActionIcon></Menu.Target><Menu.Dropdown>
        <Menu.Label>Panel · {health.data?.version ?? 'local'}</Menu.Label>
        <Menu.Item leftSection={<IconLayoutRows size={16}/>} onClick={toggleDensity}>{compact ? 'Use comfortable density' : 'Use compact density'}</Menu.Item>
        <Menu.Item leftSection={<IconRefresh size={16}/>} onClick={()=>client.invalidateQueries()}>Refresh all data</Menu.Item>
        <Menu.Divider/><Menu.Item disabled={seed.isPending} onClick={()=>seed.mutate()}>Load demo data</Menu.Item>
      </Menu.Dropdown></Menu>
    </Group>
    {!live && <Text className="header-notice" size="xs" c="orange" role="status">Auto-refresh paused. Collection continues. <Button variant="subtle" size="compact-xs" onClick={toggleLive}>Resume</Button></Text>}
    {seed.isError && <Text className="header-notice" size="xs" c="red" role="alert">{seed.error.message}</Text>}
    {seed.isSuccess && <Text className="header-notice" size="xs" c="teal" role="status">Demo data loaded. Enable “Include demo” in Overview to see it.</Text>}
  </header>;
}
