import type {
  APIModel,
  AuthFlow,
  Endpoint,
  FieldPresence,
  SessionSource,
  TrafficCategory,
  TrafficCookieSummary,
  TrafficEntrySummary,
  TrafficTimingsSummary,
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

const NON_SECRET_KEYS = new Set([
  'token_type',
  'page_token',
  'next_page_token',
  'prev_page_token',
  'sync_token',
  'continuation_token',
  'grant_type',
  'expires_in',
  'scope',
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
  'auth_token',
  'bearer_token',
  'token',
  'jwt',
  'session_id',
  'sid',
]);

const SENSITIVE_KEY_SUFFIXES = [
  '_token',
  '_secret',
  '_password',
  '_passwd',
  '_api_key',
  '_apikey',
  '_private_key',
  '_session_id',
];

export function normalizeKeySnake(key: string): string {
  return key
    .trim()
    .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
    .replace(/[-\s]+/g, '_')
    .toLowerCase();
}

export function isSensitiveKeyName(key: string): boolean {
  const normalized = normalizeKeySnake(key);
  if (NON_SECRET_KEYS.has(normalized)) {
    return false;
  }
  if (SENSITIVE_KEYS.has(normalized)) {
    return true;
  }
  return SENSITIVE_KEY_SUFFIXES.some((sfx) => normalized.endsWith(sfx));
}

export function fnvFingerprint(raw: string): string {
  let hash = 0x811c9dc5;
  for (let i = 0; i < raw.length; i += 1) {
    hash ^= raw.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return (hash >>> 0).toString(16).padStart(8, '0');
}

export function parseRedactedMarker(
  value: string,
): { kind: string; fingerprint: string } | null {
  const match = /^\[REDACTED:([^:\]]+):([0-9a-f]{8})\]$/i.exec(value.trim());
  if (!match) {
    return null;
  }
  return { kind: match[1]!.toLowerCase(), fingerprint: match[2]!.toLowerCase() };
}

function extractCredentialPayload(headerName: string, value: string): string {
  const trimmed = value.trim();
  const lowerHeader = headerName.trim().toLowerCase();
  if (
    (lowerHeader === 'authorization' || lowerHeader === 'proxy-authorization') &&
    trimmed.toLowerCase().startsWith('bearer ')
  ) {
    return trimmed.slice(7).trim();
  }
  if (lowerHeader === 'set-cookie') {
    const semi = trimmed.indexOf(';');
    const pair = semi >= 0 ? trimmed.slice(0, semi) : trimmed;
    const eq = pair.indexOf('=');
    return eq >= 0 ? pair.slice(eq + 1).trim() : pair.trim();
  }
  return trimmed;
}

function looksLikeJwt(value: string): boolean {
  const parts = value.trim().split('.');
  return (
    parts.length === 3 &&
    parts[0]!.startsWith('eyJ') &&
    parts[1]!.length >= 6 &&
    parts[2]!.length >= 4
  );
}

function redactHeader(
  name: string,
  value: string,
): { value: string; redacted: boolean } {
  const lower = name.trim().toLowerCase();
  if (SENSITIVE_HEADERS.has(lower)) {
    const payload = extractCredentialPayload(lower, value);
    return {
      value: `[REDACTED:${lower.replace(/-/g, '_')}:${fnvFingerprint(payload)}]`,
      redacted: true,
    };
  }
  const trimmed = value.trimStart();
  if (trimmed.toLowerCase().startsWith('bearer ')) {
    const payload = trimmed.slice(7).trim();
    return {
      value: `[REDACTED:bearer_token:${fnvFingerprint(payload)}]`,
      redacted: true,
    };
  }
  if (trimmed.toLowerCase().startsWith('basic ') || looksLikeJwt(trimmed)) {
    return {
      value: `[REDACTED:credential:${fnvFingerprint(trimmed)}]`,
      redacted: true,
    };
  }
  return { value, redacted: false };
}

function redactQuery(
  name: string,
  value: string,
): { value: string; redacted: boolean } {
  if (parseRedactedMarker(value)) {
    return { value, redacted: false };
  }
  if (isSensitiveKeyName(name)) {
    const kind = normalizeKeySnake(name) || 'query';
    return {
      value: `[REDACTED:${kind}:${fnvFingerprint(value.trim())}]`,
      redacted: true,
    };
  }
  if (looksLikeJwt(value)) {
    return {
      value: `[REDACTED:jwt:${fnvFingerprint(value.trim())}]`,
      redacted: true,
    };
  }
  return { value, redacted: false };
}

function sanitizeJsonValue(
  value: unknown,
  path: string,
  redactedFields: string[],
): unknown {
  if (Array.isArray(value)) {
    return value.map((item, idx) =>
      sanitizeJsonValue(item, `${path}[${idx}]`, redactedFields),
    );
  }
  if (typeof value === 'string') {
    if (!parseRedactedMarker(value) && looksLikeJwt(value)) {
      redactedFields.push(`body.${path}`);
      return `[REDACTED:jwt:${fnvFingerprint(value.trim())}]`;
    }
    return value;
  }
  if (value !== null && typeof value === 'object') {
    const out: Record<string, unknown> = {};
    for (const [key, rawChild] of Object.entries(value as Record<string, unknown>)) {
      const childPath = path ? `${path}.${key}` : key;
      if (
        isSensitiveKeyName(key) &&
        typeof rawChild === 'string' &&
        rawChild.length > 0 &&
        !parseRedactedMarker(rawChild)
      ) {
        const kind = normalizeKeySnake(key);
        redactedFields.push(`body.${childPath}`);
        out[key] = `[REDACTED:${kind}:${fnvFingerprint(rawChild.trim())}]`;
      } else {
        out[key] = sanitizeJsonValue(rawChild, childPath, redactedFields);
      }
    }
    return out;
  }
  return value;
}

function sanitizeBodyText(
  rawText: string | undefined,
  mimeType: string | undefined,
  harEncoding: string | undefined,
  redactedFields: string[],
): string | null {
  if (rawText === undefined) {
    return null;
  }
  let decoded = rawText;
  if (harEncoding?.toLowerCase() === 'base64') {
    try {
      if (typeof atob === 'function') {
        decoded = atob(rawText);
      }
    } catch {
      return rawText;
    }
  }

  const trimmed = decoded.trim();
  if (!trimmed) {
    return decoded;
  }

  if (
    mimeType?.toLowerCase().includes('json') ||
    trimmed.startsWith('{') ||
    trimmed.startsWith('[')
  ) {
    try {
      const parsed: unknown = JSON.parse(trimmed);
      const sanitized = sanitizeJsonValue(parsed, '$', redactedFields);
      return JSON.stringify(sanitized, null, 2);
    } catch {
      return decoded;
    }
  }

  if (mimeType?.toLowerCase().includes('x-www-form-urlencoded')) {
    const params = new URLSearchParams(trimmed);
    const out = new URLSearchParams();
    for (const [k, v] of params.entries()) {
      const res = redactQuery(k, v);
      if (res.redacted) {
        redactedFields.push(`form.${k}`);
      }
      out.append(k, res.value);
    }
    return out.toString();
  }

  return decoded;
}

export function classifyEntry(
  method: string,
  path: string,
  mimeType: string,
  host: string,
  requestHeaders: Array<{ name: string; value: string }>,
): { category: TrafficCategory; reason: string } {
  const pathLower = path.toLowerCase();
  const mimeLower = mimeType.toLowerCase();
  const hostLower = host.toLowerCase();
  const methodUpper = method.trim().toUpperCase();

  if (
    methodUpper === 'OPTIONS' &&
    requestHeaders.some(
      (h) => h.name.trim().toLowerCase() === 'access-control-request-method',
    )
  ) {
    return {
      category: 'other',
      reason: 'CORS preflight request (OPTIONS)',
    };
  }

  if (
    ['/auth', '/login', '/logout', '/signin', '/signup', '/oauth', '/sso', '/saml'].some(
      (kw) => pathLower.includes(kw),
    ) ||
    pathLower.endsWith('/token') ||
    pathLower.endsWith('/refresh')
  ) {
    return {
      category: 'authentication',
      reason: 'Path matches authentication or token lifecycle keywords',
    };
  }

  if (
    ['doubleclick', 'googlesyndication', 'adservice', 'adsystem'].some((kw) =>
      hostLower.includes(kw),
    ) ||
    ['/ads/', '/adserver', '/pixel'].some((kw) => pathLower.includes(kw))
  ) {
    return {
      category: 'advertising',
      reason: 'Host or path matches advertising/tracking signatures',
    };
  }

  if (
    [
      'google-analytics',
      'googletagmanager',
      'segment.io',
      'mixpanel',
      'amplitude',
      'posthog',
      'plausible',
      'analytics',
    ].some((kw) => hostLower.includes(kw)) ||
    ['/analytics', '/collect'].some((kw) => pathLower.includes(kw))
  ) {
    return {
      category: 'analytics',
      reason: 'Host or path matches analytics endpoint',
    };
  }

  if (
    ['sentry', 'datadog', 'newrelic', 'bugsnag', 'otel'].some((kw) =>
      hostLower.includes(kw),
    ) ||
    ['/telemetry', '/metrics', '/traces'].some((kw) => pathLower.includes(kw))
  ) {
    return {
      category: 'telemetry',
      reason: 'Host or path matches telemetry or observability endpoint',
    };
  }

  if (
    mimeLower.startsWith('image/') ||
    /\.(png|jpe?g|gif|webp|svg|ico)$/.test(pathLower)
  ) {
    return {
      category: 'images',
      reason: 'Response MIME or path extension indicates an image asset',
    };
  }

  if (
    mimeLower.includes('font') ||
    /\.(woff2?|ttf|otf|eot)$/.test(pathLower)
  ) {
    return {
      category: 'fonts',
      reason: 'Response MIME or path extension indicates a font file',
    };
  }

  if (mimeLower.includes('css') || pathLower.endsWith('.css')) {
    return {
      category: 'css',
      reason: 'Stylesheet asset detected',
    };
  }

  if (
    mimeLower.includes('javascript') ||
    /\.(js|mjs)$/.test(pathLower)
  ) {
    return {
      category: 'javascript',
      reason: 'JavaScript bundle asset detected',
    };
  }

  if (
    mimeLower.startsWith('video/') ||
    mimeLower.startsWith('audio/') ||
    /\.(mp4|webm|mp3|m3u8)$/.test(pathLower)
  ) {
    return {
      category: 'media',
      reason: 'Audio or video media stream detected',
    };
  }

  if (
    mimeLower.includes('text/html') ||
    mimeLower.includes('application/pdf') ||
    /\.(html?|pdf)$/.test(pathLower)
  ) {
    return {
      category: 'documents',
      reason: 'HTML or document response detected',
    };
  }

  if (
    /\.(map|wasm)$/.test(pathLower) ||
    pathLower.startsWith('/static/') ||
    pathLower.startsWith('/assets/')
  ) {
    return {
      category: 'static_assets',
      reason: 'Static bundle or asset path detected',
    };
  }

  if (
    mimeLower.includes('json') ||
    pathLower.startsWith('/api/') ||
    /^\/v\d+\//.test(pathLower) ||
    pathLower.includes('/graphql') ||
    ['POST', 'PUT', 'PATCH', 'DELETE'].includes(methodUpper)
  ) {
    return {
      category: 'api',
      reason: 'JSON payload, API route prefix, or mutating REST method detected',
    };
  }

  return { category: 'other', reason: 'Default classification rule' };
}

export function splitFormatSuffix(segment: string): { stem: string; suffix: string } {
  for (const sfx of ['.json', '.xml']) {
    if (segment.length > sfx.length && segment.toLowerCase().endsWith(sfx)) {
      return {
        stem: segment.slice(0, segment.length - sfx.length),
        suffix: segment.slice(segment.length - sfx.length),
      };
    }
  }
  return { stem: segment, suffix: '' };
}

function singularizeSegmentParamCamel(prev: string): string {
  const clean = prev.toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/^_|_$/g, '');
  if (!clean || /^v\d+$/i.test(clean) || clean === 'api') {
    return 'id';
  }
  let singular = clean;
  if (clean.endsWith('species') || clean.endsWith('series')) {
    singular = clean;
  } else if (clean.endsWith('ies') && clean.length > 4) {
    singular = `${clean.slice(0, -3)}y`;
  } else if (clean.endsWith('sses') || clean.endsWith('xes') || clean.endsWith('ches')) {
    singular = clean.slice(0, -2);
  } else if (clean.length > 2 && clean.endsWith('s') && !clean.endsWith('ss')) {
    singular = clean.slice(0, -1);
  }
  const camel = singular.replace(/_([a-z0-9])/g, (_, ch: string) => ch.toUpperCase());
  return `${camel}Id`;
}

