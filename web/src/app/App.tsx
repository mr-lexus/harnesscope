import React, { lazy, Suspense } from 'react';
import { Routes, Route, Navigate } from 'react-router-dom';
import { Box, Center, Loader } from '@mantine/core';
const SourcesPage = lazy(() => import('../pages/sources/SourcesPage').then(m => ({default:m.SourcesPage})));
const EvidencePage = lazy(() => import('../pages/evidence/EvidencePage').then(m => ({default:m.EvidencePage})));
const MonitoringPage = lazy(() => import('../pages/monitoring/MonitoringPage').then(m => ({default:m.MonitoringPage})));
import { Header } from '../widgets/Header';
const ExecutionsPage = lazy(() => import('../pages/executions/ExecutionsPage').then(m => ({default:m.ExecutionsPage})));
const ExecutionDetailPage = lazy(() => import('../pages/execution-detail/ExecutionDetailPage').then(m => ({default:m.ExecutionDetailPage})));
const SessionsPage = lazy(() => import('../pages/sessions/SessionsPage').then(m => ({default:m.SessionsPage})));
const SessionDetailPage = lazy(() => import('../pages/session-detail/SessionDetailPage').then(m => ({default:m.SessionDetailPage})));
const OverviewPage = lazy(() => import('../pages/overview/OverviewPage').then(m => ({default:m.OverviewPage})));
const StatsPage = lazy(() => import('../pages/stats/StatsPage').then(m => ({default:m.StatsPage})));

export const App: React.FC = () => {
  return (
    <Box style={{ minHeight: '100vh', backgroundColor: 'var(--mantine-color-body)' }}>
      <Header />
      <main id="main" tabIndex={-1}><Suspense fallback={<Center p="xl"><Loader aria-label="Loading page"/></Center>}><Routes>
        <Route path="/" element={<OverviewPage />} />
        <Route path="/executions" element={<ExecutionsPage />} />
        <Route path="/executions/:id" element={<ExecutionDetailPage />} />
        <Route path="/sessions" element={<SessionsPage />} />
        <Route path="/sessions/:id" element={<SessionDetailPage />} />
        <Route path="/sources" element={<SourcesPage />} />
        <Route path="/evidence" element={<EvidencePage />} />
        <Route path="/monitoring" element={<MonitoringPage />} />
        <Route path="/stats" element={<StatsPage />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes></Suspense></main>
    </Box>
  );
};
