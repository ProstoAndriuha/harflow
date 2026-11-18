import type {
  APIModel,
  ConfirmationStatus,
  GraphEdge,
  GraphKind,
  GraphNode,
  GraphViewModel,
  TrafficEntrySummary,
} from '@harflow/shared-types';

function hasDirectedCycle(nodes: GraphNode[], edges: GraphEdge[]): boolean {
  const adj = new Map<string, string[]>();
  const inDegree = new Map<string, number>();

  for (const n of nodes) {
    adj.set(n.id, []);
    inDegree.set(n.id, 0);
  }
  for (const e of edges) {
    if (e.sourceNodeId === e.targetNodeId) {
      return true;
    }
    if (!adj.has(e.sourceNodeId)) adj.set(e.sourceNodeId, []);
    if (!inDegree.has(e.targetNodeId)) inDegree.set(e.targetNodeId, 0);
    adj.get(e.sourceNodeId)!.push(e.targetNodeId);
    inDegree.set(e.targetNodeId, (inDegree.get(e.targetNodeId) ?? 0) + 1);
  }

  const queue: string[] = [];
  for (const [id, deg] of inDegree.entries()) {
    if (deg === 0) queue.push(id);
  }

  let visited = 0;
  while (queue.length > 0) {
    const curr = queue.shift()!;
    visited += 1;
    for (const next of adj.get(curr) ?? []) {
      const nextDeg = (inDegree.get(next) ?? 0) - 1;
      inDegree.set(next, nextDeg);
      if (nextDeg === 0) queue.push(next);
    }
  }

  return visited < inDegree.size;
}

