import type {
  APIModel,
  AnalysisReport,
  AppCommandName,
  ApplicationAdapter,
  CleanerConfig,
  CleanerPreview,
  CommandPayloads,
  CommandResults,
  ConfirmationStatus,
  Dependency,
  DependencyMappingOverride,
  GraphKind,
  ProjectManifest,
  SessionComparisonReport,
  SessionLifecycleReport,
  TrafficEntrySummary,
} from '@harflow/shared-types';
import {
  buildClientGraph,
  buildClientOpenApiJson,
  renderGraphSvg,
  assertNoSecretLeakage,
} from '../features/graphBuilder.js';
import { parseAndSanitizeHarClientSide } from '../features/harImport.js';
import {
  DEFAULT_CLEANER_CONFIG,
  analyzeSessions,
  applyCleanerPatch,
  cloneCleanerConfig,
  detectClientDependencies,
  discoverEndpointsAndSchemas,
  evaluateCleaner,
} from '../features/intelligenceEngine.js';

interface DemoState {
  manifest: ProjectManifest;
  model: APIModel;
  rawEntries: TrafficEntrySummary[];
  entries: TrafficEntrySummary[];
  cleanerPreview: CleanerPreview;
  sessionLifecycle: SessionLifecycleReport;
  sessionComparison: SessionComparisonReport;
}

