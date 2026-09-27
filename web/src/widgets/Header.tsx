import React from 'react';
import { Group, Title, Button, Badge, Anchor, Box } from '@mantine/core';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { NavLink } from 'react-router-dom';
import { fetchHealth, seedDemoData } from '../shared/api/client';
import { IconDatabase, IconActivity, IconPlayerPlay } from '@tabler/icons-react';

export const Header: React.FC = () => {
  const queryClient = useQueryClient();
  const { data: health } = useQuery({
    queryKey: ['health'],
    queryFn: fetchHealth,
    refetchInterval: 5000,
  });

  const seedMutation = useMutation({
    mutationFn: seedDemoData,
    onSuccess: () => {
      queryClient.invalidateQueries();
    },
  });

  return (
    <Box
      style={{
        borderBottom: '1px solid var(--mantine-color-default-border)',
        padding: '12px 24px',
        backgroundColor: 'var(--mantine-color-body)',
        position: 'sticky',
        top: 0,
        zIndex: 100,
      }}
    >
      <Group justify="space-between" align="center">
        <Group gap="lg">
          <Group gap="xs" style={{ cursor: 'pointer' }}>
            <span style={{ fontSize: '26px' }}>🔭</span>
            <div>
              <Title order={3} style={{ lineHeight: 1.1 }}>
                Harnesscope
              </Title>
              <Group gap={6} mt={2}>
                <Badge size="xs" variant="outline" color="blue">
                  v0.2.0
                </Badge>
                <Badge size="xs" variant="dot" color={health?.db_connected ? 'green' : 'red'}>
                  {health?.db_connected ? 'SQLite WAL' : 'Offline'}
                </Badge>
              </Group>
            </div>
          </Group>

          <Group gap="xs" ml="md">
            <Button
              component={NavLink}
              to="/executions"
              variant="subtle"
              size="sm"
              styles={{
                root: {
                  fontWeight: 600,
                },
              }}
            >
              Executions
            </Button>
            <Button
              component={NavLink}
              to="/sessions"
              variant="subtle"
              size="sm"
              styles={{
                root: {
                  fontWeight: 600,
                },
              }}
            >
              Sessions
            </Button>
            <Button
              component={NavLink}
              to="/stats"
              variant="subtle"
              size="sm"
              styles={{
                root: {
                  fontWeight: 600,
                },
              }}
            >
              Stats
            </Button>
          </Group>
        </Group>

        <Group gap="sm">
          {health && (
            <Group gap="xs" visibleFrom="sm">
              <Badge variant="light" color="indigo" size="sm" leftSection={<IconActivity size={12} />}>
                Active: {health.active_executions} execs, {health.active_runtimes} runs
              </Badge>
              <Badge variant="light" color="gray" size="sm" leftSection={<IconDatabase size={12} />}>
                Total: {health.total_executions} execs
              </Badge>
            </Group>
          )}

          <Button
            size="xs"
            variant="outline"
            color="teal"
            leftSection={<IconPlayerPlay size={14} />}
            loading={seedMutation.isPending}
            onClick={() => seedMutation.mutate()}
            title="Seed deterministic demo data into SQLite"
          >
            Seed Demo Data
          </Button>
        </Group>
      </Group>
    </Box>
  );
};
