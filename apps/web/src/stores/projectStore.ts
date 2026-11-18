import type {
  AnalysisReport,
  APIModel,
  ApplicationAdapter,
  CleanerConfig,
  CleanerPreview,
  ConfirmationStatus,
  Dependency,
  GraphKind,
  GraphViewModel,
  ProjectManifest,
  SessionComparisonReport,
  SessionLifecycleReport,
  TrafficEntrySummary,
  Transformation,
} from '@harflow/shared-types';
import {
  createHistoryStack,
  type HistoryStack,
} from './historyStack.js';

export type ActiveTab =
  | 'dashboard'
  | 'traffic'
  | 'endpoints'
  | 'dependencies'
  | 'sessions'
  | 'settings';

export type ThemeMode = 'dark' | 'light';

export type UndoableMutation =
  | {
      kind: 'dependency';
      dependencyId: string;
      before: {
        status: ConfirmationStatus;
        sourcePath: string;
        targetPath: string;
        transformation: Transformation | null;
      };
      after: {
        status: ConfirmationStatus;
        sourcePath: string;
        targetPath: string;
        transformation: Transformation | null;
      };
    }
  | {
      kind: 'cleaner';
      before: CleanerConfig;
      after: CleanerConfig;
    }
  | {
      kind: 'endpoint_override';
      before: Record<string, string>;
      after: Record<string, string>;
    };

export interface ProjectViewState {
  manifest: ProjectManifest | null;
  model: APIModel | null;
  entries: TrafficEntrySummary[];
  report: AnalysisReport | null;
  cleanerPreview: CleanerPreview | null;
  sessionLifecycle: SessionLifecycleReport | null;
  sessionComparison: SessionComparisonReport | null;
  graphKind: GraphKind;
  graph: GraphViewModel | null;
  selectedEntryId: string | null;
  selectedEndpointId: string | null;
  selectedDependencyId: string | null;
  history: HistoryStack<UndoableMutation>;
  error: string | null;
  loading: boolean;
}

export async function loadInitialProject(
  adapter: ApplicationAdapter,
  graphKind: GraphKind = 'endpoint',
): Promise<Omit<ProjectViewState, 'error' | 'loading'>> {
  const results = await adapter.invoke('analysis.results', {
    projectId: 'proj-demo-0001',
  });
  const graph = await adapter.invoke('graph.get', {
    projectId: results.manifest.id,
    kind: graphKind,
  });
  return {
    manifest: results.manifest,
    model: results.model,
    entries: results.entries,
    report: results.report,
    cleanerPreview: results.cleanerPreview ?? null,
    sessionLifecycle: results.sessionLifecycle ?? null,
    sessionComparison: results.sessionComparison ?? null,
    graphKind,
    graph,
    selectedEntryId: results.entries[0]?.id ?? null,
    selectedEndpointId: results.model.endpoints[0]?.id ?? null,
    selectedDependencyId: results.model.dependencies[0]?.id ?? null,
    history: createHistoryStack<UndoableMutation>(),
  };
}

export function applyUpdatedDependencies(
  prev: ProjectViewState,
  dependencies: Dependency[],
  updatedGraph?: GraphViewModel | null,
): ProjectViewState {
  if (!prev.model || !prev.manifest) {
    return prev;
  }
  const unknownMappings = dependencies.filter((d) => d.status === 'unknown').length;
  return {
    ...prev,
    model: { ...prev.model, dependencies },
    graph: updatedGraph !== undefined ? updatedGraph : prev.graph,
    manifest: {
      ...prev.manifest,
      statistics: {
        ...prev.manifest.statistics,
        dependencies: dependencies.length,
        unknownMappings,
      },
    },
    report: prev.report
      ? {
          ...prev.report,
          totals: {
            ...prev.report.totals,
            dependencies: dependencies.length,
            unknownMappings,
          },
        }
      : null,
  };
}