export function parameterizePath(path: string): string {
  const segments = path.split('/').filter(Boolean);
  if (segments.length === 0) {
    return '/';
  }
  const normalized = segments.map((seg, idx) => {
    const { stem, suffix } = splitFormatSuffix(seg);
    if (/^v\d+$/i.test(stem)) {
      return seg;
    }
    if (
      /^\d+$/.test(stem) ||
      /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(stem) ||
      /^(ord|usr|cus|sess|tok|item)_[a-z0-9_-]+$/i.test(stem)
    ) {
      const prev = idx > 0 ? segments[idx - 1] ?? '' : '';
      const paramName =
        prev === 'orders'
          ? 'orderId'
          : prev === 'users'
            ? 'userId'
            : prev === 'item'
              ? 'itemId'
              : prev
                ? singularizeSegmentParamCamel(prev)
                : 'id';
      return `{${paramName}}${suffix}`;
    }
    return seg;
  });
  return `/${normalized.join('/')}`;
}

interface RawHarCookie {
  name?: string;
  value?: string;
  httpOnly?: boolean;
  secure?: boolean;
  sameSite?: string;
}

interface RawHarEntry {
  startedDateTime?: string;
  time?: number;
  timings?: {
    blocked?: number;
    dns?: number;
    connect?: number;
    ssl?: number;
    send?: number;
    wait?: number;
    receive?: number;
  };
  request?: {
    method?: string;
    url?: string;
    headers?: Array<{ name?: string; value?: string }>;
    queryString?: Array<{ name?: string; value?: string }>;
    cookies?: RawHarCookie[];
    postData?: {
      mimeType?: string;
      text?: string | null;
      params?: Array<{ name?: string; value?: string }>;
    };
  };
  response?: {
    status?: number;
    headers?: Array<{ name?: string; value?: string }>;
    cookies?: RawHarCookie[];
    content?: {
      size?: number;
      mimeType?: string;
      text?: string | null;
      encoding?: string;
    };
  };
}

