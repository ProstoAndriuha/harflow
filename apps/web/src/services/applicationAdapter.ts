import type {
  APIModel,
  AnalysisReport,
  AppCommandName,
  ApplicationAdapter,
  CommandPayloads,
  CommandResults,
  ConfirmationStatus,
  Dependency,
  GraphViewModel,
  ProjectManifest,
  TrafficEntrySummary,
} from '@harflow/shared-types';
import { mergeModelUpdates, parseAndSanitizeHarClientSide } from '../features/harImport.js';

interface DemoState {
  manifest: ProjectManifest;
  model: APIModel;
  entries: TrafficEntrySummary[];
}

function createInitialDemoState(name = 'Orders API Demo'): DemoState {
  const now = '2026-11-18T00:00:00Z';
  const projectId = 'proj-demo-0001';

  const model: APIModel = {
    schemaVersion: '1.0.0',
    projectId,
    sources: [
      {
        id: 'sess-0001',
        name: 'synthetic-orders.har.json',
        harVersion: '1.2',
        importedAt: now,
        entryCount: 3,
        redactedSecretsCount: 1,
      },
    ],
    endpoints: [
      {
        id: 'ep-0001',
        operationId: 'post_oauth_token',
        method: 'POST',
        host: 'auth.example.test',
        pathTemplate: '/oauth/token',
        parameters: [],
        requestSchemaId: null,
        responseSchemaId: null,
        statusCodes: [200],
        confidence: 0.96,
        category: 'authentication',
        observedRequestIds: ['sess-0001-req-0001'],
      },
      {
        id: 'ep-0002',
        operationId: 'post_v1_orders',
        method: 'POST',
        host: 'api.example.test',
        pathTemplate: '/v1/orders',
        parameters: [],
        requestSchemaId: null,
        responseSchemaId: null,
        statusCodes: [201],
        confidence: 0.94,
        category: 'api',
        observedRequestIds: ['sess-0001-req-0002'],
      },
      {
        id: 'ep-0003',
        operationId: 'get_v1_orders_id',
        method: 'GET',
        host: 'api.example.test',
        pathTemplate: '/v1/orders/{id}',
        parameters: [
          {
            name: 'id',
            location: 'path',
            inferredType: 'string',
            required: true,
            observedCount: 1,
          },
        ],
        requestSchemaId: null,
        responseSchemaId: null,
        statusCodes: [200],
        confidence: 0.89,
        category: 'api',
        observedRequestIds: ['sess-0001-req-0003'],
      },
    ],
    schemas: [],
    dependencies: [
      {
        id: 'dep-0001',
        source: {
          sessionId: 'sess-0001',
          requestId: 'sess-0001-req-0001',
          location: 'response_body',
          jsonPathOrKey: '$.access_token',
        },
        target: {
          sessionId: 'sess-0001',
          requestId: 'sess-0001-req-0002',
          location: 'header',
          jsonPathOrKey: 'Authorization',
        },
        type: 'response_to_header',
        transformation: 'prefix_suffix',
        confidence: 0.94,
        status: 'confirmed',
        evidence: [
          {
            sessionId: 'sess-0001',
            requestId: 'sess-0001-req-0002',
            ruleId: 'rule-bearer-fingerprint-match',
            description: 'Redacted token fingerprint matched sequential Authorization header',
            observedAt: now,
          },
        ],
        observedSessionIds: ['sess-0001'],
      },
      {
        id: 'dep-0002',
        source: {
          sessionId: 'sess-0001',
          requestId: 'sess-0001-req-0002',
          location: 'response_body',
          jsonPathOrKey: '$.orderId',
        },
        target: {
          sessionId: 'sess-0001',
          requestId: 'sess-0001-req-0003',
          location: 'path',
          jsonPathOrKey: 'id',
        },
        type: 'response_to_path',
        transformation: 'identity',
        confidence: 0.84,
        status: 'likely',
        evidence: [
          {
            sessionId: 'sess-0001',
            requestId: 'sess-0001-req-0003',
            ruleId: 'rule-value-reuse-path',
            description: 'Order identifier ord_synthetic_9001 appeared in subsequent path segment',
            observedAt: now,
          },
        ],
        observedSessionIds: ['sess-0001'],
      },
    ],
    authFlows: [
      {
        id: 'auth-bearer',
        scheme: 'bearer',
        description: 'Observed Authorization Bearer header (redacted)',
        requestIds: ['sess-0001-req-0002', 'sess-0001-req-0003'],
        confidence: 0.94,
      },
    ],
    workflows: [],
    diagnostics: [],
    metadata: {
      projectName: name,
      generatedAt: now,
      generatorVersion: '0.1.0',
      totalRequests: 3,
      excludedRequests: 0,
    },
  };

  const entries: TrafficEntrySummary[] = [
    {
      id: 'sess-0001-req-0001',
      sessionId: 'sess-0001',
      sessionName: 'synthetic-orders.har.json',
      method: 'POST',
      status: 200,
      endpoint: '/oauth/token',
      domain: 'auth.example.test',
      durationMs: 42,
      transferSize: 256,
      category: 'authentication',
      classificationReason: 'Path matches authentication or token lifecycle keywords',
      sanitizedHeaders: [{ name: 'Content-Type', value: 'application/json' }],
      queryParams: [],
    },
    {
      id: 'sess-0001-req-0002',
      sessionId: 'sess-0001',
      sessionName: 'synthetic-orders.har.json',
      method: 'POST',
      status: 201,
      endpoint: '/v1/orders',
      domain: 'api.example.test',
      durationMs: 18,
      transferSize: 128,
      category: 'api',
      classificationReason: 'JSON payload and API route prefix detected',
      sanitizedHeaders: [
        { name: 'Authorization', value: '[REDACTED:authorization:a1b2c3d4]' },
        { name: 'Content-Type', value: 'application/json' },
      ],
      queryParams: [],
    },
    {
      id: 'sess-0001-req-0003',
      sessionId: 'sess-0001',
      sessionName: 'synthetic-orders.har.json',
      method: 'GET',
      status: 200,
      endpoint: '/v1/orders/{id}',
      domain: 'api.example.test',
      durationMs: 14,
      transferSize: 112,
      category: 'api',
      classificationReason: 'JSON response and API route prefix detected',
      sanitizedHeaders: [
        { name: 'Authorization', value: '[REDACTED:authorization:a1b2c3d4]' },
      ],
      queryParams: [],
    },
  ];

  const manifest = computeManifest(projectId, name, now, model);
  return { manifest, model, entries };
}

