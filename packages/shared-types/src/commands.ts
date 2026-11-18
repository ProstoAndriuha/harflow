import type {
  APIModel,
  Dependency,
  Endpoint,
  SessionSource,
} from './api-model.js';
import type { AnalysisReport, GraphViewModel } from './graph.js';
import type { ProjectManifest, TrafficEntrySummary } from './project.js';

export interface CommandPayloads {
  'project.create': { name: string };
  'project.open': { projectId: string };
  'project.save': { projectId: string };
  'har.import': { projectId: string; sourceName: string; rawText: string };
  'analysis.start': { projectId: string };
  'analysis.cancel': { projectId: string };
  'analysis.results': { projectId: string };
  'endpoints.list': { projectId: string };
  'dependencies.list': { projectId: string };
  'dependencies.confirm': { projectId: string; dependencyId: string };
  'dependencies.reject': { projectId: string; dependencyId: string };
  'graph.get': { projectId: string };
  'sdk.generate': { projectId: string; target: 'python_httpx' | 'typescript_fetch' };
  'sdk.preview': { projectId: string; target: 'python_httpx' | 'typescript_fetch' };
  'validation.mock': { projectId: string };
  'export.create': { projectId: string; format: 'openapi_json' | 'markdown_report' };
}

export interface CommandResults {
  'project.create': ProjectManifest;
  'project.open': APIModel;
  'project.save': ProjectManifest;
  'har.import': {
    source: SessionSource;
    entries: TrafficEntrySummary[];
    model: APIModel;
    manifest: ProjectManifest;
  };
  'analysis.start': {
    model: APIModel;
    manifest: ProjectManifest;
    report: AnalysisReport;
  };
  'analysis.cancel': { cancelled: boolean };
  'analysis.results': {
    model: APIModel;
    manifest: ProjectManifest;
    entries: TrafficEntrySummary[];
    report: AnalysisReport;
  };
  'endpoints.list': Endpoint[];
  'dependencies.list': Dependency[];
  'dependencies.confirm': Dependency[];
  'dependencies.reject': Dependency[];
  'graph.get': GraphViewModel;
  'sdk.generate': { files: Array<{ path: string; content: string }> };
  'sdk.preview': { files: Array<{ path: string; content: string }> };
  'validation.mock': { totalChecks: number; passed: number; failed: number; skipped: number };
  'export.create': { filename: string; content: string };
}

export type AppCommandName = keyof CommandPayloads;

export interface ApplicationAdapter {
  readonly mode: 'in_memory_web' | 'tauri_ipc';
  invoke<K extends AppCommandName>(
    command: K,
    payload: CommandPayloads[K],
  ): Promise<CommandResults[K]>;
}