function createDemoEntries(): {
  sources: APIModel['sources'];
  rawEntries: TrafficEntrySummary[];
} {
  const now = '2026-11-18T00:00:00Z';
  const sources: APIModel['sources'] = [
    {
      id: 'sess-0001',
      name: 'synthetic-orders.har.json',
      harVersion: '1.2',
      importedAt: now,
      entryCount: 5,
      redactedSecretsCount: 4,
    },
    {
      id: 'sess-0002',
      name: 'synthetic-orders-v2.har.json',
      harVersion: '1.2',
      importedAt: now,
      entryCount: 5,
      redactedSecretsCount: 6,
    },
  ];

  const rawEntries: TrafficEntrySummary[] = [
    {
      id: 'sess-0001-req-0001',
      sessionId: 'sess-0001',
      sessionName: 'synthetic-orders.har.json',
      method: 'POST',
      status: 200,
      endpoint: '/oauth/token',
      rawPath: '/oauth/token',
      url: 'https://auth.example.test/oauth/token',
      startedDateTime: '2026-11-18T00:00:01.000Z',
      mimeType: 'application/json',
      domain: 'auth.example.test',
      durationMs: 42,
      transferSize: 256,
      category: 'authentication',
      classificationReason: 'Path matches authentication or token lifecycle keywords',
      sanitizedHeaders: [{ name: 'Content-Type', value: 'application/json' }],
      responseHeaders: [{ name: 'Content-Type', value: 'application/json' }],
      queryParams: [],
      requestCookies: [],
      responseCookies: [],
      requestBodyPresence: 'value',
      requestBody: JSON.stringify({ grant_type: 'client_credentials' }, null, 2),
      responseBody: JSON.stringify(
        {
          access_token: '[REDACTED:access_token:a1b2c3d4]',
          token_type: 'Bearer',
          expires_in: 3600,
        },
        null,
        2,
      ),
      timings: { blockedMs: 2, dnsMs: 4, connectMs: 8, sslMs: 10, sendMs: 2, waitMs: 14, receiveMs: 2 },
      redactedCount: 1,
      redactedFields: ['body.$.access_token'],
    },
    {
      id: 'sess-0001-req-0002',
      sessionId: 'sess-0001',
      sessionName: 'synthetic-orders.har.json',
      method: 'POST',
      status: 201,
      endpoint: '/v1/orders',
      rawPath: '/v1/orders',
      url: 'https://api.example.test/v1/orders',
      startedDateTime: '2026-11-18T00:00:02.000Z',
      mimeType: 'application/json',
      domain: 'api.example.test',
      durationMs: 18,
      transferSize: 224,
      category: 'api',
      classificationReason: 'JSON payload and API route prefix detected',
      sanitizedHeaders: [
        { name: 'Authorization', value: '[REDACTED:authorization:a1b2c3d4]' },
        { name: 'Content-Type', value: 'application/json' },
      ],
      responseHeaders: [{ name: 'Content-Type', value: 'application/json' }],
      queryParams: [],
      requestCookies: [],
      responseCookies: [],
      requestBodyPresence: 'value',
      requestBody: JSON.stringify({ sku: 'SKU-100', qty: 2, channel: 'web' }, null, 2),
      responseBody: JSON.stringify(
        {
          orderId: 'ord_synthetic_9001',
          status: 'created',
          notes: null,
          meta: { priority: 'normal' },
        },
        null,
        2,
      ),
      timings: { blockedMs: 1, sendMs: 2, waitMs: 12, receiveMs: 3 },
      redactedCount: 1,
      redactedFields: ['request.header.Authorization'],
    },
    {
      id: 'sess-0001-req-0003',
      sessionId: 'sess-0001',
      sessionName: 'synthetic-orders.har.json',
      method: 'GET',
      status: 200,
      endpoint: '/v1/orders/{orderId}',
      rawPath: '/v1/orders/ord_synthetic_9001',
      url: 'https://api.example.test/v1/orders/ord_synthetic_9001',
      startedDateTime: '2026-11-18T00:00:03.000Z',
      mimeType: 'application/json',
      domain: 'api.example.test',
      durationMs: 14,
      transferSize: 184,
      category: 'api',
      classificationReason: 'JSON response and API route prefix detected',
      sanitizedHeaders: [
        { name: 'Cookie', value: '[REDACTED:cookie:c1c2c3c4]' },
      ],
      responseHeaders: [{ name: 'Content-Type', value: 'application/json' }],
      queryParams: [{ name: 'expand', value: 'items' }],
      requestCookies: [{ name: 'sid', value: '[REDACTED:cookie:c1c2c3c4]', httpOnly: true, secure: true }],
      responseCookies: [],
      requestBodyPresence: 'missing',
      requestBody: null,
      responseBody: JSON.stringify(
        {
          orderId: 'ord_synthetic_9001',
          status: 'paid',
          nextCursor: 'cur_page_2',
        },
        null,
        2,
      ),
      timings: { waitMs: 11, receiveMs: 3 },
      redactedCount: 2,
      redactedFields: ['request.header.Cookie', 'request.cookie.sid'],
    },
    {
      id: 'sess-0001-req-0004',
      sessionId: 'sess-0001',
      sessionName: 'synthetic-orders.har.json',
      method: 'GET',
      status: 200,
      endpoint: '/v1/orders',
      rawPath: '/v1/orders',
      url: 'https://api.example.test/v1/orders?cursor=cur_page_2&limit=20',
      startedDateTime: '2026-11-18T00:00:04.000Z',
      mimeType: 'application/json',
      domain: 'api.example.test',
      durationMs: 16,
      transferSize: 210,
      category: 'api',
      classificationReason: 'JSON response and API route prefix detected',
      sanitizedHeaders: [],
      responseHeaders: [{ name: 'Content-Type', value: 'application/json' }],
      queryParams: [
        { name: 'cursor', value: 'cur_page_2' },
        { name: 'limit', value: '20' },
      ],
      requestCookies: [],
      responseCookies: [],
      requestBodyPresence: 'missing',
      requestBody: null,
      responseBody: JSON.stringify(
        {
          items: [{ orderId: 'ord_synthetic_9001', status: 'paid' }],
          nextCursor: 'cur_page_3',
        },
        null,
        2,
      ),
      timings: { waitMs: 12, receiveMs: 4 },
      redactedCount: 0,
      redactedFields: [],
    },
    {
      id: 'sess-0001-req-0005',
      sessionId: 'sess-0001',
      sessionName: 'synthetic-orders.har.json',
      method: 'POST',
      status: 204,
      endpoint: '/collect',
      rawPath: '/collect',
      url: 'https://analytics.example.test/collect',
      startedDateTime: '2026-11-18T00:00:05.000Z',
      mimeType: 'text/plain',
      domain: 'analytics.example.test',
      durationMs: 9,
      transferSize: 0,
      category: 'analytics',
      classificationReason: 'Host or path matches analytics endpoint',
      sanitizedHeaders: [{ name: 'Content-Type', value: 'text/plain' }],
      responseHeaders: [],
      queryParams: [],
      requestCookies: [],
      responseCookies: [],
      requestBodyPresence: 'value',
      requestBody: '{"event":"page_view"}',
      responseBody: null,
      timings: { waitMs: 7, receiveMs: 2 },
      redactedCount: 0,
      redactedFields: [],
    },
    {
      id: 'sess-0002-req-0001',
      sessionId: 'sess-0002',
      sessionName: 'synthetic-orders-v2.har.json',
      method: 'POST',
      status: 200,
      endpoint: '/oauth/token',
      rawPath: '/oauth/token',
      url: 'https://auth.example.test/oauth/token',
      startedDateTime: '2026-11-18T00:05:01.000Z',
      mimeType: 'application/json',
      domain: 'auth.example.test',
      durationMs: 39,
      transferSize: 312,
      category: 'authentication',
      classificationReason: 'Path matches authentication or token lifecycle keywords',
      sanitizedHeaders: [{ name: 'Content-Type', value: 'application/json' }],
      responseHeaders: [{ name: 'Content-Type', value: 'application/json' }],
      queryParams: [],
      requestCookies: [],
      responseCookies: [],
      requestBodyPresence: 'value',
      requestBody: JSON.stringify({ grant_type: 'refresh_token' }, null, 2),
      responseBody: JSON.stringify(
        {
          access_token: '[REDACTED:access_token:e5f6a7b8]',
          refresh_token: '[REDACTED:refresh_token:99887766]',
          token_type: 'Bearer',
          expires_in: 3600,
        },
        null,
        2,
      ),
      timings: { blockedMs: 1, dnsMs: 2, connectMs: 6, sslMs: 9, sendMs: 2, waitMs: 16, receiveMs: 3 },
      redactedCount: 2,
      redactedFields: ['body.$.access_token', 'body.$.refresh_token'],
    },
    {
      id: 'sess-0002-req-0002',
      sessionId: 'sess-0002',
      sessionName: 'synthetic-orders-v2.har.json',
      method: 'POST',
      status: 201,
      endpoint: '/v1/orders',
      rawPath: '/v1/orders',
      url: 'https://api.example.test/v1/orders',
      startedDateTime: '2026-11-18T00:05:02.000Z',
      mimeType: 'application/json',
      domain: 'api.example.test',
      durationMs: 22,
      transferSize: 248,
      category: 'api',
      classificationReason: 'JSON payload and API route prefix detected',
      sanitizedHeaders: [
        { name: 'Authorization', value: '[REDACTED:authorization:e5f6a7b8]' },
        { name: 'Content-Type', value: 'application/json' },
      ],
      responseHeaders: [{ name: 'Content-Type', value: 'application/json' }],
      queryParams: [],
      requestCookies: [],
      responseCookies: [],
      requestBodyPresence: 'value',
      requestBody: JSON.stringify(
        { sku: 'SKU-200', qty: 1, channel: 'mobile', coupon: 'SPRING' },
        null,
        2,
      ),
      responseBody: JSON.stringify(
        {
          orderId: 'ord_synthetic_9002',
          status: 'created',
          currency: 'USD',
          notes: 'gift wrap',
          meta: { priority: 'express' },
        },
        null,
        2,
      ),
      timings: { sendMs: 2, waitMs: 16, receiveMs: 4 },
      redactedCount: 1,
      redactedFields: ['request.header.Authorization'],
    },
    {
      id: 'sess-0002-req-0003',
      sessionId: 'sess-0002',
      sessionName: 'synthetic-orders-v2.har.json',
      method: 'GET',
      status: 401,
      endpoint: '/v1/orders',
      rawPath: '/v1/orders',
      url: 'https://api.example.test/v1/orders?cursor=cur_page_2&limit=50',
      startedDateTime: '2026-11-18T00:05:03.000Z',
      mimeType: 'application/json',
      domain: 'api.example.test',
      durationMs: 12,
      transferSize: 96,
      category: 'api',
      classificationReason: 'JSON response and API route prefix detected',
      sanitizedHeaders: [
        { name: 'Authorization', value: '[REDACTED:authorization:e5f6a7b8]' },
      ],
      responseHeaders: [{ name: 'Content-Type', value: 'application/json' }],
      queryParams: [
        { name: 'cursor', value: 'cur_page_2' },
        { name: 'limit', value: '50' },
      ],
      requestCookies: [],
      responseCookies: [],
      requestBodyPresence: 'missing',
      requestBody: null,
      responseBody: JSON.stringify({ error: 'token_expired' }, null, 2),
      timings: { waitMs: 10, receiveMs: 2 },
      redactedCount: 1,
      redactedFields: ['request.header.Authorization'],
    },
    {
      id: 'sess-0002-req-0004',
      sessionId: 'sess-0002',
      sessionName: 'synthetic-orders-v2.har.json',
      method: 'POST',
      status: 200,
      endpoint: '/oauth/refresh',
      rawPath: '/oauth/refresh',
      url: 'https://auth.example.test/oauth/refresh',
      startedDateTime: '2026-11-18T00:05:04.000Z',
      mimeType: 'application/json',
      domain: 'auth.example.test',
      durationMs: 31,
      transferSize: 268,
      category: 'authentication',
      classificationReason: 'Path matches authentication or token lifecycle keywords',
      sanitizedHeaders: [{ name: 'Content-Type', value: 'application/json' }],
      responseHeaders: [{ name: 'Content-Type', value: 'application/json' }],
      queryParams: [],
      requestCookies: [],
      responseCookies: [],
      requestBodyPresence: 'value',
      requestBody: JSON.stringify(
        { refresh_token: '[REDACTED:refresh_token:99887766]' },
        null,
        2,
      ),
      responseBody: JSON.stringify(
        {
          access_token: '[REDACTED:access_token:11223344]',
          token_type: 'Bearer',
          expires_in: 3600,
        },
        null,
        2,
      ),
      timings: { sendMs: 2, waitMs: 25, receiveMs: 4 },
      redactedCount: 2,
      redactedFields: ['body.$.refresh_token', 'body.$.access_token'],
    },
    {
      id: 'sess-0002-req-0005',
      sessionId: 'sess-0002',
      sessionName: 'synthetic-orders-v2.har.json',
      method: 'GET',
      status: 200,
      endpoint: '/v1/orders',
      rawPath: '/v1/orders',
      url: 'https://api.example.test/v1/orders?cursor=cur_page_3&limit=50',
      startedDateTime: '2026-11-18T00:05:05.000Z',
      mimeType: 'application/json',
      domain: 'api.example.test',
      durationMs: 15,
      transferSize: 220,
      category: 'api',
      classificationReason: 'JSON response and API route prefix detected',
      sanitizedHeaders: [
        { name: 'Authorization', value: '[REDACTED:authorization:11223344]' },
      ],
      responseHeaders: [{ name: 'Content-Type', value: 'application/json' }],
      queryParams: [
        { name: 'cursor', value: 'cur_page_3' },
        { name: 'limit', value: '50' },
      ],
      requestCookies: [],
      responseCookies: [],
      requestBodyPresence: 'missing',
      requestBody: null,
      responseBody: JSON.stringify(
        {
          items: [{ orderId: 'ord_synthetic_9002', status: 'created' }],
          nextCursor: null,
        },
        null,
        2,
      ),
      timings: { waitMs: 12, receiveMs: 3 },
      redactedCount: 1,
      redactedFields: ['request.header.Authorization'],
    },
  ];

  return { sources, rawEntries };
}

