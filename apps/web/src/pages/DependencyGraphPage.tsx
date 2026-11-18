import { useMemo, useState, type FormEvent } from 'react';
import type {
  ConfirmationStatus,
  Dependency,
  DependencyMappingOverride,
  DependencyType,
  GraphKind,
  GraphViewModel,
  Transformation,
} from '@harflow/shared-types';
import { Button, DependencyGraphCanvas, StatusBadge } from '@harflow/ui';
import { useI18n } from '../i18n/index.js';

const GRAPH_KINDS: readonly GraphKind[] = [
  'endpoint',
  'request_dependency',
  'auth_flow',
  'session_timeline',
  'data_flow',
] as const;

const ALL_STATUSES: readonly ConfirmationStatus[] = [
  'confirmed',
  'likely',
  'unknown',
  'rejected',
  'user_confirmed',
] as const;

const DEPENDENCY_TYPES: readonly DependencyType[] = [
  'response_to_header',
  'response_to_query',
  'response_to_body',
  'response_to_path',
  'set_cookie_to_cookie',
  'pagination_chain',
  'session_chain',
  'derived_value',
] as const;

const TRANSFORMATIONS: readonly Transformation[] = [
  'identity',
  'url_encoding',
  'base64',
  'prefix_suffix',
  'json_extract',
  'custom',
] as const;

interface MappingEditorProps {
  dep: Dependency;
  onSave: (edit: DependencyMappingOverride) => Promise<void>;
}

function MappingEditor({ dep, onSave }: MappingEditorProps) {
  const { t } = useI18n();
  const [sourcePath, setSourcePath] = useState(dep.source.jsonPathOrKey);
  const [targetPath, setTargetPath] = useState(dep.target.jsonPathOrKey);
  const [transformation, setTransformation] = useState<Transformation>(
    dep.transformation ?? 'identity',
  );

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    void onSave({
      status: 'user_confirmed',
      sourcePath: sourcePath.trim() || dep.source.jsonPathOrKey,
      targetPath: targetPath.trim() || dep.target.jsonPathOrKey,
      transformation,
    });
  };

  return (
    <form
      onSubmit={handleSubmit}
      data-testid="dependency-mapping-form"
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: '0.55rem',
        paddingTop: '0.65rem',
        borderTop: '1px solid var(--hf-border)',
      }}
    >
      <strong style={{ fontSize: '0.88rem' }}>{t.dependencies.inspector.editMapping}</strong>
      <label style={{ display: 'flex', flexDirection: 'column', gap: '0.2rem', fontSize: '0.8rem' }}>
        <span>{t.dependencies.inspector.sourcePathLabel}</span>
        <input
          type="text"
          className="hf-input"
          value={sourcePath}
          onChange={(e) => setSourcePath(e.target.value)}
        />
      </label>
      <label style={{ display: 'flex', flexDirection: 'column', gap: '0.2rem', fontSize: '0.8rem' }}>
        <span>{t.dependencies.inspector.targetPathLabel}</span>
        <input
          type="text"
          className="hf-input"
          value={targetPath}
          onChange={(e) => setTargetPath(e.target.value)}
        />
      </label>
      <label style={{ display: 'flex', flexDirection: 'column', gap: '0.2rem', fontSize: '0.8rem' }}>
        <span>{t.dependencies.inspector.transformation}</span>
        <select
          className="hf-input"
          value={transformation}
          onChange={(e) => setTransformation(e.target.value as Transformation)}
        >
          {TRANSFORMATIONS.map((tr) => (
            <option key={tr} value={tr}>
              {tr}
            </option>
          ))}
        </select>
      </label>
      <Button type="submit" variant="primary">
        {t.dependencies.inspector.saveMapping}
      </Button>
    </form>
  );
}

interface DependencyGraphPageProps {
  dependencies: Dependency[];
  graph?: GraphViewModel | null;
  graphKind?: GraphKind;
  selectedDependencyId?: string | null;
  canUndo?: boolean;
  canRedo?: boolean;
  onChangeGraphKind?: (kind: GraphKind) => Promise<void>;
  onSelectDependency?: (dependencyId: string | null) => void;
  onConfirm: (dependencyId: string) => Promise<void>;
  onReject: (dependencyId: string) => Promise<void>;
  onEditMapping?: (
    dependencyId: string,
    edit: DependencyMappingOverride,
  ) => Promise<void>;
  onUndo?: () => Promise<void>;
  onRedo?: () => Promise<void>;
  onExportGraph?: (
    format: 'graph_svg' | 'graph_json',
  ) => Promise<{ filename: string; content: string }>;
}

