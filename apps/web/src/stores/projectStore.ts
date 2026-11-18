import type {
  AnalysisReport,
  APIModel,
  ApplicationAdapter,
  Dependency,
  ProjectManifest,
  TrafficEntrySummary,
} from '@harflow/shared-types';

export type ActiveTab = 'dashboard' | 'traffic' | 'dependencies' | 'settings';
export type ThemeMode = 'dark' | 'light';

export interface ProjectViewState {
  manifest: ProjectManifest | null;
  model: APIModel | null;
  entries: TrafficEntrySummary[];
  report: AnalysisReport | null;
  selectedEntryId: string | null;
  error: string | null;
  loading: boolean;
}

export async function loadInitialProject(
  adapter: ApplicationAdapter,
): Promise<Omit<ProjectViewState, 'error' | 'loading'>> {
  const results = await adapter.invoke('analysis.results', {
    projectId: 'proj-demo-0001',
  });
  return {
    manifest: results.manifest,
    model: results.model,
    entries: results.entries,
    report: results.report,
    selectedEntryId: results.entries[0]?.id ?? null,
  };
}

export function applyUpdatedDependencies(
  prev: ProjectViewState,
  dependencies: Dependency[],
): ProjectViewState {
  if (!prev.model || !prev.manifest) {
    return prev;
  }
  const unknownMappings = dependencies.filter((d) => d.status === 'unknown').length;
  return {
    ...prev,
    model: { ...prev.model, dependencies },
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
