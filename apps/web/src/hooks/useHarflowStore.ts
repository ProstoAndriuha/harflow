import { useCallback, useEffect, useRef, useState } from 'react';
import type {
  ApplicationAdapter,
  CleanerUpdatePatch,
  CommandResults,
  DependencyMappingOverride,
  GraphKind,
} from '@harflow/shared-types';
import {
  cloneCleanerConfig,
  DEFAULT_CLEANER_CONFIG,
} from '../features/intelligenceEngine.js';
import {
  canRedo,
  canUndo,
  createHistoryStack,
  pushHistory,
  stepRedo,
  stepUndo,
} from '../stores/historyStack.js';
import {
  applyUpdatedDependencies,
  loadInitialProject,
  type ProjectViewState,
  type UndoableMutation,
} from '../stores/projectStore.js';

export function useHarflowStore(adapter: ApplicationAdapter) {
  const [state, setState] = useState<ProjectViewState>({
    manifest: null,
    model: null,
    entries: [],
    report: null,
    cleanerPreview: null,
    sessionLifecycle: null,
    sessionComparison: null,
    graphKind: 'endpoint',
    graph: null,
    selectedEntryId: null,
    selectedEndpointId: null,
    selectedDependencyId: null,
    history: createHistoryStack<UndoableMutation>(),
    error: null,
    loading: true,
  });

  const stateRef = useRef(state);
  stateRef.current = state;

  useEffect(() => {
    let active = true;
    void loadInitialProject(adapter, 'endpoint')
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

  const selectEndpoint = useCallback((endpointId: string) => {
    setState((prev) => ({ ...prev, selectedEndpointId: endpointId }));
  }, []);

  const selectDependency = useCallback((dependencyId: string | null) => {
    setState((prev) => ({ ...prev, selectedDependencyId: dependencyId }));
  }, []);

  const setGraphKind = useCallback(
    async (kind: GraphKind) => {
      const projectId = stateRef.current.manifest?.id ?? 'proj-demo-0001';
      const graph = await adapter.invoke('graph.get', { projectId, kind });
      setState((prev) => ({ ...prev, graphKind: kind, graph }));
    },
    [adapter],
  );

  const applyAnalysisResponse = useCallback(
    async (
      analysis: CommandResults['analysis.results'],
      nextHistory = stateRef.current.history,
    ) => {
      const kind = stateRef.current.graphKind;
      const graph = await adapter.invoke('graph.get', {
        projectId: analysis.manifest.id,
        kind,
      });
      setState((prev) => ({
        ...prev,
        manifest: analysis.manifest,
        model: analysis.model,
        entries: analysis.entries,
        report: analysis.report,
        cleanerPreview: analysis.cleanerPreview ?? null,
        sessionLifecycle: analysis.sessionLifecycle ?? null,
        sessionComparison: analysis.sessionComparison ?? null,
        graph,
        selectedEntryId:
          analysis.entries.some((e) => e.id === prev.selectedEntryId)
            ? prev.selectedEntryId
            : (analysis.entries[0]?.id ?? null),
        selectedEndpointId:
          analysis.model.endpoints.some((e) => e.id === prev.selectedEndpointId)
            ? prev.selectedEndpointId
            : (analysis.model.endpoints[0]?.id ?? null),
        selectedDependencyId:
          analysis.model.dependencies.some((d) => d.id === prev.selectedDependencyId)
            ? prev.selectedDependencyId
            : (analysis.model.dependencies[0]?.id ?? null),
        history: nextHistory,
        error: null,
        loading: false,
      }));
    },
    [adapter],
  );

  const loadDemoProject = useCallback(async () => {
    await adapter.invoke('project.create', { name: 'Orders API Demo' });
    const results = await adapter.invoke('analysis.results', {
      projectId: 'proj-demo-0001',
    });
    await applyAnalysisResponse(results, createHistoryStack<UndoableMutation>());
  }, [adapter, applyAnalysisResponse]);

  const importHarFiles = useCallback(
    async (files: Array<{ sourceName: string; rawText: string }>) => {
      try {
        let projectId = stateRef.current.manifest?.id ?? 'proj-demo-0001';
        for (const file of files) {
          const res = await adapter.invoke('har.import', {
            projectId,
            sourceName: file.sourceName,
            rawText: file.rawText,
          });
          projectId = res.manifest.id;
        }
        const analysis = await adapter.invoke('analysis.results', { projectId });
        await applyAnalysisResponse(analysis);
      } catch (err: unknown) {
        setState((prev) => ({
          ...prev,
          error: err instanceof Error ? err.message : 'Import failed',
        }));
      }
    },
    [adapter, applyAnalysisResponse],
  );

  const importHarText = useCallback(
    async (sourceName: string, rawText: string) => {
      await importHarFiles([{ sourceName, rawText }]);
    },
    [importHarFiles],
  );

  const mutateDependencyWithHistory = useCallback(
    async (
      dependencyId: string,
      nextStatus: 'user_confirmed' | 'rejected' | 'confirmed' | 'likely' | 'unknown',
      edit?: DependencyMappingOverride,
      recordHistory = true,
    ) => {
      const curr = stateRef.current;
      const projectId = curr.manifest?.id ?? 'proj-demo-0001';
      const existing = curr.model?.dependencies.find((d) => d.id === dependencyId);
      const before = existing
        ? {
            status: existing.status,
            sourcePath: existing.source.jsonPathOrKey,
            targetPath: existing.target.jsonPathOrKey,
            transformation: existing.transformation ?? null,
          }
        : null;

      const updated =
        nextStatus === 'rejected' && !edit
          ? await adapter.invoke('dependencies.reject', {
              projectId,
              dependencyId,
            })
          : await adapter.invoke('dependencies.confirm', {
              projectId,
              dependencyId,
              status: nextStatus,
              edit,
            });
      const graph = await adapter.invoke('graph.get', {
        projectId,
        kind: curr.graphKind,
      });

      const afterItem = updated.find((d) => d.id === dependencyId);
      setState((prev) => {
        const applied = applyUpdatedDependencies(prev, updated, graph);
        if (!recordHistory || !before || !afterItem) {
          return applied;
        }
        return {
          ...applied,
          history: pushHistory(prev.history, {
            kind: 'dependency',
            dependencyId,
            before,
            after: {
              status: afterItem.status,
              sourcePath: afterItem.source.jsonPathOrKey,
              targetPath: afterItem.target.jsonPathOrKey,
              transformation: afterItem.transformation ?? null,
            },
          }),
        };
      });
    },
    [adapter],
  );

  const confirmDependency = useCallback(
    async (dependencyId: string) => {
      await mutateDependencyWithHistory(dependencyId, 'user_confirmed');
    },
    [mutateDependencyWithHistory],
  );

  const rejectDependency = useCallback(
    async (dependencyId: string) => {
      await mutateDependencyWithHistory(dependencyId, 'rejected');
    },
    [mutateDependencyWithHistory],
  );

  const editDependencyMapping = useCallback(
    async (dependencyId: string, edit: DependencyMappingOverride) => {
      await mutateDependencyWithHistory(
        dependencyId,
        edit.status ?? 'user_confirmed',
        edit,
      );
    },
    [mutateDependencyWithHistory],
  );

  const updateCleaner = useCallback(
    async (patch: CleanerUpdatePatch, recordHistory = true) => {
      const curr = stateRef.current;
      const projectId = curr.manifest?.id ?? 'proj-demo-0001';
      const before = cloneCleanerConfig(
        curr.manifest?.settings.cleaner ?? DEFAULT_CLEANER_CONFIG,
      );
      const analysis = await adapter.invoke('analysis.start', {
        projectId,
        cleanerPatch: patch,
      });
      const after = cloneCleanerConfig(
        analysis.manifest.settings.cleaner ?? DEFAULT_CLEANER_CONFIG,
      );
      const nextHistory = recordHistory
        ? pushHistory(curr.history, { kind: 'cleaner', before, after })
        : curr.history;
      await applyAnalysisResponse(analysis, nextHistory);
    },
    [adapter, applyAnalysisResponse],
  );

  const setEndpointOverride = useCallback(
    async (endpointKey: string, template: string, recordHistory = true) => {
      const curr = stateRef.current;
      const projectId = curr.manifest?.id ?? 'proj-demo-0001';
      const before = { ...(curr.manifest?.settings.endpointOverrides ?? {}) };
      const after = { ...before };
      if (!template.trim()) {
        delete after[endpointKey];
      } else {
        after[endpointKey] = template.trim();
      }
      const analysis = await adapter.invoke('analysis.start', {
        projectId,
        endpointOverrides: after,
      });
      const nextHistory = recordHistory
        ? pushHistory(curr.history, { kind: 'endpoint_override', before, after })
        : curr.history;
      await applyAnalysisResponse(analysis, nextHistory);
    },
    [adapter, applyAnalysisResponse],
  );

  const applyMutationTarget = useCallback(
    async (
      mutation: UndoableMutation,
      direction: 'undo' | 'redo',
      nextHistory: ProjectViewState['history'],
    ) => {
      const projectId = stateRef.current.manifest?.id ?? 'proj-demo-0001';
      if (mutation.kind === 'dependency') {
        const snap = direction === 'undo' ? mutation.before : mutation.after;
        const updated = await adapter.invoke('dependencies.confirm', {
          projectId,
          dependencyId: mutation.dependencyId,
          status: snap.status,
          edit: {
            status: snap.status,
            sourcePath: snap.sourcePath,
            targetPath: snap.targetPath,
            transformation: snap.transformation,
          },
        });
        const graph = await adapter.invoke('graph.get', {
          projectId,
          kind: stateRef.current.graphKind,
        });
        setState((prev) => ({
          ...applyUpdatedDependencies(prev, updated, graph),
          history: nextHistory,
        }));
      } else if (mutation.kind === 'cleaner') {
        const cfg = direction === 'undo' ? mutation.before : mutation.after;
        const analysis = await adapter.invoke('analysis.start', {
          projectId,
          cleanerPatch: { replaceConfig: cfg },
        });
        await applyAnalysisResponse(analysis, nextHistory);
      } else if (mutation.kind === 'endpoint_override') {
        const overrides = direction === 'undo' ? mutation.before : mutation.after;
        const analysis = await adapter.invoke('analysis.start', {
          projectId,
          endpointOverrides: overrides,
        });
        await applyAnalysisResponse(analysis, nextHistory);
      }
    },
    [adapter, applyAnalysisResponse],
  );

  const undo = useCallback(async () => {
    const { stack, item } = stepUndo(stateRef.current.history);
    if (!item) {
      return;
    }
    await applyMutationTarget(item, 'undo', stack);
  }, [applyMutationTarget]);

  const redo = useCallback(async () => {
    const { stack, item } = stepRedo(stateRef.current.history);
    if (!item) {
      return;
    }
    await applyMutationTarget(item, 'redo', stack);
  }, [applyMutationTarget]);

  const exportArtifact = useCallback(
    async (
      format: 'openapi_json' | 'markdown_report' | 'graph_json' | 'graph_svg',
      graphKind?: GraphKind,
    ) => {
      const projectId = stateRef.current.manifest?.id ?? 'proj-demo-0001';
      return adapter.invoke('export.create', {
        projectId,
        format,
        graphKind: graphKind ?? stateRef.current.graphKind,
      });
    },
    [adapter],
  );

  const clearError = useCallback(() => {
    setState((prev) => ({ ...prev, error: null }));
  }, []);

  return {
    state,
    canUndoAction: canUndo(state.history),
    canRedoAction: canRedo(state.history),
    selectEntry,
    selectEndpoint,
    selectDependency,
    setGraphKind,
    loadDemoProject,
    importHarText,
    importHarFiles,
    confirmDependency,
    rejectDependency,
    editDependencyMapping,
    updateCleaner,
    setEndpointOverride,
    undo,
    redo,
    exportArtifact,
    clearError,
  };
}