function triggerBrowserDownload(filename: string, content: string, mime: string) {
  if (typeof URL.createObjectURL !== 'function' || typeof document === 'undefined') {
    return;
  }
  const blob = new Blob([content], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL?.(url);
}

export function DependencyGraphPage({
  dependencies,
  graph = null,
  graphKind = 'endpoint',
  selectedDependencyId = null,
  canUndo = false,
  canRedo = false,
  onChangeGraphKind,
  onSelectDependency,
  onConfirm,
  onReject,
  onEditMapping,
  onUndo,
  onRedo,
  onExportGraph,
}: DependencyGraphPageProps) {
  const { t } = useI18n();
  const [search, setSearch] = useState('');
  const [typeFilter, setTypeFilter] = useState<string>('all');
  const [minConfidence, setMinConfidence] = useState(0);
  const [activeStatuses, setActiveStatuses] = useState<Set<ConfirmationStatus>>(
    () => new Set(ALL_STATUSES),
  );
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [exportNotice, setExportNotice] = useState<string | null>(null);

  const toggleStatus = (status: ConfirmationStatus) => {
    setActiveStatuses((prev) => {
      const next = new Set(prev);
      if (next.has(status)) {
        next.delete(status);
      } else {
        next.add(status);
      }
      return next;
    });
  };

  const filteredDeps = useMemo(() => {
    const q = search.trim().toLowerCase();
    return dependencies.filter((dep) => {
      if (!activeStatuses.has(dep.status)) {
        return false;
      }
      if (typeFilter !== 'all' && dep.type !== typeFilter) {
        return false;
      }
      if (dep.confidence * 100 < minConfidence) {
        return false;
      }
      if (q) {
        const hay =
          `${dep.id} ${dep.source.requestId} ${dep.source.jsonPathOrKey} ${dep.target.requestId} ${dep.target.jsonPathOrKey} ${dep.type}`.toLowerCase();
        if (!hay.includes(q)) {
          return false;
        }
      }
      return true;
    });
  }, [dependencies, activeStatuses, typeFilter, minConfidence, search]);

  const filteredGraph = useMemo<GraphViewModel | null>(() => {
    if (!graph) {
      return null;
    }
    const q = search.trim().toLowerCase();
    const visibleDepIds = new Set(filteredDeps.map((d) => d.id));
    const edges = graph.edges.filter((edge) => {
      if (dependencies.some((d) => d.id === edge.id)) {
        return visibleDepIds.has(edge.id);
      }
      if (!activeStatuses.has(edge.status)) {
        return false;
      }
      if (edge.confidence * 100 < minConfidence) {
        return false;
      }
      return true;
    });

    const nodes = q
      ? graph.nodes.filter(
          (n) =>
            n.label.toLowerCase().includes(q) ||
            (n.host ?? '').toLowerCase().includes(q) ||
            edges.some((e) => e.sourceNodeId === n.id || e.targetNodeId === n.id),
        )
      : graph.nodes;

    const visibleNodeIds = new Set(nodes.map((n) => n.id));
    return {
      ...graph,
      nodes,
      edges: edges.filter(
        (e) => visibleNodeIds.has(e.sourceNodeId) && visibleNodeIds.has(e.targetNodeId),
      ),
    };
  }, [graph, filteredDeps, dependencies, activeStatuses, minConfidence, search]);

  const selectedDep =
    filteredDeps.find((d) => d.id === selectedDependencyId) ??
    dependencies.find((d) => d.id === selectedDependencyId) ??
    filteredDeps[0] ??
    null;

  const handleExport = async (format: 'graph_svg' | 'graph_json') => {
    if (!onExportGraph) {
      return;
    }
    const artifact = await onExportGraph(format);
    triggerBrowserDownload(
      artifact.filename,
      artifact.content,
      format === 'graph_svg' ? 'image/svg+xml' : 'application/json',
    );
    setExportNotice(artifact.filename);
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
      <div
        style={{
          display: 'flex',
          flexWrap: 'wrap',
          alignItems: 'center',
          justifyContent: 'space-between',
          gap: '0.75rem',
        }}
      >
        <h1 style={{ margin: 0, fontSize: '1.35rem' }}>{t.dependencies.title}</h1>

        <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.45rem', alignItems: 'center' }}>
          {onUndo ? (
            <Button onClick={() => void onUndo()} disabled={!canUndo}>
              ↶ {t.undo}
            </Button>
          ) : null}
          {onRedo ? (
            <Button onClick={() => void onRedo()} disabled={!canRedo}>
              ↷ {t.redo}
            </Button>
          ) : null}
          {onExportGraph ? (
            <>
              <Button onClick={() => void handleExport('graph_svg')}>
                {t.dependencies.toolbar.exportSvg}
              </Button>
              <Button onClick={() => void handleExport('graph_json')}>
                {t.dependencies.toolbar.exportJson}
              </Button>
            </>
          ) : null}
        </div>
      </div>

      {exportNotice ? (
        <div className="hf-panel" role="status" data-testid="graph-export-status">
          Exported {exportNotice}
        </div>
      ) : null}

      {onChangeGraphKind ? (
        <div
          role="group"
          aria-label="Graph perspectives"
          style={{ display: 'flex', flexWrap: 'wrap', gap: '0.4rem' }}
        >
          {GRAPH_KINDS.map((kind) => (
            <Button
              key={kind}
              variant={graphKind === kind ? 'primary' : 'secondary'}
              aria-pressed={graphKind === kind}
              onClick={() => void onChangeGraphKind(kind)}
            >
              {t.dependencies.perspectives[kind]}
            </Button>
          ))}
        </div>
      ) : null}

      {dependencies.length === 0 ? (
        <div className="hf-panel">{t.dependencies.empty}</div>
      ) : (
        <div className="hf-studio-grid">
          <aside className="hf-panel" aria-label={t.dependencies.filters.title} style={{ display: 'flex', flexDirection: 'column', gap: '0.85rem' }}>
            <h2 style={{ margin: 0, fontSize: '0.95rem' }}>{t.dependencies.filters.title}</h2>
            <input
              type="search"
              className="hf-input"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder={t.dependencies.filters.searchPlaceholder}
              aria-label={t.dependencies.filters.searchPlaceholder}
            />

            <label style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem', fontSize: '0.82rem' }}>
              <span>{t.dependencies.inspector.type}</span>
              <select
                className="hf-input"
                value={typeFilter}
                onChange={(e) => setTypeFilter(e.target.value)}
                aria-label={t.dependencies.filters.allTypes}
              >
                <option value="all">{t.dependencies.filters.allTypes}</option>
                {DEPENDENCY_TYPES.map((dt) => (
                  <option key={dt} value={dt}>
                    {dt}
                  </option>
                ))}
              </select>
            </label>

            <label style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem', fontSize: '0.82rem' }}>
              <span>
                {t.dependencies.filters.minConfidence}: {minConfidence}%
              </span>
              <input
                type="range"
                min={0}
                max={100}
                step={5}
                value={minConfidence}
                onChange={(e) => setMinConfidence(Number(e.target.value))}
              />
            </label>

            <div style={{ display: 'flex', flexDirection: 'column', gap: '0.35rem' }}>
              <span style={{ fontSize: '0.82rem', fontWeight: 600 }}>
                {t.dependencies.filters.statuses}
              </span>
              {ALL_STATUSES.map((status) => {
                const active = activeStatuses.has(status);
                return (
                  <button
                    key={status}
                    type="button"
                    aria-pressed={active}
                    onClick={() => toggleStatus(status)}
                    className="hf-pill"
                    style={{
                      cursor: 'pointer',
                      justifyContent: 'space-between',
                      borderColor: active ? 'var(--hf-accent)' : 'var(--hf-border)',
                      color: 'var(--hf-text)',
                    }}
                  >
                    <span>{status}</span>
                    <span>{active ? '✓' : '○'}</span>
                  </button>
                );
              })}
            </div>
          </aside>

          <div style={{ display: 'flex', flexDirection: 'column', gap: '0.9rem', minWidth: 0 }}>
            {filteredGraph ? (
              <DependencyGraphCanvas
                graph={filteredGraph}
                selectedNodeId={selectedNodeId}
                selectedEdgeId={selectedDep?.id ?? null}
                onSelectNode={(nid) => setSelectedNodeId((prev) => (prev === nid ? null : nid))}
                onSelectEdge={(eid) => {
                  if (dependencies.some((d) => d.id === eid)) {
                    onSelectDependency?.(eid);
                  }
                }}
                labels={{
                  zoomIn: t.dependencies.toolbar.zoomIn,
                  zoomOut: t.dependencies.toolbar.zoomOut,
                  fitView: t.dependencies.toolbar.fitView,
                  autoLayout: t.dependencies.toolbar.autoLayout,
                  cycleWarning: t.dependencies.inspector.cycleWarning,
                }}
              />
            ) : null}

            <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
              {filteredDeps.map((dep) => {
                const isSelected = selectedDep?.id === dep.id;
                return (
                  <article
                    key={dep.id}
                    className="hf-panel"
                    data-testid={`dependency-${dep.id}`}
                    data-selected={isSelected ? 'true' : 'false'}
                    onClick={() => onSelectDependency?.(dep.id)}
                    style={{
                      cursor: 'pointer',
                      borderColor: isSelected ? 'var(--hf-accent)' : undefined,
                    }}
                  >
                    <div
                      style={{
                        display: 'flex',
                        flexWrap: 'wrap',
                        alignItems: 'center',
                        justifyContent: 'space-between',
                        gap: '0.75rem',
                      }}
                    >
                      <div
                        style={{
                          display: 'flex',
                          alignItems: 'center',
                          gap: '0.65rem',
                          flexWrap: 'wrap',
                        }}
                      >
                        <StatusBadge status={dep.status} />
                        <code className="hf-mono">
                          {dep.source.requestId} ({dep.source.jsonPathOrKey}) → {dep.target.requestId} (
                          {dep.target.jsonPathOrKey})
                        </code>
                        <span className="hf-pill">
                          {t.dependencies.confidence}: {(dep.confidence * 100).toFixed(0)}%
                        </span>
                      </div>

                      <div
                        style={{ display: 'flex', gap: '0.5rem' }}
                        onClick={(e) => e.stopPropagation()}
                      >
                        <Button
                          variant="primary"
                          onClick={() => void onConfirm(dep.id)}
                          aria-label={`${t.dependencies.confirm} ${dep.id}`}
                        >
                          {t.dependencies.confirm}
                        </Button>
                        <Button
                          variant="danger"
                          onClick={() => void onReject(dep.id)}
                          aria-label={`${t.dependencies.reject} ${dep.id}`}
                        >
                          {t.dependencies.reject}
                        </Button>
                      </div>
                    </div>

                    <div style={{ marginTop: '0.65rem', fontSize: '0.84rem' }}>
                      <strong>{t.dependencies.evidence}:</strong>
                      <ul style={{ margin: '0.3rem 0 0', paddingLeft: '1.25rem' }}>
                        {dep.evidence.map((ev, idx) => (
                          <li key={`${ev.ruleId}-${idx}`}>
                            <code className="hf-mono">{ev.ruleId}</code> — {ev.description} (
                            {ev.requestId})
                          </li>
                        ))}
                      </ul>
                    </div>
                  </article>
                );
              })}
            </div>
          </div>

          <aside
            className="hf-panel"
            aria-label={t.dependencies.inspector.title}
            data-testid="dependency-evidence-inspector"
            style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}
          >
            <h2 style={{ margin: 0, fontSize: '0.95rem' }}>
              {t.dependencies.inspector.title}
            </h2>
            {!selectedDep ? (
              <div style={{ color: 'var(--hf-text-muted)', fontSize: '0.84rem' }}>
                {t.dependencies.inspector.selectHint}
              </div>
            ) : (
              <>
                <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', flexWrap: 'wrap' }}>
                  <StatusBadge status={selectedDep.status} />
                  <code className="hf-mono">{selectedDep.id}</code>
                  <span className="hf-pill">
                    {(selectedDep.confidence * 100).toFixed(0)}%
                  </span>
                </div>
                <div style={{ fontSize: '0.82rem', display: 'grid', gap: '0.35rem' }}>
                  <div>
                    <strong>{t.dependencies.inspector.source}:</strong>{' '}
                    <code className="hf-mono">
                      {selectedDep.source.requestId} · {selectedDep.source.location}:
                      {selectedDep.source.jsonPathOrKey}
                    </code>
                  </div>
                  <div>
                    <strong>{t.dependencies.inspector.target}:</strong>{' '}
                    <code className="hf-mono">
                      {selectedDep.target.requestId} · {selectedDep.target.location}:
                      {selectedDep.target.jsonPathOrKey}
                    </code>
                  </div>
                  <div>
                    <strong>{t.dependencies.inspector.type}:</strong> {selectedDep.type}
                  </div>
                  <div>
                    <strong>{t.dependencies.inspector.sessions}:</strong>{' '}
                    {selectedDep.observedSessionIds.join(', ')}
                  </div>
                </div>

                <div style={{ fontSize: '0.82rem' }}>
                  <strong>{t.dependencies.evidence}:</strong>
                  <ul style={{ margin: '0.35rem 0 0', paddingLeft: '1.15rem' }}>
                    {selectedDep.evidence.map((ev, idx) => (
                      <li key={`${ev.ruleId}-${idx}`}>
                        <code className="hf-mono">{ev.ruleId}</code>: {ev.description}
                      </li>
                    ))}
                  </ul>
                </div>

                {onEditMapping ? (
                  <MappingEditor
                    key={`${selectedDep.id}:${selectedDep.source.jsonPathOrKey}:${selectedDep.target.jsonPathOrKey}:${selectedDep.transformation ?? 'identity'}`}
                    dep={selectedDep}
                    onSave={(edit) => onEditMapping(selectedDep.id, edit)}
                  />
                ) : null}
              </>
            )}
          </aside>
        </div>
      )}
    </div>
  );
}
