export interface KeyCount {
  key: string;
  count: number;
}

export interface McpStat {
  name: string;
  configured: number;
  invoked: number;
}

export interface StatsSummary {
  total_executions: number;
  total_sessions: number;
  total_runtimes: number;
  by_runner: KeyCount[];
  by_model: KeyCount[];
  by_reasoning: KeyCount[];
  by_agent: KeyCount[];
  by_status: KeyCount[];
  mcp_stats: McpStat[];
  skill_stats: KeyCount[];
  plugin_stats: KeyCount[];
  avg_duration_ms: number;
}

export interface HealthMetrics {
  status: string;
  version: string;
  db_connected: boolean;
  active_executions: number;
  active_runtimes: number;
  total_executions: number;
  total_sessions: number;
}