function computeManifest(
  id: string,
  name: string,
  createdAt: string,
  model: APIModel,
  userOverrides: Record<string, ConfirmationStatus> = {},
): ProjectManifest {
  const domains = new Set(model.endpoints.map((e) => e.host));
  const unknownMappings = model.dependencies.filter((d) => d.status === 'unknown').length;
  return {
    schemaVersion: '1.0.0',
    id,
    name,
    createdAt,
    updatedAt: new Date().toISOString(),
    sources: model.sources,
    settings: {
      retainOriginalHar: false,
      maxHarSizeBytes: 100 * 1024 * 1024,
      offlineOnly: true,
    },
    statistics: {
      totalRequests: model.metadata.totalRequests,
      uniqueEndpoints: model.endpoints.length,
      apiDomains: domains.size,
      dependencies: model.dependencies.length,
      authFlows: model.authFlows.length,
      unknownMappings,
    },
    generatorVersions: { core: '0.1.0' },
    exportMetadata: { lastExportedAt: null, lastFormats: [] },
    userOverrides,
  };
}

function buildReport(model: APIModel): AnalysisReport {
  const domains = new Set(model.endpoints.map((e) => e.host));
  const unknownMappings = model.dependencies.filter((d) => d.status === 'unknown').length;
  return {
    schemaVersion: '1.0.0',
    projectId: model.projectId,
    generatedAt: new Date().toISOString(),
    totals: {
      requests: model.metadata.totalRequests,
      endpoints: model.endpoints.length,
      domains: domains.size,
      dependencies: model.dependencies.length,
      authFlows: model.authFlows.length,
      unknownMappings,
    },
    exportReadiness:
      model.endpoints.length === 0
        ? 'not_ready'
        : unknownMappings > 0
          ? 'partial'
          : 'ready',
    diagnostics: model.diagnostics,
  };
}

function buildGraph(model: APIModel): GraphViewModel {
  const reqToEndpoint = new Map<string, string>();
  for (const ep of model.endpoints) {
    for (const reqId of ep.observedRequestIds) {
      reqToEndpoint.set(reqId, ep.id);
    }
  }
  return {
    schemaVersion: '1.0.0',
    projectId: model.projectId,
    kind: 'endpoint',
    nodes: model.endpoints.map((ep) => ({
      id: ep.id,
      label: `${ep.method} ${ep.pathTemplate}`,
      nodeType: 'endpoint',
      method: ep.method,
      host: ep.host,
    })),
    edges: model.dependencies.map((dep) => ({
      id: dep.id,
      sourceNodeId: reqToEndpoint.get(dep.source.requestId) ?? dep.source.requestId,
      targetNodeId: reqToEndpoint.get(dep.target.requestId) ?? dep.target.requestId,
      status: dep.status,
      confidence: dep.confidence,
      label: `${dep.source.jsonPathOrKey} -> ${dep.target.jsonPathOrKey}`,
    })),
    hasCycles: false,
  };
}

