import React from 'react';
import { Badge, Text } from '@mantine/core';

export const StatusBadge: React.FC<{ status: string }> = ({ status }) => {
  const s = status ? status.toUpperCase() : 'UNKNOWN';
  let color = 'gray';
  if (s === 'COMPLETED' || s === 'ACTIVE') color = 'green';
  else if (s === 'RUNNING') color = 'blue';
  else if (s === 'FAILED') color = 'red';
  else if (s === 'CANCELLED' || s === 'ABORTED') color = 'orange';

  return (
    <Badge color={color} variant="light" size="sm">
      {s}
    </Badge>
  );
};

export const AttributionBadge: React.FC<{ attribution: string }> = ({ attribution }) => {
  const a = attribution ? attribution.toUpperCase() : 'UNKNOWN';
  let color = 'gray';
  if (a === 'OBSERVED') color = 'teal';
  else if (a === 'CORRELATED') color = 'indigo';
  else if (a === 'AMBIGUOUS') color = 'yellow';

  return (
    <Badge color={color} variant="outline" size="sm">
      {a}
    </Badge>
  );
};

export const ComponentStateBadge: React.FC<{ state: string; count?: number }> = ({ state, count }) => {
  const s = state ? state.toUpperCase() : 'UNKNOWN';
  let color = 'gray';
  if (s === 'INVOKED') color = 'green';
  else if (s === 'LOADED') color = 'grape';
  else if (s === 'DISCOVERED') color = 'cyan';
  else if (s === 'CONFIGURED') color = 'blue';

  return (
    <Badge color={color} variant={s === 'INVOKED' ? 'filled' : 'light'} size="sm">
      {s}{count && count > 0 ? ` (${count})` : ''}
    </Badge>
  );
};

export const UnknownText: React.FC<{ value: string | null | undefined; fallback?: string }> = ({ value, fallback = 'UNKNOWN' }) => {
  if (!value || value === 'UNKNOWN') {
    return (
      <Badge color="gray" variant="outline" size="xs">
        {fallback}
      </Badge>
    );
  }
  return <Text span inherit>{value}</Text>;
};