function runFullAnalysis(
  projectId: string,
  name: string,
  createdAt: string,
  sources: APIModel['sources'],
  rawEntries: TrafficEntrySummary[],
  cleanerConfig: CleanerConfig,
  endpointOverrides: Record<string, string>,
  userOverrides: Record<string, ConfirmationStatus>,
  dependencyEdits: Record<string, DependencyMappingOverride>,
  exportMetadata = { lastExportedAt: null as string | null, lastFormats: [] as string[] },
): DemoState {
  const { annotatedEntries, preview } = evaluateCleaner(rawEntries, cleanerConfig);
  const keptEntries = annotatedEntries.filter((e) => e.kept !== false);

  const { endpoints, schemas } = discoverEndpointsAndSchemas(
    keptEntries,
    endpointOverrides,
  );
  const dependencies = detectClientDependencies(
    keptEntries,
    userOverrides,
    dependencyEdits,
    endpoints,
  );
  const { authFlows, lifecycle, comparison } = analyzeSessions(
    sources,
    keptEntries,
    endpoints,
    dependencies,
  );

  const now = new Date().toISOString();
  const model: APIModel = {
    schemaVersion: '1.0.0',
    projectId,
    sources,
    endpoints,
    schemas,
    dependencies,
    authFlows,
    workflows:
      endpoints.length >= 2 && dependencies.length > 0
        ? [
            {
              id: 'wf-0001',
              name: 'Observed Primary API Flow',
              stepEndpointIds: endpoints.map((e) => e.id),
            },
          ]
        : [],
    diagnostics: [],
    metadata: {
      projectName: name,
      generatedAt: now,
      generatorVersion: '0.1.0',
      totalRequests: rawEntries.length,
      excludedRequests: preview.excludedEntries,
    },
  };

  const manifest = computeManifest(
    projectId,
    name,
    createdAt,
    model,
    cleanerConfig,
    endpointOverrides,
    userOverrides,
    dependencyEdits,
    exportMetadata,
  );

  return {
    manifest,
    model,
    rawEntries,
    entries: annotatedEntries,
    cleanerPreview: preview,
    sessionLifecycle: lifecycle,
    sessionComparison: comparison,
  };
}