export function buildClientGraph(
  model: APIModel,
  entries: TrafficEntrySummary[],
  kind: GraphKind = 'endpoint',
): GraphViewModel {
  const reqToEp = new Map<string, string>();
  for (const ep of model.endpoints) {
    for (const reqId of ep.observedRequestIds) {
      reqToEp.set(reqId, ep.id);
    }
  }

  if (kind === 'endpoint' || kind === 'data_flow') {
    const nodes: GraphNode[] = model.endpoints.map((ep) => ({
      id: ep.id,
      label: `${ep.method} ${ep.pathTemplate}`,
      nodeType: 'endpoint',
      method: ep.method,
      host: ep.host,
    }));

    const edges: GraphEdge[] = model.dependencies.map((dep) => {
      const sourceNodeId = reqToEp.get(dep.source.requestId) ?? dep.source.requestId;
      const targetNodeId = reqToEp.get(dep.target.requestId) ?? dep.target.requestId;
      const isPaginationLoop =
        dep.type === 'pagination_chain' && sourceNodeId === targetNodeId;
      const label =
        kind === 'data_flow'
          ? `${dep.source.location}:${dep.source.jsonPathOrKey} -> ${dep.target.location}:${dep.target.jsonPathOrKey}`
          : `${dep.source.jsonPathOrKey} -> ${dep.target.jsonPathOrKey}`;
      return {
        id: dep.id,
        sourceNodeId,
        targetNodeId,
        status: dep.status,
        confidence: dep.confidence,
        label,
        dependencyType: dep.type,
        isPaginationLoop,
      };
    });

    return {
      schemaVersion: model.schemaVersion,
      projectId: model.projectId,
      kind,
      nodes,
      edges,
      hasCycles: hasDirectedCycle(nodes, edges),
    };
  }

  if (kind === 'request_dependency') {
    const nodeMap = new Map<string, GraphNode>();
    for (const entry of entries.filter((e) => e.kept !== false)) {
      nodeMap.set(entry.id, {
        id: entry.id,
        label: `${entry.method} ${entry.endpoint} (${entry.id})`,
        nodeType: 'request',
        method: entry.method,
        host: entry.domain,
      });
    }
    for (const dep of model.dependencies) {
      for (const reqId of [dep.source.requestId, dep.target.requestId]) {
        if (!nodeMap.has(reqId)) {
          nodeMap.set(reqId, {
            id: reqId,
            label: reqId,
            nodeType: 'request',
            method: null,
            host: null,
          });
        }
      }
    }
    const nodes = Array.from(nodeMap.values());
    const edges: GraphEdge[] = model.dependencies.map((dep) => ({
      id: dep.id,
      sourceNodeId: dep.source.requestId,
      targetNodeId: dep.target.requestId,
      status: dep.status,
      confidence: dep.confidence,
      label: `${dep.source.jsonPathOrKey} -> ${dep.target.jsonPathOrKey}`,
      dependencyType: dep.type,
      isPaginationLoop: dep.type === 'pagination_chain',
    }));
    return {
      schemaVersion: model.schemaVersion,
      projectId: model.projectId,
      kind,
      nodes,
      edges,
      hasCycles: hasDirectedCycle(nodes, edges),
    };
  }

  if (kind === 'auth_flow') {
    const nodeMap = new Map<string, GraphNode>();
    for (const ep of model.endpoints) {
      nodeMap.set(ep.id, {
        id: ep.id,
        label: `${ep.method} ${ep.pathTemplate}`,
        nodeType: 'endpoint',
        method: ep.method,
        host: ep.host,
      });
    }
    const edges: GraphEdge[] = [];
    for (const flow of model.authFlows) {
      nodeMap.set(flow.id, {
        id: flow.id,
        label: `${flow.scheme}: ${flow.id}`,
        nodeType: 'auth_flow',
        method: null,
        host: null,
      });
      const connected = new Set<string>();
      for (const reqId of flow.requestIds) {
        const epId = reqToEp.get(reqId);
        if (epId) {
          connected.add(epId);
        }
      }
      for (const epId of connected) {
        edges.push({
          id: `${flow.id}->${epId}`,
          sourceNodeId: flow.id,
          targetNodeId: epId,
          status: flow.confidence >= 0.85 ? 'confirmed' : 'likely',
          confidence: flow.confidence,
          label: flow.scheme,
          dependencyType: 'session_chain',
          isPaginationLoop: false,
        });
      }
    }
    const nodes = Array.from(nodeMap.values());
    return {
      schemaVersion: model.schemaVersion,
      projectId: model.projectId,
      kind,
      nodes,
      edges,
      hasCycles: hasDirectedCycle(nodes, edges),
    };
  }

  // session_timeline
  const kept = entries.filter((e) => e.kept !== false);
  const nodes: GraphNode[] = kept.map((entry) => ({
    id: entry.id,
    label: `[${entry.sessionId}] ${entry.method} ${entry.endpoint}`,
    nodeType: 'timeline_request',
    method: entry.method,
    host: entry.domain,
  }));
  const edges: GraphEdge[] = [];
  const bySession = new Map<string, TrafficEntrySummary[]>();
  for (const e of kept) {
    const list = bySession.get(e.sessionId) ?? [];
    list.push(e);
    bySession.set(e.sessionId, list);
  }
  for (const list of bySession.values()) {
    for (let i = 0; i + 1 < list.length; i += 1) {
      const a = list[i]!;
      const b = list[i + 1]!;
      edges.push({
        id: `seq-${a.id}->${b.id}`,
        sourceNodeId: a.id,
        targetNodeId: b.id,
        status: 'confirmed',
        confidence: 1.0,
        label: 'sequence',
        dependencyType: 'session_chain',
        isPaginationLoop: false,
      });
    }
  }
  return {
    schemaVersion: model.schemaVersion,
    projectId: model.projectId,
    kind: 'session_timeline',
    nodes,
    edges,
    hasCycles: hasDirectedCycle(nodes, edges),
  };
}

const STATUS_COLORS: Record<ConfirmationStatus, string> = {
  confirmed: '#22c55e',
  likely: '#eab308',
  unknown: '#94a3b8',
  rejected: '#ef4444',
  user_confirmed: '#3b82f6',
};