export class InMemoryApplicationAdapter implements ApplicationAdapter {
  readonly mode = 'in_memory_web' as const;
  private state: DemoState;

  constructor(initialState: DemoState = createInitialDemoState()) {
    this.state = initialState;
  }

  async invoke<K extends AppCommandName>(
    command: K,
    payload: CommandPayloads[K],
  ): Promise<CommandResults[K]> {
    return this.dispatchSync(command, payload) as CommandResults[K];
  }

  private dispatchSync(command: AppCommandName, payload: unknown): unknown {
    switch (command) {
      case 'project.create': {
        const p = payload as CommandPayloads['project.create'];
        this.state = createInitialDemoState(p.name);
        return this.state.manifest;
      }
      case 'project.open':
        return this.state.model;
      case 'project.save':
        return this.state.manifest;
      case 'har.import': {
        const p = payload as CommandPayloads['har.import'];
        const parsed = parseAndSanitizeHarClientSide(
          p.sourceName,
          p.rawText,
          this.state.model.sources.length + 1,
        );
        const entries = [...this.state.entries, ...parsed.entries];
        const model = mergeModelUpdates(
          this.state.model,
          parsed.source,
          parsed.endpoints,
          parsed.authFlows,
          entries.length,
        );
        const manifest = computeManifest(
          this.state.manifest.id,
          this.state.manifest.name,
          this.state.manifest.createdAt,
          model,
          this.state.manifest.userOverrides,
        );
        this.state = { manifest, model, entries };
        return {
          source: parsed.source,
          entries,
          model,
          manifest,
        };
      }
      case 'analysis.start':
      case 'analysis.results': {
        const report = buildReport(this.state.model);
        return {
          model: this.state.model,
          manifest: this.state.manifest,
          entries: this.state.entries,
          report,
        };
      }
      case 'analysis.cancel':
        return { cancelled: true };
      case 'endpoints.list':
        return this.state.model.endpoints;
      case 'dependencies.list':
        return this.state.model.dependencies;
      case 'dependencies.confirm': {
        const p = payload as CommandPayloads['dependencies.confirm'];
        return this.updateDependencyStatus(p.dependencyId, 'user_confirmed');
      }
      case 'dependencies.reject': {
        const p = payload as CommandPayloads['dependencies.reject'];
        return this.updateDependencyStatus(p.dependencyId, 'rejected');
      }
      case 'graph.get':
        return buildGraph(this.state.model);
      case 'sdk.generate':
      case 'sdk.preview':
        return { files: [] };
      case 'validation.mock':
        return {
          totalChecks: this.state.model.endpoints.length,
          passed: this.state.model.endpoints.length,
          failed: 0,
          skipped: 0,
        };
      case 'export.create':
        return {
          filename: 'api-model.json',
          content: JSON.stringify(this.state.model, null, 2),
        };
    }
  }

  private updateDependencyStatus(
    dependencyId: string,
    status: ConfirmationStatus,
  ): Dependency[] {
    const updatedDeps = this.state.model.dependencies.map((dep) =>
      dep.id === dependencyId ? { ...dep, status } : dep,
    );
    const model: APIModel = { ...this.state.model, dependencies: updatedDeps };
    const userOverrides = {
      ...(this.state.manifest.userOverrides ?? {}),
      [dependencyId]: status,
    };
    const manifest = computeManifest(
      this.state.manifest.id,
      this.state.manifest.name,
      this.state.manifest.createdAt,
      model,
      userOverrides,
    );
    this.state = { ...this.state, model, manifest };
    return updatedDeps;
  }
}

declare global {
  interface Window {
    __TAURI_INTERNALS__?: {
      invoke?: <T>(cmd: string, args?: unknown) => Promise<T>;
    };
  }
}

export function createApplicationAdapter(): ApplicationAdapter {
  const tauriInvoke =
    typeof window !== 'undefined' ? window.__TAURI_INTERNALS__?.invoke : undefined;

  if (typeof tauriInvoke === 'function') {
    return {
      mode: 'tauri_ipc',
      invoke: <K extends AppCommandName>(
        command: K,
        payload: CommandPayloads[K],
      ): Promise<CommandResults[K]> =>
        tauriInvoke<CommandResults[K]>('harflow_dispatch', { command, payload }),
    };
  }

  return new InMemoryApplicationAdapter();
}