function createInitialDemoState(name = 'Orders API Demo'): DemoState {
  const createdAt = '2026-11-18T00:00:00Z';
  const projectId = 'proj-demo-0001';
  const { sources, rawEntries } = createDemoEntries();
  return runFullAnalysis(
    projectId,
    name,
    createdAt,
    sources,
    rawEntries,
    cloneCleanerConfig(DEFAULT_CLEANER_CONFIG),
    {},
    {},
    {},
  );
}

function computeManifest(
  id: string,
  name: string,
  createdAt: string,
  model: APIModel,
  cleaner: CleanerConfig,
  endpointOverrides: Record<string, string>,
  userOverrides: Record<string, ConfirmationStatus>,
  dependencyEdits: Record<string, DependencyMappingOverride>,
  exportMetadata: { lastExportedAt: string | null; lastFormats: string[] },
): ProjectManifest {
  const domains = new Set(model.endpoints.map((e) => e.host));
  const unknownMappings = model.dependencies.filter(
    (d) => d.status === 'unknown',
  ).length;
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
      cleaner,
      endpointOverrides,
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
    exportMetadata,
    userOverrides,
    dependencyEdits,
  };
}

function buildReport(model: APIModel): AnalysisReport {
  const domains = new Set(model.endpoints.map((e) => e.host));
  const unknownMappings = model.dependencies.filter(
    (d) => d.status === 'unknown',
  ).length;
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
        const rawEntries = [...this.state.rawEntries, ...parsed.entries];
        const sources = [...this.state.model.sources, parsed.source];
        this.state = runFullAnalysis(
          this.state.manifest.id,
          this.state.manifest.name,
          this.state.manifest.createdAt,
          sources,
          rawEntries,
          this.state.manifest.settings.cleaner ?? DEFAULT_CLEANER_CONFIG,
          this.state.manifest.settings.endpointOverrides ?? {},
          this.state.manifest.userOverrides ?? {},
          this.state.manifest.dependencyEdits ?? {},
          this.state.manifest.exportMetadata,
        );
        return {
          source: parsed.source,
          entries: this.state.entries,
          model: this.state.model,
          manifest: this.state.manifest,
        };
      }
      case 'analysis.start': {
        const p = payload as CommandPayloads['analysis.start'];
        let nextCleaner =
          this.state.manifest.settings.cleaner ?? DEFAULT_CLEANER_CONFIG;
        if (p.cleanerPatch) {
          nextCleaner = applyCleanerPatch(nextCleaner, p.cleanerPatch);
        }
        const nextOverrides =
          p.endpointOverrides ??
          this.state.manifest.settings.endpointOverrides ??
          {};
        this.state = runFullAnalysis(
          this.state.manifest.id,
          this.state.manifest.name,
          this.state.manifest.createdAt,
          this.state.model.sources,
          this.state.rawEntries,
          nextCleaner,
          nextOverrides,
          this.state.manifest.userOverrides ?? {},
          this.state.manifest.dependencyEdits ?? {},
          this.state.manifest.exportMetadata,
        );
        return {
          model: this.state.model,
          manifest: this.state.manifest,
          entries: this.state.entries,
          report: buildReport(this.state.model),
          cleanerPreview: this.state.cleanerPreview,
          sessionLifecycle: this.state.sessionLifecycle,
          sessionComparison: this.state.sessionComparison,
        };
      }
      case 'analysis.results': {
        const report = buildReport(this.state.model);
        return {
          model: this.state.model,
          manifest: this.state.manifest,
          entries: this.state.entries,
          report,
          cleanerPreview: this.state.cleanerPreview,
          sessionLifecycle: this.state.sessionLifecycle,
          sessionComparison: this.state.sessionComparison,
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
        return this.updateDependency(
          p.dependencyId,
          p.status ?? 'user_confirmed',
          p.edit,
        );
      }
      case 'dependencies.reject': {
        const p = payload as CommandPayloads['dependencies.reject'];
        return this.updateDependency(p.dependencyId, 'rejected');
      }
      case 'graph.get': {
        const p = payload as CommandPayloads['graph.get'];
        const kind: GraphKind = p.kind ?? 'endpoint';
        return buildClientGraph(this.state.model, this.state.entries, kind);
      }
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
      case 'export.create': {
        const p = payload as CommandPayloads['export.create'];
        const kind = p.graphKind ?? 'endpoint';
        let filename = 'api-model.json';
        let content = JSON.stringify(this.state.model, null, 2);
        if (p.format === 'openapi_json') {
          filename = 'openapi.json';
          content = buildClientOpenApiJson(this.state.model);
        } else if (p.format === 'markdown_report') {
          filename = 'analysis-report.md';
          content = [
            `# ${this.state.manifest.name}`,
            `- Endpoints: ${this.state.model.endpoints.length}`,
            `- Dependencies: ${this.state.model.dependencies.length}`,
            `- Auth flows: ${this.state.model.authFlows.length}`,
          ].join('\n');
        } else if (p.format === 'graph_json') {
          filename = `graph-${kind}.json`;
          content = JSON.stringify(
            buildClientGraph(this.state.model, this.state.entries, kind),
            null,
            2,
          );
        } else if (p.format === 'graph_svg') {
          filename = `graph-${kind}.svg`;
          content = renderGraphSvg(
            buildClientGraph(this.state.model, this.state.entries, kind),
          );
        }
        assertNoSecretLeakage(content);
        return { filename, content };
      }
    }
  }

  private updateDependency(
    dependencyId: string,
    status: ConfirmationStatus,
    edit?: DependencyMappingOverride,
  ): Dependency[] {
    const prevEdits = this.state.manifest.dependencyEdits ?? {};
    const mergedEdit: DependencyMappingOverride | undefined = edit
      ? { ...prevEdits[dependencyId], ...edit, status }
      : prevEdits[dependencyId]
        ? { ...prevEdits[dependencyId], status }
        : undefined;

    const nextDependencyEdits = { ...prevEdits };
    if (mergedEdit) {
      nextDependencyEdits[dependencyId] = mergedEdit;
    }

    const updatedDeps = this.state.model.dependencies.map((dep) => {
      if (dep.id !== dependencyId) {
        return dep;
      }
      return {
        ...dep,
        status,
        source: {
          ...dep.source,
          jsonPathOrKey: edit?.sourcePath ?? dep.source.jsonPathOrKey,
        },
        target: {
          ...dep.target,
          jsonPathOrKey: edit?.targetPath ?? dep.target.jsonPathOrKey,
        },
        transformation:
          edit?.transformation !== undefined
            ? edit.transformation
            : dep.transformation,
      };
    });

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
      this.state.manifest.settings.cleaner ?? DEFAULT_CLEANER_CONFIG,
      this.state.manifest.settings.endpointOverrides ?? {},
      userOverrides,
      nextDependencyEdits,
      this.state.manifest.exportMetadata,
    );
    this.state = {
      ...this.state,
      model,
      manifest,
      sessionComparison: analyzeSessions(
        model.sources,
        this.state.entries.filter((e) => e.kept !== false),
        model.endpoints,
        updatedDeps,
      ).comparison,
    };
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
