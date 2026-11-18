import type {
  APIModel,
  AuthFlow,
  Endpoint,
  SessionSource,
  TrafficEntrySummary,
} from '@harflow/shared-types';

const SENSITIVE_HEADERS = new Set([
  'authorization',
  'proxy-authorization',
  'cookie',
  'set-cookie',
  'x-api-key',
  'x-auth-token',
  'x-csrf-token',
  'x-xsrf-token',
]);

const SENSITIVE_KEYS = new Set([
  'password',
  'passwd',
  'secret',
  'client_secret',
  'access_token',
  'refresh_token',
  'id_token',
  'api_key',
  'apikey',
  'private_key',
  'session_token',
  'csrf_token',
]);

function fnvFingerprint(raw: string): string {
  let hash = 0x811c9dc5;
  for (let i = 0; i < raw.length; i += 1) {
    hash ^= raw.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return (hash >>> 0).toString(16).padStart(8, '0');
}

function redactHeader(name: string, value: string): { value: string; redacted: boolean } {
  const lower = name.trim().toLowerCase();
  if (SENSITIVE_HEADERS.has(lower)) {
    return {
      value: `[REDACTED:${lower.replace(/-/g, '_')}:${fnvFingerprint(value)}]`,
      redacted: true,
    };
  }
  if (value.trimStart().toLowerCase().startsWith('bearer ')) {
    return {
      value: `[REDACTED:bearer_token:${fnvFingerprint(value)}]`,
      redacted: true,
    };
  }
  return { value, redacted: false };
}

function redactQuery(name: string, value: string): { value: string; redacted: boolean } {
  const lower = name.trim().toLowerCase();
  if (SENSITIVE_KEYS.has(lower)) {
    return {
      value: `[REDACTED:${lower}:${fnvFingerprint(value)}]`,
      redacted: true,
    };
  }
  return { value, redacted: false };
}

function classifyUrl(path: string, mimeType: string): { category: string; reason: string } {
  const lowerPath = path.toLowerCase();
  const lowerMime = mimeType.toLowerCase();

  if (
    lowerPath.includes('/auth') ||
    lowerPath.includes('/login') ||
    lowerPath.includes('/oauth') ||
    lowerPath.endsWith('/token')
  ) {
    return {
      category: 'authentication',
      reason: 'Path matches authentication or token lifecycle keywords',
    };
  }
  if (lowerMime.startsWith('image/') || /\.(png|jpe?g|gif|webp|svg|ico)$/.test(lowerPath)) {
    return { category: 'images', reason: 'Image asset MIME or file extension detected' };
  }
  if (lowerMime.includes('json') || lowerPath.startsWith('/api/') || lowerPath.startsWith('/v1/')) {
    return { category: 'api', reason: 'JSON response or API route prefix detected' };
  }
  return { category: 'other', reason: 'Default classification rule' };
}

function parameterizePath(path: string): string {
  const segments = path.split('/').filter(Boolean);
  if (segments.length === 0) {
    return '/';
  }
  const normalized = segments.map((seg) => {
    if (/^v\d+$/i.test(seg)) {
      return seg;
    }
    if (/^\d+$/.test(seg) || /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(seg)) {
      return '{id}';
    }
    return seg;
  });
  return `/${normalized.join('/')}`;
}

interface RawHarEntry {
  time?: number;
  request?: {
    method?: string;
    url?: string;
    headers?: Array<{ name?: string; value?: string }>;
    queryString?: Array<{ name?: string; value?: string }>;
  };
  response?: {
    status?: number;
    content?: { size?: number; mimeType?: string };
  };
}

interface RawHarRoot {
  log?: {
    version?: string;
    entries?: RawHarEntry[];
  };
}

export function parseAndSanitizeHarClientSide(
  sourceName: string,
  rawText: string,
  sessionIndex: number,
): {
  source: SessionSource;
  entries: TrafficEntrySummary[];
  endpoints: Endpoint[];
  authFlows: AuthFlow[];
} {
  let parsed: RawHarRoot;
  try {
    parsed = JSON.parse(rawText) as RawHarRoot;
  } catch {
    throw new Error('Invalid HAR JSON input');
  }

  if (!parsed?.log?.version || !Array.isArray(parsed.log.entries)) {
    throw new Error('HAR log.version and log.entries are required');
  }

  const sessionId = `sess-${String(sessionIndex).padStart(4, '0')}`;
  let redactedSecretsCount = 0;
  const entries: TrafficEntrySummary[] = [];
  const endpointGroups = new Map<string, Endpoint>();
  const bearerReqIds: string[] = [];

  for (let i = 0; i < parsed.log.entries.length; i += 1) {
    const raw = parsed.log.entries[i];
    if (!raw?.request?.url) {
      continue;
    }
    const reqUrl = raw.request.url;
    let host = 'unknown';
    let path = '/';
    try {
      const u = new URL(reqUrl);
      host = u.hostname || 'unknown';
      path = u.pathname || '/';
    } catch {
      path = reqUrl;
    }

    const method = (raw.request.method ?? 'GET').toUpperCase();
    const mimeType = raw.response?.content?.mimeType ?? '';
    const { category, reason } = classifyUrl(path, mimeType);
    const reqId = `${sessionId}-req-${String(i + 1).padStart(4, '0')}`;

    const sanitizedHeaders = (raw.request.headers ?? []).map((h) => {
      const hName = h.name ?? '';
      const hVal = h.value ?? '';
      const res = redactHeader(hName, hVal);
      if (res.redacted) {
        redactedSecretsCount += 1;
        if (hName.toLowerCase() === 'authorization') {
          bearerReqIds.push(reqId);
        }
      }
      return { name: hName, value: res.value };
    });

    const queryParams = (raw.request.queryString ?? []).map((q) => {
      const qName = q.name ?? '';
      const qVal = q.value ?? '';
      const res = redactQuery(qName, qVal);
      if (res.redacted) {
        redactedSecretsCount += 1;
      }
      return { name: qName, value: res.value };
    });

    const pathTemplate = parameterizePath(path);
    entries.push({
      id: reqId,
      sessionId,
      sessionName: sourceName,
      method,
      status: raw.response?.status ?? 0,
      endpoint: pathTemplate,
      domain: host,
      durationMs: Math.max(0, raw.time ?? 0),
      transferSize: Math.max(0, raw.response?.content?.size ?? 0),
      category,
      classificationReason: reason,
      sanitizedHeaders,
      queryParams,
    });

    const epKey = `${method}::${host}::${pathTemplate}`;
    const existing = endpointGroups.get(epKey);
    const status = raw.response?.status ?? 0;
    if (existing) {
      existing.observedRequestIds.push(reqId);
      if (status > 0 && !existing.statusCodes.includes(status)) {
        existing.statusCodes.push(status);
      }
      existing.confidence = 0.95;
    } else {
      endpointGroups.set(epKey, {
        id: `ep-${String(endpointGroups.size + 1).padStart(4, '0')}`,
        operationId: `${method.toLowerCase()}_${pathTemplate.replace(/[^a-z0-9]+/gi, '_').replace(/^_|_$/g, '')}`,
        method,
        host,
        pathTemplate,
        parameters: [],
        requestSchemaId: null,
        responseSchemaId: null,
        statusCodes: status > 0 ? [status] : [],
        confidence: 0.85,
        category,
        observedRequestIds: [reqId],
      });
    }
  }

  const authFlows: AuthFlow[] = [];
  if (bearerReqIds.length > 0) {
    authFlows.push({
      id: 'auth-bearer',
      scheme: 'bearer',
      description: 'Observed Authorization header (redacted locally)',
      requestIds: bearerReqIds,
      confidence: 0.9,
    });
  }

  return {
    source: {
      id: sessionId,
      name: sourceName,
      harVersion: parsed.log.version,
      importedAt: new Date().toISOString(),
      entryCount: entries.length,
      redactedSecretsCount,
    },
    entries,
    endpoints: Array.from(endpointGroups.values()),
    authFlows,
  };
}

export function mergeModelUpdates(
  prev: APIModel,
  source: SessionSource,
  newEndpoints: Endpoint[],
  newAuthFlows: AuthFlow[],
  totalEntries: number,
): APIModel {
  const endpointMap = new Map(prev.endpoints.map((e) => [`${e.method}::${e.host}::${e.pathTemplate}`, e]));
  for (const ep of newEndpoints) {
    const key = `${ep.method}::${ep.host}::${ep.pathTemplate}`;
    if (!endpointMap.has(key)) {
      endpointMap.set(key, ep);
    }
  }

  return {
    ...prev,
    sources: [...prev.sources, source],
    endpoints: Array.from(endpointMap.values()),
    authFlows: newAuthFlows.length > 0 ? newAuthFlows : prev.authFlows,
    metadata: {
      ...prev.metadata,
      generatedAt: new Date().toISOString(),
      totalRequests: totalEntries,
    },
  };
}