function escapeXml(raw: string): string {
  return raw
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

export function renderGraphSvg(graph: GraphViewModel): string {
  const cols = Math.max(1, Math.min(4, Math.ceil(Math.sqrt(Math.max(1, graph.nodes.length)))));
  const nodeW = 210;
  const nodeH = 56;
  const gapX = 70;
  const gapY = 65;
  const pad = 40;

  const positions = new Map<string, { x: number; y: number }>();
  graph.nodes.forEach((node, idx) => {
    const col = idx % cols;
    const row = Math.floor(idx / cols);
    positions.set(node.id, {
      x: pad + col * (nodeW + gapX),
      y: pad + row * (nodeH + gapY),
    });
  });

  const rows = Math.max(1, Math.ceil(graph.nodes.length / cols));
  const width = Math.max(480, pad * 2 + cols * nodeW + (cols - 1) * gapX);
  const height = Math.max(260, pad * 2 + rows * nodeH + (rows - 1) * gapY);

  const edgeLines = graph.edges
    .map((edge) => {
      const from = positions.get(edge.sourceNodeId);
      const to = positions.get(edge.targetNodeId);
      if (!from || !to) return '';
      const color = STATUS_COLORS[edge.status] ?? '#94a3b8';
      const x1 = from.x + nodeW / 2;
      const y1 = from.y + nodeH / 2;
      const x2 = to.x + nodeW / 2;
      const y2 = to.y + nodeH / 2;
      if (edge.sourceNodeId === edge.targetNodeId) {
        return `<path d="M ${from.x + nodeW} ${from.y + 18} C ${from.x + nodeW + 45} ${from.y - 10}, ${from.x + nodeW + 45} ${from.y + nodeH + 10}, ${from.x + nodeW} ${from.y + nodeH - 18}" fill="none" stroke="${color}" stroke-width="2" />`;
      }
      return `<line x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}" stroke="${color}" stroke-width="2" />`;
    })
    .join('\n  ');

  const nodeBoxes = graph.nodes
    .map((node) => {
      const pos = positions.get(node.id)!;
      return `<g transform="translate(${pos.x},${pos.y})">
    <rect width="${nodeW}" height="${nodeH}" rx="8" fill="#161b22" stroke="#30363d" />
    <text x="12" y="24" fill="#f0f6fc" font-family="monospace" font-size="12">${escapeXml(node.label.slice(0, 26))}</text>
    <text x="12" y="42" fill="#8b949e" font-family="sans-serif" font-size="11">${escapeXml(node.nodeType)}${node.host ? ` · ${escapeXml(node.host)}` : ''}</text>
  </g>`;
    })
    .join('\n  ');

  return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">
  <rect width="100%" height="100%" fill="#0d1117" />
  ${edgeLines}
  ${nodeBoxes}
</svg>`;
}

export function assertNoSecretLeakage(content: string): void {
  const markers = [
    ['AK', 'IA'].join(''),
    ['gh', 'p_'].join(''),
    ['github_', 'pat_'].join(''),
    ['xo', 'xb-'].join(''),
    ['-----BEGIN ', 'PRIVATE KEY-----'].join(''),
  ];
  for (const m of markers) {
    if (content.includes(m)) {
      throw new Error('Export blocked by secret-hygiene scanner');
    }
  }
}

export function buildClientOpenApiJson(model: APIModel): string {
  const paths: Record<string, Record<string, unknown>> = {};
  for (const ep of model.endpoints) {
    const pathObj = (paths[ep.pathTemplate] ??= {});
    pathObj[ep.method.toLowerCase()] = {
      operationId: ep.operationId ?? ep.id,
      parameters: ep.parameters.map((p) => ({
        name: p.name,
        in: p.location,
        required: p.required,
        schema: { type: p.inferredType === 'integer' ? 'integer' : 'string' },
      })),
      responses: Object.fromEntries(
        (ep.statusCodes.length > 0 ? ep.statusCodes : [200]).map((code) => [
          String(code),
          { description: `Observed HTTP ${code}` },
        ]),
      ),
    };
  }
  return JSON.stringify(
    {
      openapi: '3.1.0',
      info: {
        title: model.metadata.projectName,
        version: model.schemaVersion,
      },
      paths,
    },
    null,
    2,
  );
}
