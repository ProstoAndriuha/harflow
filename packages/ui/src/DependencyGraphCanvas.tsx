import { useCallback, useMemo, useRef, useState, type PointerEvent, type WheelEvent } from 'react';
import type { ConfirmationStatus, GraphEdge, GraphNode, GraphViewModel } from '@harflow/shared-types';
import { Button } from './Button.js';

const STATUS_META: Record<
  ConfirmationStatus,
  { color: string; icon: string }
> = {
  confirmed: { color: '#22c55e', icon: '✓' },
  likely: { color: '#eab308', icon: '~' },
  unknown: { color: '#94a3b8', icon: '?' },
  rejected: { color: '#ef4444', icon: '✕' },
  user_confirmed: { color: '#3b82f6', icon: '★' },
};

export interface NodePosition {
  x: number;
  y: number;
  rank: number;
}

export interface GraphLayoutResult {
  positions: Map<string, NodePosition>;
  width: number;
  height: number;
}

const NODE_WIDTH = 200;
const NODE_HEIGHT = 56;
const RANK_GAP = 80;
const ROW_GAP = 36;
const PAD = 36;

export function computeGraphLayout(
  nodes: GraphNode[],
  edges: GraphEdge[],
): GraphLayoutResult {
  const rankMap = new Map<string, number>();
  for (const n of nodes) {
    rankMap.set(n.id, 0);
  }

  // Bounded Bellman-Ford relaxation so DAGs arrange left-to-right and cycles terminate deterministically
  const maxIters = Math.max(1, nodes.length);
  for (let iter = 0; iter < maxIters; iter += 1) {
    let changed = false;
    for (const e of edges) {
      if (e.sourceNodeId === e.targetNodeId) {
        continue;
      }
      const sr = rankMap.get(e.sourceNodeId);
      const tr = rankMap.get(e.targetNodeId);
      if (sr !== undefined && tr !== undefined && tr <= sr) {
        const candidate = Math.min(maxIters - 1, sr + 1);
        if (candidate > tr) {
          rankMap.set(e.targetNodeId, candidate);
          changed = true;
        }
      }
    }
    if (!changed) {
      break;
    }
  }

  const byRank = new Map<number, GraphNode[]>();
  for (const n of nodes) {
    const r = rankMap.get(n.id) ?? 0;
    const bucket = byRank.get(r);
    if (bucket) {
      bucket.push(n);
    } else {
      byRank.set(r, [n]);
    }
  }

  const positions = new Map<string, NodePosition>();
  const ranks = Array.from(byRank.keys()).sort((a, b) => a - b);
  let maxRows = 1;

  for (const r of ranks) {
    const bucket = byRank.get(r)!;
    maxRows = Math.max(maxRows, bucket.length);
    bucket.forEach((node, idx) => {
      positions.set(node.id, {
        x: PAD + r * (NODE_WIDTH + RANK_GAP),
        y: PAD + idx * (NODE_HEIGHT + ROW_GAP),
        rank: r,
      });
    });
  }

  const rankSpan = Math.max(1, ranks.length);
  const width = Math.max(520, PAD * 2 + rankSpan * NODE_WIDTH + (rankSpan - 1) * RANK_GAP);
  const height = Math.max(260, PAD * 2 + maxRows * NODE_HEIGHT + (maxRows - 1) * ROW_GAP);

  return { positions, width, height };
}

export interface DependencyGraphCanvasProps {
  graph: GraphViewModel;
  selectedNodeId?: string | null;
  selectedEdgeId?: string | null;
  onSelectNode?: (nodeId: string) => void;
  onSelectEdge?: (edgeId: string) => void;
  labels?: {
    zoomIn?: string;
    zoomOut?: string;
    fitView?: string;
    autoLayout?: string;
    cycleWarning?: string;
    empty?: string;
  };
}

