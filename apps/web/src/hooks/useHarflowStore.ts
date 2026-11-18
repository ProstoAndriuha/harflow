import { useCallback, useEffect, useState } from 'react';
import type { ApplicationAdapter } from '@harflow/shared-types';
import {
  applyUpdatedDependencies,
  loadInitialProject,
  type ProjectViewState,
} from '../stores/projectStore.js';

export function useHarflowStore(adapter: ApplicationAdapter) {
  const [state, setState] = useState<ProjectViewState>({
    manifest: null,
    model: null,
    entries: [],
    report: null,
    selectedEntryId: null,
    error: null,
    loading: true,
  });

  useEffect(() => {
    let active = true;
    void loadInitialProject(adapter)
      .then((initial) => {
        if (active) {
          setState({ ...initial, error: null, loading: false });
        }
      })
      .catch((err: unknown) => {
        if (active) {
          setState((prev) => ({
            ...prev,
            loading: false,
            error: err instanceof Error ? err.message : 'Failed to load project',
          }));
        }
      });
    return () => {
      active = false;
    };
  }, [adapter]);

  const selectEntry = useCallback((entryId: string) => {
    setState((prev) => ({ ...prev, selectedEntryId: entryId }));
  }, []);

  const loadDemoProject = useCallback(async () => {
    await adapter.invoke('project.create', { name: 'Orders API Demo' });
    const results = await adapter.invoke('analysis.results', {
      projectId: 'proj-demo-0001',
    });
    setState({
      manifest: results.manifest,
      model: results.model,
      entries: results.entries,
      report: results.report,
      selectedEntryId: results.entries[0]?.id ?? null,
      error: null,
      loading: false,
    });
  }, [adapter]);

  const importHarText = useCallback(
    async (sourceName: string, rawText: string) => {
      try {
        const res = await adapter.invoke('har.import', {
          projectId: state.manifest?.id ?? 'proj-demo-0001',
          sourceName,
          rawText,
        });
        const analysis = await adapter.invoke('analysis.results', {
          projectId: res.manifest.id,
        });
        setState((prev) => ({
          ...prev,
          manifest: analysis.manifest,
          model: analysis.model,
          entries: analysis.entries,
          report: analysis.report,
          selectedEntryId: analysis.entries[0]?.id ?? null,
          error: null,
        }));
      } catch (err: unknown) {
        setState((prev) => ({
          ...prev,
          error: err instanceof Error ? err.message : 'Import failed',
        }));
      }
    },
    [adapter, state.manifest?.id],
  );

  const confirmDependency = useCallback(
    async (dependencyId: string) => {
      const updated = await adapter.invoke('dependencies.confirm', {
        projectId: state.manifest?.id ?? 'proj-demo-0001',
        dependencyId,
      });
      setState((prev) => applyUpdatedDependencies(prev, updated));
    },
    [adapter, state.manifest?.id],
  );

  const rejectDependency = useCallback(
    async (dependencyId: string) => {
      const updated = await adapter.invoke('dependencies.reject', {
        projectId: state.manifest?.id ?? 'proj-demo-0001',
        dependencyId,
      });
      setState((prev) => applyUpdatedDependencies(prev, updated));
    },
    [adapter, state.manifest?.id],
  );

  return {
    state,
    selectEntry,
    loadDemoProject,
    importHarText,
    confirmDependency,
    rejectDependency,
  };
}