interface RawHarRoot {
  log?: {
    version?: string;
    entries?: RawHarEntry[];
  };
}

function sanitizeCookies(
  cookies: RawHarCookie[] | undefined,
  prefix: string,
  redactedFields: string[],
): TrafficCookieSummary[] {
  return (cookies ?? []).map((c) => {
    const name = c.name ?? '';
    const rawVal = c.value ?? '';
    const redactedValue = `[REDACTED:cookie:${fnvFingerprint(rawVal.trim())}]`;
    redactedFields.push(`${prefix}.${name || 'cookie'}`);
    return {
      name,
      value: redactedValue,
      httpOnly: c.httpOnly,
      secure: c.secure,
      sameSite: c.sameSite,
    };
  });
}

function cleanTiming(val: number | undefined): number | undefined {
  return typeof val === 'number' && val >= 0 ? val : undefined;
}

function extractTimings(raw: RawHarEntry['timings']): TrafficTimingsSummary | undefined {
  if (!raw) {
    return undefined;
  }
  return {
    blockedMs: cleanTiming(raw.blocked),
    dnsMs: cleanTiming(raw.dns),
    connectMs: cleanTiming(raw.connect),
    sslMs: cleanTiming(raw.ssl),
    sendMs: cleanTiming(raw.send),
    waitMs: cleanTiming(raw.wait),
    receiveMs: cleanTiming(raw.receive),
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
  let totalRedactedSecretsCount = 0;
  const entries: TrafficEntrySummary[] = [];
  const endpointGroups = new Map<string, Endpoint>();
  const bearerReqIds: string[] = [];

  const rawEntries: RawHarEntry[] = parsed.log.entries;
  for (let i = 0; i < rawEntries.length; i += 1) {
    const raw: RawHarEntry | undefined = rawEntries[i];
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
    const rawReqHeaders = (raw.request.headers ?? []).map((h) => ({
      name: h.name ?? '',
      value: h.value ?? '',
    }));
    const { category, reason } = classifyEntry(method, path, mimeType, host, rawReqHeaders);
    const reqId = `${sessionId}-req-${String(i + 1).padStart(4, '0')}`;
    const redactedFields: string[] = [];

    const sanitizedHeaders = rawReqHeaders.map((h) => {
      const res = redactHeader(h.name, h.value);
      if (res.redacted) {
        redactedFields.push(`request.header.${h.name}`);
        if (h.name.toLowerCase() === 'authorization') {
          bearerReqIds.push(reqId);
        }
      }
      return { name: h.name, value: res.value };
    });

    const responseHeaders = (raw.response?.headers ?? []).map((h) => {
      const hName = h.name ?? '';
      const hVal = h.value ?? '';
      const res = redactHeader(hName, hVal);
      if (res.redacted) {
        redactedFields.push(`response.header.${hName}`);
      }
      return { name: hName, value: res.value };
    });

    const queryParams = (raw.request.queryString ?? []).map((q) => {
      const qName = q.name ?? '';
      const qVal = q.value ?? '';
      const res = redactQuery(qName, qVal);
      if (res.redacted) {
        redactedFields.push(`query.${qName}`);
      }
      return { name: qName, value: res.value };
    });

    const requestCookies = sanitizeCookies(raw.request.cookies, 'request.cookie', redactedFields);
    const responseCookies = sanitizeCookies(raw.response?.cookies, 'response.cookie', redactedFields);

    let requestBodyPresence: FieldPresence = 'missing';
    let requestBody: string | null = null;
    if (raw.request.postData) {
      if (raw.request.postData.text === null) {
        requestBodyPresence = 'null';
      } else if (
        typeof raw.request.postData.text === 'string' &&
        raw.request.postData.text.length === 0 &&
        (!raw.request.postData.params || raw.request.postData.params.length === 0)
      ) {
        requestBodyPresence = 'empty';
        requestBody = '';
      } else if (typeof raw.request.postData.text === 'string') {
        requestBodyPresence = 'value';
        requestBody = sanitizeBodyText(
          raw.request.postData.text,
          raw.request.postData.mimeType,
          undefined,
          redactedFields,
        );
      } else if (raw.request.postData.params && raw.request.postData.params.length > 0) {
        requestBodyPresence = 'value';
        const formObj: Record<string, string> = {};
        const formParams: Array<{ name?: string; value?: string }> = raw.request.postData.params;
        for (const p of formParams) {
          const pName = p.name ?? '';
          const pVal = p.value ?? '';
          const res = redactQuery(pName, pVal);
          if (res.redacted) {
            redactedFields.push(`form.${pName}`);
          }
          formObj[pName] = res.value;
        }
        requestBody = JSON.stringify(formObj, null, 2);
      }
    }

    const harEncoding = raw.response?.content?.encoding ?? null;
    const contentEncodingHeader = responseHeaders.find(
      (h) => h.name.toLowerCase() === 'content-encoding',
    );
    const contentEncoding = contentEncodingHeader?.value ?? null;

    const responseBody =
      typeof raw.response?.content?.text === 'string'
        ? sanitizeBodyText(
            raw.response.content.text,
            mimeType,
            harEncoding ?? undefined,
            redactedFields,
          )
        : null;

    totalRedactedSecretsCount += redactedFields.length;

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
      url: reqUrl,
      rawPath: path,
      startedDateTime: raw.startedDateTime,
      mimeType,
      requestCookies,
      responseHeaders,
      responseCookies,
      requestBody,
      requestBodyPresence,
      responseBody,
      contentEncoding,
      harEncoding,
      timings: extractTimings(raw.timings),
      redactedCount: redactedFields.length,
      redactedFields,
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
        operationId: `${method.toLowerCase()}_${pathTemplate.replace(/[^a-z0-9]+/gi, '_').replace(/^_|_$/g, '') || 'root'}`,
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
      redactedSecretsCount: totalRedactedSecretsCount,
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
  const endpointMap = new Map(
    prev.endpoints.map((e) => [`${e.method}::${e.host}::${e.pathTemplate}`, e]),
  );
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
