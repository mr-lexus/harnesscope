import React from 'react';
import { Routes, Route, Navigate } from 'react-router-dom';
import { Box } from '@mantine/core';
import { Header } from '../widgets/Header';
import { ExecutionsPage } from '../pages/executions/ExecutionsPage';
import { ExecutionDetailPage } from '../pages/execution-detail/ExecutionDetailPage';
import { SessionsPage } from '../pages/sessions/SessionsPage';
import { SessionDetailPage } from '../pages/session-detail/SessionDetailPage';
import { StatsPage } from '../pages/stats/StatsPage';

export const App: React.FC = () => {
  return (
    <Box style={{ minHeight: '100vh', backgroundColor: 'var(--mantine-color-body)' }}>
      <Header />
      <Routes>
        <Route path="/" element={<Navigate to="/executions" replace />} />
        <Route path="/executions" element={<ExecutionsPage />} />
        <Route path="/executions/:id" element={<ExecutionDetailPage />} />
        <Route path="/sessions" element={<SessionsPage />} />
        <Route path="/sessions/:id" element={<SessionDetailPage />} />
        <Route path="/stats" element={<StatsPage />} />
        <Route path="*" element={<Navigate to="/executions" replace />} />
      </Routes>
    </Box>
  );
};