export function DependencyGraphCanvas({
  graph,
  selectedNodeId = null,
  selectedEdgeId = null,
  onSelectNode,
  onSelectEdge,
  labels,
}: DependencyGraphCanvasProps) {
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const dragRef = useRef<{ active: boolean; startX: number; startY: number; origX: number; origY: number }>({
    active: false,
    startX: 0,
    startY: 0,
    origX: 0,
    origY: 0,
  });

  const layout = useMemo(
    () => computeGraphLayout(graph.nodes, graph.edges),
    [graph.nodes, graph.edges],
  );

  const highlightedNodes = useMemo(() => {
    const set = new Set<string>();
    if (selectedNodeId) {
      set.add(selectedNodeId);
      for (const e of graph.edges) {
        if (e.sourceNodeId === selectedNodeId || e.targetNodeId === selectedNodeId) {
          set.add(e.sourceNodeId);
          set.add(e.targetNodeId);
        }
      }
    }
    if (selectedEdgeId) {
      const edge = graph.edges.find((e) => e.id === selectedEdgeId);
      if (edge) {
        set.add(edge.sourceNodeId);
        set.add(edge.targetNodeId);
      }
    }
    return set;
  }, [graph.edges, selectedNodeId, selectedEdgeId]);

  const handleZoomIn = useCallback(() => {
    setZoom((z) => Math.min(2.5, Math.round((z + 0.2) * 100) / 100));
  }, []);

  const handleZoomOut = useCallback(() => {
    setZoom((z) => Math.max(0.4, Math.round((z - 0.2) * 100) / 100));
  }, []);

  const handleFitView = useCallback(() => {
    const fitZoom = Math.min(1, Math.round((720 / layout.width) * 100) / 100);
    setZoom(Math.max(0.5, fitZoom));
    setPan({ x: 0, y: 0 });
  }, [layout.width]);

  const handleResetLayout = useCallback(() => {
    setZoom(1);
    setPan({ x: 0, y: 0 });
  }, []);

  const handleWheel = (e: WheelEvent<HTMLDivElement>) => {
    if (e.deltaY < 0) {
      handleZoomIn();
    } else if (e.deltaY > 0) {
      handleZoomOut();
    }
  };

  const handlePointerDown = (e: PointerEvent<SVGSVGElement>) => {
    if ((e.target as HTMLElement).closest?.('[data-graph-interactive="true"]')) {
      return;
    }
    dragRef.current = {
      active: true,
      startX: e.clientX,
      startY: e.clientY,
      origX: pan.x,
      origY: pan.y,
    };
  };

  const handlePointerMove = (e: PointerEvent<SVGSVGElement>) => {
    if (!dragRef.current.active) {
      return;
    }
    const dx = e.clientX - dragRef.current.startX;
    const dy = e.clientY - dragRef.current.startY;
    setPan({ x: dragRef.current.origX + dx, y: dragRef.current.origY + dy });
  };

  const handlePointerUp = () => {
    dragRef.current.active = false;
  };

  if (graph.nodes.length === 0) {
    return (
      <div className="hf-panel" role="status">
        {labels?.empty ?? 'No nodes match current graph filters.'}
      </div>
    );
  }

  return (
    <div
      className="hf-panel"
      data-testid="dependency-graph-canvas"
      data-zoom={zoom.toFixed(2)}
      data-pan={`${pan.x},${pan.y}`}
      onWheel={handleWheel}
      style={{ display: 'flex', flexDirection: 'column', gap: '0.6rem' }}
    >
      <div
        style={{
          display: 'flex',
          flexWrap: 'wrap',
          alignItems: 'center',
          justifyContent: 'space-between',
          gap: '0.5rem',
        }}
      >
        <div style={{ display: 'flex', gap: '0.4rem', flexWrap: 'wrap' }}>
          <Button onClick={handleZoomIn}>{labels?.zoomIn ?? 'Zoom In'}</Button>
          <Button onClick={handleZoomOut}>{labels?.zoomOut ?? 'Zoom Out'}</Button>
          <Button onClick={handleFitView}>{labels?.fitView ?? 'Fit View'}</Button>
          <Button onClick={handleResetLayout}>{labels?.autoLayout ?? 'Reset Layout'}</Button>
        </div>
        <span className="hf-pill" data-testid="graph-zoom-indicator">
          {Math.round(zoom * 100)}%
        </span>
      </div>

      {graph.hasCycles ? (
        <div
          role="status"
          data-testid="graph-cycle-banner"
          style={{
            padding: '0.45rem 0.7rem',
            borderRadius: '6px',
            border: '1px solid #eab308',
            backgroundColor: 'rgba(234, 179, 8, 0.12)',
            fontSize: '0.82rem',
          }}
        >
          ↻ {labels?.cycleWarning ?? 'Cycle detected in this perspective (e.g. pagination loop).'}
        </div>
      ) : null}

      <div
        style={{
          border: '1px solid var(--hf-border, #30363d)',
          borderRadius: '6px',
          backgroundColor: 'var(--hf-bg, #0d1117)',
          overflow: 'hidden',
          height: '340px',
        }}
      >
        <svg
          width="100%"
          height="100%"
          viewBox={`0 0 ${layout.width} ${layout.height}`}
          role="img"
          aria-label={`Dependency graph (${graph.kind})`}
          onPointerDown={handlePointerDown}
          onPointerMove={handlePointerMove}
          onPointerUp={handlePointerUp}
          onPointerLeave={handlePointerUp}
          style={{ cursor: 'grab', display: 'block' }}
        >
          <defs>
            <marker
              id="hf-arrow"
              markerWidth="8"
              markerHeight="8"
              refX="7"
              refY="4"
              orient="auto"
            >
              <path d="M0,0 L8,4 L0,8 Z" fill="var(--hf-text-muted, #8b949e)" />
            </marker>
          </defs>

          <g transform={`translate(${pan.x}, ${pan.y}) scale(${zoom})`}>
            {graph.edges.map((edge) => {
              const from = layout.positions.get(edge.sourceNodeId);
              const to = layout.positions.get(edge.targetNodeId);
              if (!from || !to) {
                return null;
              }
              const meta = STATUS_META[edge.status];
              const isSelected = selectedEdgeId === edge.id;
              const hasSelection = Boolean(selectedEdgeId || selectedNodeId);
              const isConnected =
                isSelected ||
                (selectedNodeId !== null &&
                  (edge.sourceNodeId === selectedNodeId ||
                    edge.targetNodeId === selectedNodeId));
              const opacity = hasSelection && !isConnected ? 0.35 : 1;

              const isSelfLoop = edge.sourceNodeId === edge.targetNodeId;
              const labelX = isSelfLoop
                ? from.x + NODE_WIDTH + 34
                : (from.x + NODE_WIDTH + to.x) / 2;
              const labelY = isSelfLoop
                ? from.y + NODE_HEIGHT / 2
                : (from.y + NODE_HEIGHT / 2 + to.y + NODE_HEIGHT / 2) / 2 - 6;

              const pathD = isSelfLoop
                ? `M ${from.x + NODE_WIDTH} ${from.y + 16} C ${from.x + NODE_WIDTH + 52} ${from.y - 12}, ${from.x + NODE_WIDTH + 52} ${from.y + NODE_HEIGHT + 12}, ${from.x + NODE_WIDTH} ${from.y + NODE_HEIGHT - 16}`
                : `M ${from.x + NODE_WIDTH} ${from.y + NODE_HEIGHT / 2} C ${from.x + NODE_WIDTH + 36} ${from.y + NODE_HEIGHT / 2}, ${to.x - 36} ${to.y + NODE_HEIGHT / 2}, ${to.x} ${to.y + NODE_HEIGHT / 2}`;

              return (
                <g
                  key={edge.id}
                  data-graph-interactive="true"
                  data-edge-id={edge.id}
                  tabIndex={0}
                  role="button"
                  aria-label={`Edge ${edge.id}: ${edge.label} (${edge.status})`}
                  onClick={() => onSelectEdge?.(edge.id)}
                  onKeyDown={(ev) => {
                    if (ev.key === 'Enter' || ev.key === ' ') {
                      ev.preventDefault();
                      onSelectEdge?.(edge.id);
                    }
                  }}
                  style={{ cursor: 'pointer', opacity }}
                >
                  <path
                    d={pathD}
                    fill="none"
                    stroke={meta.color}
                    strokeWidth={isSelected ? 3.5 : 2}
                    strokeDasharray={edge.status === 'rejected' ? '5 4' : undefined}
                    markerEnd="url(#hf-arrow)"
                  />
                  <text
                    x={labelX}
                    y={labelY}
                    textAnchor="middle"
                    fill={meta.color}
                    fontSize="11"
                    fontWeight="600"
                  >
                    {meta.icon}
                    {edge.isPaginationLoop ? '↻ ' : ' '}
                    {edge.label.slice(0, 24)}
                  </text>
                </g>
              );
            })}

            {graph.nodes.map((node) => {
              const pos = layout.positions.get(node.id);
              if (!pos) {
                return null;
              }
              const isSelected = selectedNodeId === node.id;
              const isHighlighted = highlightedNodes.has(node.id);
              const dim =
                (selectedNodeId !== null || selectedEdgeId !== null) && !isHighlighted;

              return (
                <g
                  key={node.id}
                  transform={`translate(${pos.x}, ${pos.y})`}
                  data-graph-interactive="true"
                  data-node-id={node.id}
                  tabIndex={0}
                  role="button"
                  aria-label={`Node ${node.label}`}
                  onClick={() => onSelectNode?.(node.id)}
                  onKeyDown={(ev) => {
                    if (ev.key === 'Enter' || ev.key === ' ') {
                      ev.preventDefault();
                      onSelectNode?.(node.id);
                    }
                  }}
                  style={{ cursor: 'pointer', opacity: dim ? 0.45 : 1 }}
                >
                  <rect
                    width={NODE_WIDTH}
                    height={NODE_HEIGHT}
                    rx={8}
                    fill="var(--hf-surface, #161b22)"
                    stroke={
                      isSelected
                        ? 'var(--hf-accent, #3b82f6)'
                        : 'var(--hf-border, #30363d)'
                    }
                    strokeWidth={isSelected ? 2.5 : 1.2}
                  />
                  <text
                    x={12}
                    y={23}
                    fill="var(--hf-text, #f0f6fc)"
                    fontSize="12"
                    fontWeight="600"
                    fontFamily="ui-monospace, SFMono-Regular, monospace"
                  >
                    {node.label.slice(0, 24)}
                  </text>
                  <text
                    x={12}
                    y={41}
                    fill="var(--hf-text-muted, #8b949e)"
                    fontSize="11"
                  >
                    {node.nodeType}
                    {node.host ? ` · ${node.host}` : ''}
                  </text>
                </g>
              );
            })}
          </g>
        </svg>
      </div>
    </div>
  );
}
