import type {
  AuthFlow,
  AuthScheme,
  AuthSessionDiff,
  CleanerConfig,
  CleanerPreview,
  CleanerUpdatePatch,
  ConfirmationStatus,
  Dependency,
  DependencyMappingOverride,
  DependencyType,
  Endpoint,
  EndpointParameter,
  EntryFilterDecision,
  ExpirationSignal,
  PaginationInfo,
  ParameterLocation,
  ParameterVariability,
  SchemaDefinition,
  SchemaDiff,
  SchemaDriftEntry,
  SchemaField,
  SessionComparisonReport,
  SessionEndpointPresence,
  SessionLifecycleReport,
  SessionSource,
  TokenRotation,
  TrafficCategory,
  TrafficEntrySummary,
  Transformation,
  VariabilityKind,
} from '@harflow/shared-types';
import { parseRedactedMarker } from './harImport.js';

export const DEFAULT_CLEANER_CONFIG: CleanerConfig = {
  keepCategories: ['api', 'authentication'],
  includedDomains: [],
  excludedDomains: [],
  keepRequestIds: [],
  excludeRequestIds: [],
  customRules: [],
};

export function cloneCleanerConfig(config?: CleanerConfig): CleanerConfig {
  const base = config ?? DEFAULT_CLEANER_CONFIG;
  return {
    keepCategories: [...base.keepCategories],
    includedDomains: [...base.includedDomains],
    excludedDomains: [...base.excludedDomains],
    keepRequestIds: [...base.keepRequestIds],
    excludeRequestIds: [...base.excludeRequestIds],
    customRules: base.customRules.map((r) => ({ ...r })),
  };
}

export function applyCleanerPatch(
  prev: CleanerConfig,
  patch: CleanerUpdatePatch,
): CleanerConfig {
  if (patch.replaceConfig) {
    return cloneCleanerConfig(patch.replaceConfig);
  }
  const next = cloneCleanerConfig(prev);
  if (patch.keepCategories) {
    next.keepCategories = Array.from(new Set(patch.keepCategories));
  }
  if (patch.excludeDomain) {
    const dom = patch.excludeDomain.trim().toLowerCase();
    if (dom) {
      next.includedDomains = next.includedDomains.filter((d) => d.toLowerCase() !== dom);
      if (!next.excludedDomains.some((d) => d.toLowerCase() === dom)) {
        next.excludedDomains.push(dom);
      }
    }
  }
  if (patch.includeDomain) {
    const dom = patch.includeDomain.trim().toLowerCase();
    if (dom) {
      next.excludedDomains = next.excludedDomains.filter((d) => d.toLowerCase() !== dom);
      if (!next.includedDomains.some((d) => d.toLowerCase() === dom)) {
        next.includedDomains.push(dom);
      }
    }
  }
  if (patch.excludeRequest) {
    const id = patch.excludeRequest;
    next.keepRequestIds = next.keepRequestIds.filter((r) => r !== id);
    if (!next.excludeRequestIds.includes(id)) {
      next.excludeRequestIds.push(id);
    }
  }
  if (patch.restoreRequest) {
    const id = patch.restoreRequest;
    next.excludeRequestIds = next.excludeRequestIds.filter((r) => r !== id);
    if (!next.keepRequestIds.includes(id)) {
      next.keepRequestIds.push(id);
    }
  }
  if (patch.addCustomRule) {
    next.customRules = [
      ...next.customRules.filter((r) => r.id !== patch.addCustomRule!.id),
      { ...patch.addCustomRule },
    ];
  }
  if (patch.removeCustomRuleId) {
    next.customRules = next.customRules.filter(
      (r) => r.id !== patch.removeCustomRuleId,
    );
  }
  return next;
}

export function evaluateCleaner(
  entries: TrafficEntrySummary[],
  rawConfig?: CleanerConfig,
): { annotatedEntries: TrafficEntrySummary[]; preview: CleanerPreview } {
  const config = rawConfig ?? DEFAULT_CLEANER_CONFIG;
  const keepCats = new Set<string>(config.keepCategories);
  const includedDomains = new Set(config.includedDomains.map((d) => d.toLowerCase()));
  const excludedDomains = new Set(config.excludedDomains.map((d) => d.toLowerCase()));
  const keepIds = new Set(config.keepRequestIds);
  const excludeIds = new Set(config.excludeRequestIds);

  const categoryCounts: Record<string, number> = {};
  const decisions: EntryFilterDecision[] = [];
  const annotatedEntries: TrafficEntrySummary[] = [];
  let keptEntries = 0;
  let excludedEntries = 0;

  for (const entry of entries) {
    const hostLower = entry.domain.toLowerCase();
    const path = entry.rawPath ?? entry.endpoint;
    let effectiveCategory = (entry.category || 'other') as TrafficCategory;
    let kept = true;
    let filterReason = `Kept category '${effectiveCategory}'`;

    if (excludeIds.has(entry.id)) {
      kept = false;
      filterReason = 'Manually excluded request override';
    } else if (keepIds.has(entry.id)) {
      kept = true;
      filterReason = 'Manually restored request override';
    } else {
      const matchedRule = config.customRules.find((rule) => {
        if (
          rule.domainSuffix &&
          !hostLower.endsWith(rule.domainSuffix.toLowerCase())
        ) {
          return false;
        }
        if (rule.pathPrefix && !path.startsWith(rule.pathPrefix)) {
          return false;
        }
        if (
          rule.method &&
          rule.method.toUpperCase() !== entry.method.toUpperCase()
        ) {
          return false;
        }
        return true;
      });

      if (matchedRule) {
        if (matchedRule.overrideCategory) {
          effectiveCategory = matchedRule.overrideCategory;
        }
        kept = matchedRule.action === 'keep';
        filterReason = `Matched custom rule '${matchedRule.id}' (${matchedRule.action})`;
      } else if (excludedDomains.has(hostLower)) {
        kept = false;
        filterReason = `Domain '${entry.domain}' excluded by cleaner rule`;
      } else if (includedDomains.has(hostLower)) {
        kept = true;
        filterReason = `Domain '${entry.domain}' explicitly included`;
      } else if (keepCats.has(effectiveCategory)) {
        kept = true;
        filterReason = `Kept category '${effectiveCategory}'`;
      } else {
        kept = false;
        filterReason = `Filtered non-API category '${effectiveCategory}'`;
      }
    }

    categoryCounts[effectiveCategory] = (categoryCounts[effectiveCategory] ?? 0) + 1;
    if (kept) {
      keptEntries += 1;
    } else {
      excludedEntries += 1;
    }

    decisions.push({
      entryId: entry.id,
      sessionId: entry.sessionId,
      method: entry.method,
      host: entry.domain,
      path,
      category: effectiveCategory,
      classificationReason: entry.classificationReason,
      kept,
      filterReason,
    });

    annotatedEntries.push({
      ...entry,
      category: effectiveCategory,
      kept,
      filterReason,
    });
  }

  return {
    annotatedEntries,
    preview: {
      totalEntries: entries.length,
      keptEntries,
      excludedEntries,
      categoryCounts,
      decisions,
    },
  };
}

function inferPrimitiveType(val: unknown): string {
  if (val === null) {
    return 'null';
  }
  if (typeof val === 'boolean') {
    return 'boolean';
  }
  if (typeof val === 'number') {
    return Number.isInteger(val) ? 'integer' : 'number';
  }
  if (Array.isArray(val)) {
    return 'array';
  }
  if (typeof val === 'object') {
    return 'object';
  }
  if (typeof val === 'string') {
    if (
      /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
        val,
      )
    ) {
      return 'uuid';
    }
    if (/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/.test(val)) {
      return 'datetime';
    }
    return 'string';
  }
  return 'unknown';
}

function inferFieldFromObservations(
  name: string,
  values: unknown[],
  totalParentSamples: number,
  depth = 0,
): SchemaField {
  const typeCounts: Record<string, number> = {};
  let nullable = false;
  const nonNullValues: unknown[] = [];

  for (const v of values) {
    const t = inferPrimitiveType(v);
    typeCounts[t] = (typeCounts[t] ?? 0) + 1;
    if (v === null) {
      nullable = true;
    } else {
      nonNullValues.push(v);
    }
  }

  const nonNullTypes = Object.keys(typeCounts).filter((t) => t !== 'null');
  let inferredType = 'unknown';
  let unionTypes: string[] | undefined;
  let enumValues: string[] | undefined;
  let itemType: string | null | undefined;
  let nestedFields: SchemaField[] | undefined;

  if (nonNullTypes.length === 0) {
    inferredType = nullable ? 'null' : 'unknown';
  } else if (
    nonNullTypes.length === 2 &&
    nonNullTypes.includes('integer') &&
    nonNullTypes.includes('number')
  ) {
    inferredType = 'number';
  } else if (nonNullTypes.length > 1) {
    inferredType = 'union';
    unionTypes = nonNullTypes.sort();
  } else {
    inferredType = nonNullTypes[0]!;
  }

  if (inferredType === 'string' && nonNullValues.length >= 2) {
    const strVals = nonNullValues.filter((v): v is string => typeof v === 'string');
    const unique = Array.from(new Set(strVals));
    if (
      unique.length >= 2 &&
      unique.length <= 6 &&
      unique.every((s) => s.length <= 32 && !s.startsWith('[REDACTED:'))
    ) {
      inferredType = 'enum';
      enumValues = unique.sort();
    }
  }

  if (inferredType === 'object' && depth < 8) {
    const objSamples = nonNullValues.filter(
      (v): v is Record<string, unknown> =>
        v !== null && typeof v === 'object' && !Array.isArray(v),
    );
    nestedFields = inferObjectFields(objSamples, depth + 1);
  }

  if (inferredType === 'array' && depth < 8) {
    const items: unknown[] = [];
    for (const arr of nonNullValues) {
      if (Array.isArray(arr)) {
        items.push(...arr);
      }
    }
    if (items.length === 0) {
      itemType = 'unknown';
    } else {
      const itemField = inferFieldFromObservations('item', items, items.length, depth + 1);
      itemType = itemField.inferredType;
      if (itemField.nestedFields) {
        nestedFields = itemField.nestedFields;
      }
    }
  }

  return {
    name,
    inferredType,
    required: values.length === totalParentSamples,
    nullable,
    observedCount: values.length,
    ...(unionTypes ? { unionTypes } : {}),
    ...(enumValues ? { enumValues } : {}),
    ...(itemType !== undefined ? { itemType } : {}),
    ...(nestedFields ? { nestedFields } : {}),
    typeCounts,
  };
}

function inferObjectFields(
  objects: Array<Record<string, unknown>>,
  depth = 0,
): SchemaField[] {
  if (objects.length === 0) {
    return [];
  }
  const byKey = new Map<string, unknown[]>();
  for (const obj of objects) {
    for (const [k, v] of Object.entries(obj)) {
      const list = byKey.get(k);
      if (list) {
        list.push(v);
      } else {
        byKey.set(k, [v]);
      }
    }
  }
  return Array.from(byKey.entries())
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([key, vals]) =>
      inferFieldFromObservations(key, vals, objects.length, depth),
    );
}

export function inferSchemaFromJsonSamples(
  id: string,
  name: string,
  rawBodies: Array<string | null | undefined>,
): SchemaDefinition | null {
  const parsed: unknown[] = [];
  for (const raw of rawBodies) {
    if (!raw || !raw.trim()) {
      continue;
    }
    try {
      parsed.push(JSON.parse(raw));
    } catch {
      // Ignore non-JSON bodies
    }
  }
  if (parsed.length === 0) {
    return null;
  }

  const rootTypes = Array.from(new Set(parsed.map((p) => inferPrimitiveType(p))));
  if (rootTypes.length === 1 && rootTypes[0] === 'object') {
    const objs = parsed.filter(
      (p): p is Record<string, unknown> =>
        p !== null && typeof p === 'object' && !Array.isArray(p),
    );
    return {
      id,
      name,
      rootType: 'object',
      fields: inferObjectFields(objs),
      observationCount: parsed.length,
    };
  }

  if (rootTypes.length === 1 && rootTypes[0] === 'array') {
    const items: unknown[] = [];
    for (const arr of parsed) {
      if (Array.isArray(arr)) {
        items.push(...arr);
      }
    }
    if (items.length === 0) {
      return {
        id,
        name,
        rootType: 'array',
        itemType: 'unknown',
        fields: [],
        observationCount: parsed.length,
      };
    }
    const itemField = inferFieldFromObservations('item', items, items.length);
    return {
      id,
      name,
      rootType: 'array',
      itemType: itemField.inferredType,
      fields: itemField.nestedFields ?? [],
      observationCount: parsed.length,
    };
  }

  return {
    id,
    name,
    rootType: rootTypes.length === 1 ? rootTypes[0]! : 'union',
    fields: [],
    observationCount: parsed.length,
  };
}

function inferScalarStringType(values: string[]): string {
  if (values.length === 0) {
    return 'string';
  }
  if (values.every((v) => /^-?\d+$/.test(v))) {
    return 'integer';
  }
  if (values.every((v) => /^-?\d+(\.\d+)?$/.test(v))) {
    return 'number';
  }
  if (values.every((v) => v === 'true' || v === 'false')) {
    return 'boolean';
  }
  if (
    values.every((v) =>
      /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
        v,
      ),
    )
  ) {
    return 'uuid';
  }
  return 'string';
}

function detectPagination(
  queryParamNames: Set<string>,
  responseSchema: SchemaDefinition | null,
): PaginationInfo | null {
  const lowerQuery = Array.from(queryParamNames).map((q) => q.toLowerCase());
  const cursorParams = ['cursor', 'after', 'next_cursor', 'page_token'];
  const matchedCursor = cursorParams.filter((c) => lowerQuery.includes(c));
  const responseNextField = responseSchema?.fields.find((f) =>
    ['nextCursor', 'next_cursor', 'nextPageToken', 'next_token'].includes(f.name),
  )?.name;

  if (matchedCursor.length > 0 || responseNextField) {
    return {
      style: 'cursor',
      queryParams: matchedCursor.length > 0 ? matchedCursor : ['cursor'],
      responseNextField: responseNextField ?? null,
    };
  }

  if (lowerQuery.includes('page')) {
    const qp = ['page'];
    if (lowerQuery.includes('per_page')) qp.push('per_page');
    if (lowerQuery.includes('page_size')) qp.push('page_size');
    if (lowerQuery.includes('limit')) qp.push('limit');
    return { style: 'page_number', queryParams: qp, responseNextField: null };
  }

  if (lowerQuery.includes('offset') && lowerQuery.includes('limit')) {
    return {
      style: 'offset_limit',
      queryParams: ['offset', 'limit'],
      responseNextField: null,
    };
  }

  return null;
}

export function discoverEndpointsAndSchemas(
  keptEntries: TrafficEntrySummary[],
  endpointOverrides: Record<string, string> = {},
): { endpoints: Endpoint[]; schemas: SchemaDefinition[] } {
  const groups = new Map<
    string,
    {
      method: string;
      host: string;
      pathTemplate: string;
      entries: TrafficEntrySummary[];
    }
  >();

  for (const entry of keptEntries) {
    const baseKey = `${entry.method}::${entry.domain}::${entry.endpoint}`;
    const overriddenTemplate = endpointOverrides[baseKey] ?? entry.endpoint;
    const groupKey = `${entry.method}::${entry.domain}::${overriddenTemplate}`;
    const existing = groups.get(groupKey);
    if (existing) {
      existing.entries.push(entry);
    } else {
      groups.set(groupKey, {
        method: entry.method,
        host: entry.domain,
        pathTemplate: overriddenTemplate,
        entries: [entry],
      });
    }
  }

  const endpoints: Endpoint[] = [];
  const schemas: SchemaDefinition[] = [];
  let epIdx = 0;

  for (const group of groups.values()) {
    epIdx += 1;
    const epId = `ep-${String(epIdx).padStart(4, '0')}`;
    const baseKey = `${group.method}::${group.host}::${group.entries[0]?.endpoint ?? group.pathTemplate}`;
    const finalTemplate = endpointOverrides[epId] ?? endpointOverrides[baseKey] ?? group.pathTemplate;

    const statusCodes: number[] = [];
    const statusDistribution: Record<string, number> = {};
    const queryObs = new Map<string, string[]>();

    for (const e of group.entries) {
      if (e.status > 0) {
        if (!statusCodes.includes(e.status)) {
          statusCodes.push(e.status);
        }
        const key = String(e.status);
        statusDistribution[key] = (statusDistribution[key] ?? 0) + 1;
      }
      for (const q of e.queryParams) {
        const list = queryObs.get(q.name);
        if (list) {
          list.push(q.value);
        } else {
          queryObs.set(q.name, [q.value]);
        }
      }
    }

    const parameters: EndpointParameter[] = [];
    const pathParamMatches = finalTemplate.matchAll(/\{([^}]+)\}/g);
    for (const m of pathParamMatches) {
      const paramName = m[1]!;
      parameters.push({
        name: paramName,
        location: 'path',
        inferredType: 'string',
        required: true,
        observedCount: group.entries.length,
      });
    }

    for (const [qName, values] of Array.from(queryObs.entries()).sort(([a], [b]) =>
      a.localeCompare(b),
    )) {
      parameters.push({
        name: qName,
        location: 'query',
        inferredType: inferScalarStringType(values),
        required: values.length === group.entries.length,
        observedCount: values.length,
      });
    }

    const opSlug =
      `${group.method.toLowerCase()}_${finalTemplate.replace(/[^a-z0-9]+/gi, '_').replace(/^_|_$/g, '') || 'root'}`;

    const reqSchema = inferSchemaFromJsonSamples(
      `schema-req-${epId}`,
      `${opSlug}_request`,
      group.entries.map((e) => e.requestBody),
    );
    if (reqSchema) {
      schemas.push(reqSchema);
    }

    const resSchema = inferSchemaFromJsonSamples(
      `schema-res-${epId}`,
      `${opSlug}_response`,
      group.entries.map((e) => e.responseBody),
    );
    if (resSchema) {
      schemas.push(resSchema);
    }

    const pagination = detectPagination(new Set(queryObs.keys()), resSchema);
    const primaryCat = group.entries[0]?.category ?? 'api';

    endpoints.push({
      id: epId,
      operationId: opSlug,
      method: group.method,
      host: group.host,
      pathTemplate: finalTemplate,
      parameters,
      requestSchemaId: reqSchema?.id ?? null,
      responseSchemaId: resSchema?.id ?? null,
      statusCodes: statusCodes.sort((a, b) => a - b),
      statusDistribution,
      pagination,
      confidence: group.entries.length > 1 ? 0.96 : 0.9,
      category: primaryCat,
      observedRequestIds: group.entries.map((e) => e.id),
    });
  }

  return { endpoints, schemas };
}

interface ExtractedToken {
  sessionId: string;
  requestId: string;
  orderIndex: number;
  location: Dependency['source']['location'];
  jsonPathOrKey: string;
  rawValue: string;
  matchKeys: string[];
  isRedactedFp: boolean;
}

const WEAK_SCALAR_VALUES = new Set([
  'true',
  'false',
  'null',
  'none',
  'json',
  'application/json',
  'utf-8',
  'ok',
  'success',
  'error',
  'pending',
  'active',
  'prod',
  'staging',
  'dev',
]);

function isStrongValue(value: string, keyName: string): boolean {
  const trimmed = value.trim();
  if (trimmed.length < 4) {
    return false;
  }
  const lower = trimmed.toLowerCase();
  if (WEAK_SCALAR_VALUES.has(lower)) {
    return false;
  }
  if (/^[a-z]{4}$/i.test(trimmed)) {
    const keyLower = keyName.toLowerCase();
    return (
      keyLower.includes('id') ||
      keyLower.includes('token') ||
      keyLower.includes('cursor') ||
      keyLower.includes('code')
    );
  }
  return true;
}

function collectJsonLeaves(
  value: unknown,
  path: string,
  out: Array<{ path: string; key: string; value: string }>,
): void {
  if (Array.isArray(value)) {
    value.slice(0, 12).forEach((item, idx) => {
      collectJsonLeaves(item, `${path}[${idx}]`, out);
    });
    return;
  }
  if (value !== null && typeof value === 'object') {
    for (const [k, v] of Object.entries(value as Record<string, unknown>)) {
      collectJsonLeaves(v, `${path}.${k}`, out);
    }
    return;
  }
  if (typeof value === 'string' || typeof value === 'number') {
    out.push({
      path,
      key: path.split('.').pop() ?? path,
      value: String(value),
    });
  }
}

function buildMatchKeys(raw: string): { keys: string[]; isRedactedFp: boolean } {
  const redacted = parseRedactedMarker(raw);
  if (redacted) {
    return { keys: [`fp:${redacted.fingerprint}`], isRedactedFp: true };
  }
  const keys = [`raw:${raw}`];
  try {
    const decoded = decodeURIComponent(raw);
    if (decoded !== raw) {
      keys.push(`raw:${decoded}`);
    }
  } catch {
    // Ignore malformed URI components
  }
  return { keys, isRedactedFp: false };
}

function classifyDependencyType(
  sourceLoc: Dependency['source']['location'],
  targetLoc: Dependency['target']['location'],
  sourcePath: string,
  targetPath: string,
): DependencyType {
  if (
    sourcePath.toLowerCase().includes('cursor') ||
    targetPath.toLowerCase().includes('cursor')
  ) {
    return 'pagination_chain';
  }
  if (sourceLoc === 'set_cookie' && targetLoc === 'cookie') {
    return 'set_cookie_to_cookie';
  }
  if (
    sourcePath.toLowerCase().includes('access_token') ||
    sourcePath.toLowerCase().includes('refresh_token') ||
    targetPath.toLowerCase() === 'authorization'
  ) {
    return targetLoc === 'header' ? 'response_to_header' : 'session_chain';
  }
  switch (targetLoc) {
    case 'header':
      return 'response_to_header';
    case 'query':
      return 'response_to_query';
    case 'body':
      return 'response_to_body';
    case 'path':
      return 'response_to_path';
    default:
      return 'derived_value';
  }
}

export function detectClientDependencies(
  keptEntries: TrafficEntrySummary[],
  userOverrides: Record<string, ConfirmationStatus> = {},
  dependencyEdits: Record<string, DependencyMappingOverride> = {},
): Dependency[] {
  const bySession = new Map<string, TrafficEntrySummary[]>();
  for (const e of keptEntries) {
    const list = bySession.get(e.sessionId);
    if (list) {
      list.push(e);
    } else {
      bySession.set(e.sessionId, [e]);
    }
  }

  const candidateMap = new Map<
    string,
    {
      source: Dependency['source'];
      target: Dependency['target'];
      type: DependencyType;
      transformation: Transformation;
      sessions: Set<string>;
      evidence: Dependency['evidence'];
      isRedactedFp: boolean;
    }
  >();

  for (const [sessionId, sessionEntries] of bySession.entries()) {
    const producerIndex = new Map<string, ExtractedToken[]>();

    for (let idx = 0; idx < sessionEntries.length; idx += 1) {
      const entry = sessionEntries[idx]!;
      const consumers: ExtractedToken[] = [];

      // 1. Collect consumer tokens in this request (headers, query, path, cookies, body)
      for (const h of entry.sanitizedHeaders) {
        const lower = h.name.toLowerCase();
        if (['content-type', 'accept', 'user-agent', 'host'].includes(lower)) {
          continue;
        }
        const { keys, isRedactedFp } = buildMatchKeys(h.value);
        if (isRedactedFp || isStrongValue(h.value, h.name)) {
          consumers.push({
            sessionId,
            requestId: entry.id,
            orderIndex: idx,
            location: 'header',
            jsonPathOrKey: h.name,
            rawValue: h.value,
            matchKeys: keys,
            isRedactedFp,
          });
        }
      }

      for (const q of entry.queryParams) {
        const { keys, isRedactedFp } = buildMatchKeys(q.value);
        if (isRedactedFp || isStrongValue(q.value, q.name)) {
          consumers.push({
            sessionId,
            requestId: entry.id,
            orderIndex: idx,
            location: 'query',
            jsonPathOrKey: q.name,
            rawValue: q.value,
            matchKeys: keys,
            isRedactedFp,
          });
        }
      }

      for (const c of entry.requestCookies ?? []) {
        const { keys, isRedactedFp } = buildMatchKeys(c.value);
        if (isRedactedFp || isStrongValue(c.value, c.name)) {
          consumers.push({
            sessionId,
            requestId: entry.id,
            orderIndex: idx,
            location: 'cookie',
            jsonPathOrKey: c.name,
            rawValue: c.value,
            matchKeys: keys,
            isRedactedFp,
          });
        }
      }

      const rawSegs = (entry.rawPath ?? '').split('/').filter(Boolean);
      const tplSegs = entry.endpoint.split('/').filter(Boolean);
      for (let sIdx = 0; sIdx < Math.min(rawSegs.length, tplSegs.length); sIdx += 1) {
        const seg = rawSegs[sIdx]!;
        const tpl = tplSegs[sIdx]!;
        if (tpl.startsWith('{') && tpl.endsWith('}') && isStrongValue(seg, tpl)) {
          const { keys, isRedactedFp } = buildMatchKeys(seg);
          consumers.push({
            sessionId,
            requestId: entry.id,
            orderIndex: idx,
            location: 'path',
            jsonPathOrKey: tpl.slice(1, -1),
            rawValue: seg,
            matchKeys: keys,
            isRedactedFp,
          });
        }
      }

      if (entry.requestBody) {
        try {
          const parsed: unknown = JSON.parse(entry.requestBody);
          const leaves: Array<{ path: string; key: string; value: string }> = [];
          collectJsonLeaves(parsed, '$', leaves);
          for (const leaf of leaves) {
            const { keys, isRedactedFp } = buildMatchKeys(leaf.value);
            if (isRedactedFp || isStrongValue(leaf.value, leaf.key)) {
              consumers.push({
                sessionId,
                requestId: entry.id,
                orderIndex: idx,
                location: 'body',
                jsonPathOrKey: leaf.path,
                rawValue: leaf.value,
                matchKeys: keys,
                isRedactedFp,
              });
            }
          }
        } catch {
          // Ignore non-JSON body
        }
      }

      // Match consumers against strictly earlier producers
      for (const consumer of consumers) {
        for (const mk of consumer.matchKeys) {
          const producers = producerIndex.get(mk);
          if (!producers || producers.length === 0) {
            continue;
          }
          // Prefer most recent earlier producer
          const producer = producers[producers.length - 1]!;
          if (producer.orderIndex >= consumer.orderIndex) {
            continue;
          }
          const depType = classifyDependencyType(
            producer.location,
            consumer.location,
            producer.jsonPathOrKey,
            consumer.jsonPathOrKey,
          );
          const transformation: Transformation =
            producer.isRedactedFp && consumer.jsonPathOrKey.toLowerCase() === 'authorization'
              ? 'prefix_suffix'
              : 'identity';

          const pairKey = `${producer.requestId}::${producer.jsonPathOrKey}->${consumer.requestId}::${consumer.jsonPathOrKey}`;
          const existing = candidateMap.get(pairKey);
          const ev = {
            sessionId,
            requestId: consumer.requestId,
            ruleId: producer.isRedactedFp
              ? 'rule-bearer-fingerprint-match'
              : `rule-value-reuse-${consumer.location}`,
            description: producer.isRedactedFp
              ? `Redacted token fingerprint matched ${producer.jsonPathOrKey} -> ${consumer.jsonPathOrKey}`
              : `Observed value from ${producer.jsonPathOrKey} reused in ${consumer.location}:${consumer.jsonPathOrKey}`,
            observedAt: entry.startedDateTime ?? new Date().toISOString(),
          };
          if (existing) {
            existing.sessions.add(sessionId);
            existing.evidence.push(ev);
          } else {
            candidateMap.set(pairKey, {
              source: {
                sessionId: producer.sessionId,
                requestId: producer.requestId,
                location: producer.location,
                jsonPathOrKey: producer.jsonPathOrKey,
              },
              target: {
                sessionId: consumer.sessionId,
                requestId: consumer.requestId,
                location: consumer.location,
                jsonPathOrKey: consumer.jsonPathOrKey,
              },
              type: depType,
              transformation,
              sessions: new Set([sessionId]),
              evidence: [ev],
              isRedactedFp: producer.isRedactedFp,
            });
          }
          break;
        }
      }

      // 2. Register producers from this response
      const producers: ExtractedToken[] = [];
      for (const h of entry.responseHeaders ?? []) {
        const lower = h.name.toLowerCase();
        const { keys, isRedactedFp } = buildMatchKeys(h.value);
        if (lower === 'set-cookie' || isRedactedFp || isStrongValue(h.value, h.name)) {
          producers.push({
            sessionId,
            requestId: entry.id,
            orderIndex: idx,
            location: lower === 'set-cookie' ? 'set_cookie' : 'response_header',
            jsonPathOrKey: h.name,
            rawValue: h.value,
            matchKeys: keys,
            isRedactedFp,
          });
        }
      }

      for (const c of entry.responseCookies ?? []) {
        const { keys, isRedactedFp } = buildMatchKeys(c.value);
        if (isRedactedFp || isStrongValue(c.value, c.name)) {
          producers.push({
            sessionId,
            requestId: entry.id,
            orderIndex: idx,
            location: 'set_cookie',
            jsonPathOrKey: c.name,
            rawValue: c.value,
            matchKeys: keys,
            isRedactedFp,
          });
        }
      }

      if (entry.responseBody) {
        try {
          const parsed: unknown = JSON.parse(entry.responseBody);
          const leaves: Array<{ path: string; key: string; value: string }> = [];
          collectJsonLeaves(parsed, '$', leaves);
          for (const leaf of leaves) {
            const { keys, isRedactedFp } = buildMatchKeys(leaf.value);
            if (isRedactedFp || isStrongValue(leaf.value, leaf.key)) {
              producers.push({
                sessionId,
                requestId: entry.id,
                orderIndex: idx,
                location: 'response_body',
                jsonPathOrKey: leaf.path,
                rawValue: leaf.value,
                matchKeys: keys,
                isRedactedFp,
              });
            }
          }
        } catch {
          // Ignore non-JSON body
        }
      }

      for (const prod of producers) {
        for (const mk of prod.matchKeys) {
          const list = producerIndex.get(mk);
          if (list) {
            list.push(prod);
          } else {
            producerIndex.set(mk, [prod]);
          }
        }
      }
    }
  }

  let depIdx = 0;
  const results: Dependency[] = [];
  for (const item of candidateMap.values()) {
    depIdx += 1;
    const id = `dep-${String(depIdx).padStart(4, '0')}`;
    const v = 1.0;
    const t = 1.0;
    const u = item.isRedactedFp ? 0.95 : 0.78;
    const r = item.sessions.size > 1 ? 1.0 : 0.7;
    const s = 0.85;
    const rawConf = 0.3 * v + 0.2 * t + 0.2 * u + 0.2 * r + 0.1 * s;
    const confidence = Math.round(rawConf * 100) / 100;

    let status: ConfirmationStatus =
      confidence >= 0.9 ? 'confirmed' : confidence >= 0.65 ? 'likely' : 'unknown';

    const edit = dependencyEdits[id];
    const override = edit?.status ?? userOverrides[id];
    if (override) {
      status = override;
    }

    results.push({
      id,
      source: {
        ...item.source,
        jsonPathOrKey: edit?.sourcePath ?? item.source.jsonPathOrKey,
      },
      target: {
        ...item.target,
        jsonPathOrKey: edit?.targetPath ?? item.target.jsonPathOrKey,
      },
      type: item.type,
      transformation:
        edit?.transformation !== undefined ? edit.transformation : item.transformation,
      confidence,
      status,
      evidence: item.evidence,
      observedSessionIds: Array.from(item.sessions).sort(),
    });
  }

  return results;
}

function diffSchemaDefinitions(
  baseline?: SchemaDefinition | null,
  compared?: SchemaDefinition | null,
): SchemaDiff | undefined {
  if (!baseline && !compared) {
    return undefined;
  }
  const baseFields = new Map((baseline?.fields ?? []).map((f) => [f.name, f]));
  const compFields = new Map((compared?.fields ?? []).map((f) => [f.name, f]));
  const addedFields: string[] = [];
  const removedFields: string[] = [];
  const changedFields: string[] = [];

  for (const [name, comp] of compFields.entries()) {
    const base = baseFields.get(name);
    if (!base) {
      addedFields.push(name);
    } else if (
      base.inferredType !== comp.inferredType ||
      base.nullable !== comp.nullable ||
      base.required !== comp.required
    ) {
      changedFields.push(name);
    }
  }
  for (const name of baseFields.keys()) {
    if (!compFields.has(name)) {
      removedFields.push(name);
    }
  }

  if (
    addedFields.length === 0 &&
    removedFields.length === 0 &&
    changedFields.length === 0
  ) {
    return undefined;
  }

  return {
    addedFields: addedFields.sort(),
    removedFields: removedFields.sort(),
    changedFields: changedFields.sort(),
  };
}

export function analyzeSessions(
  sources: SessionSource[],
  keptEntries: TrafficEntrySummary[],
  endpoints: Endpoint[],
  dependencies: Dependency[],
): {
  authFlows: AuthFlow[];
  lifecycle: SessionLifecycleReport;
  comparison: SessionComparisonReport;
} {
  const bearerIds: string[] = [];
  const cookieIds: string[] = [];
  const csrfIds: string[] = [];
  const apiKeyIds: string[] = [];
  const oauthIds: string[] = [];
  const signatureIds: string[] = [];

  const loginRequestIds: string[] = [];
  const refreshRequestIds: string[] = [];
  const tokenRotations: TokenRotation[] = [];
  const expirationSignals: ExpirationSignal[] = [];

  const bySession = new Map<string, TrafficEntrySummary[]>();
  for (const entry of keptEntries) {
    const list = bySession.get(entry.sessionId);
    if (list) {
      list.push(entry);
    } else {
      bySession.set(entry.sessionId, [entry]);
    }

    const pathLower = (entry.rawPath ?? entry.endpoint).toLowerCase();
    const hasBearer = entry.sanitizedHeaders.some(
      (h) =>
        h.name.toLowerCase() === 'authorization' &&
        (h.value.includes('authorization') || h.value.includes('bearer')),
    );
    const hasCookie =
      (entry.requestCookies?.length ?? 0) > 0 ||
      (entry.responseCookies?.length ?? 0) > 0 ||
      entry.sanitizedHeaders.some((h) => h.name.toLowerCase() === 'cookie') ||
      (entry.responseHeaders ?? []).some((h) => h.name.toLowerCase() === 'set-cookie');
    const hasCsrf = entry.sanitizedHeaders.some((h) =>
      h.name.toLowerCase().includes('csrf') || h.name.toLowerCase().includes('xsrf'),
    );
    const hasApiKey =
      entry.sanitizedHeaders.some((h) => h.name.toLowerCase() === 'x-api-key') ||
      entry.queryParams.some((q) => q.name.toLowerCase().includes('api_key'));
    const isOauth =
      pathLower.includes('/oauth') ||
      pathLower.endsWith('/token') ||
      (entry.requestBody?.includes('grant_type') ?? false) ||
      (entry.responseBody?.includes('access_token') ?? false);
    const hasCustomSig = entry.sanitizedHeaders.some((h) =>
      h.name.toLowerCase().includes('signature'),
    );

    if (hasBearer) bearerIds.push(entry.id);
    if (hasCookie) cookieIds.push(entry.id);
    if (hasCsrf) csrfIds.push(entry.id);
    if (hasApiKey) apiKeyIds.push(entry.id);
    if (isOauth) oauthIds.push(entry.id);
    if (hasCustomSig) signatureIds.push(entry.id);

    if (pathLower.includes('refresh')) {
      refreshRequestIds.push(entry.id);
    } else if (
      pathLower.includes('login') ||
      pathLower.includes('signin') ||
      pathLower.endsWith('/token')
    ) {
      loginRequestIds.push(entry.id);
    }

    if (entry.status === 401 || entry.status === 403) {
      expirationSignals.push({
        sessionId: entry.sessionId,
        requestId: entry.id,
        kind: `http_${entry.status}`,
        detail: `HTTP ${entry.status} authorization/session challenge`,
      });
    }
  }

  for (const [sessionId, sessEntries] of bySession.entries()) {
    const lastFpByKind = new Map<string, { reqId: string; fp: string }>();
    for (const entry of sessEntries) {
      for (const h of entry.sanitizedHeaders) {
        const lower = h.name.toLowerCase();
        if (lower === 'authorization' || lower === 'cookie' || lower === 'x-csrf-token') {
          const parsed = parseRedactedMarker(h.value);
          if (parsed) {
            const prev = lastFpByKind.get(lower);
            if (prev && prev.fp !== parsed.fingerprint) {
              tokenRotations.push({
                sessionId,
                kind: lower,
                fromRequestId: prev.reqId,
                fromFingerprint: prev.fp,
                toRequestId: entry.id,
                toFingerprint: parsed.fingerprint,
              });
            }
            lastFpByKind.set(lower, { reqId: entry.id, fp: parsed.fingerprint });
          }
        }
      }
    }
  }

  const authFlows: AuthFlow[] = [];
  if (bearerIds.length > 0) {
    authFlows.push({
      id: 'auth-bearer',
      scheme: 'bearer',
      description: 'Observed Authorization Bearer header (redacted)',
      requestIds: bearerIds,
      confidence: 0.9,
    });
  }
  if (cookieIds.length > 0) {
    authFlows.push({
      id: 'auth-cookie',
      scheme: 'cookie_session',
      description: 'Observed Cookie / Set-Cookie session headers',
      requestIds: cookieIds,
      confidence: 0.85,
    });
  }
  if (csrfIds.length > 0) {
    authFlows.push({
      id: 'auth-csrf',
      scheme: 'csrf',
      description: 'Observed CSRF header token usage',
      requestIds: csrfIds,
      confidence: 0.85,
    });
  }
  if (apiKeyIds.length > 0) {
    authFlows.push({
      id: 'auth-api-key',
      scheme: 'api_key',
      description: 'Observed API key header or query parameter (redacted)',
      requestIds: apiKeyIds,
      confidence: 0.88,
    });
  }
  if (oauthIds.length > 0) {
    authFlows.push({
      id: 'auth-oauth-like',
      scheme: 'oauth_like',
      description: 'Observed OAuth2 / token exchange lifecycle',
      requestIds: oauthIds,
      confidence: 0.9,
    });
  }
  if (signatureIds.length > 0) {
    authFlows.push({
      id: 'auth-unknown-signature',
      scheme: 'unknown',
      description: 'Observed custom signature header requiring extension hook',
      requestIds: signatureIds,
      confidence: 0.5,
    });
  }

  const autoRefreshEligible =
    refreshRequestIds.length > 0 &&
    (tokenRotations.length > 0 ||
      dependencies.some(
        (d) =>
          d.status !== 'rejected' &&
          (d.source.jsonPathOrKey.includes('refresh_token') ||
            d.source.jsonPathOrKey.includes('access_token')),
      ));

  const lifecycle: SessionLifecycleReport = {
    authFlows,
    loginRequestIds,
    refreshRequestIds,
    tokenRotations,
    expirationSignals,
    autoRefreshEligible,
  };

  // Multi-HAR Session Comparison (M09)
  const sessionIds =
    sources.length > 0
      ? sources.map((s) => s.id)
      : Array.from(bySession.keys()).sort();

  const endpointSessions = new Map<string, Set<string>>();
  for (const ep of endpoints) {
    const key = `${ep.method} ${ep.host}${ep.pathTemplate}`;
    const set = endpointSessions.get(key) ?? new Set<string>();
    for (const reqId of ep.observedRequestIds) {
      const entry = keptEntries.find((e) => e.id === reqId);
      if (entry) {
        set.add(entry.sessionId);
      }
    }
    endpointSessions.set(key, set);
  }

  const sharedEndpoints: string[] = [];
  const sessionOnlyEndpoints: SessionEndpointPresence[] = [];
  for (const [epKey, sessSet] of endpointSessions.entries()) {
    if (sessionIds.length > 1 && sessSet.size > 1) {
      sharedEndpoints.push(epKey);
    } else {
      for (const sid of sessSet) {
        sessionOnlyEndpoints.push({ sessionId: sid, endpointKey: epKey });
      }
    }
  }

  // Parameter variability
  const paramObservations = new Map<
    string,
    {
      endpointKey: string;
      location: ParameterLocation;
      name: string;
      bySession: Map<string, Set<string>>;
      allValues: Set<string>;
    }
  >();

  for (const ep of endpoints) {
    const epKey = `${ep.method} ${ep.host}${ep.pathTemplate}`;
    const epEntries = keptEntries.filter((e) => ep.observedRequestIds.includes(e.id));
    for (const entry of epEntries) {
      for (const q of entry.queryParams) {
        const pKey = `${epKey}::query::${q.name}`;
        let rec = paramObservations.get(pKey);
        if (!rec) {
          rec = {
            endpointKey: epKey,
            location: 'query',
            name: q.name,
            bySession: new Map(),
            allValues: new Set(),
          };
          paramObservations.set(pKey, rec);
        }
        rec.allValues.add(q.value);
        const sSet = rec.bySession.get(entry.sessionId) ?? new Set<string>();
        sSet.add(q.value);
        rec.bySession.set(entry.sessionId, sSet);
      }

      const rawSegs = (entry.rawPath ?? '').split('/').filter(Boolean);
      const tplSegs = ep.pathTemplate.split('/').filter(Boolean);
      for (let i = 0; i < Math.min(rawSegs.length, tplSegs.length); i += 1) {
        const tpl = tplSegs[i]!;
        if (tpl.startsWith('{') && tpl.endsWith('}')) {
          const pName = tpl.slice(1, -1);
          const pKey = `${epKey}::path::${pName}`;
          let rec = paramObservations.get(pKey);
          if (!rec) {
            rec = {
              endpointKey: epKey,
              location: 'path',
              name: pName,
              bySession: new Map(),
              allValues: new Set(),
            };
            paramObservations.set(pKey, rec);
          }
          rec.allValues.add(rawSegs[i]!);
          const sSet = rec.bySession.get(entry.sessionId) ?? new Set<string>();
          sSet.add(rawSegs[i]!);
          rec.bySession.set(entry.sessionId, sSet);
        }
      }
    }
  }

  const parameterVariability: ParameterVariability[] = [];
  for (const rec of paramObservations.values()) {
    let variability: VariabilityKind = 'constant';
    if (rec.allValues.size > 1) {
      const variesWithinSession = Array.from(rec.bySession.values()).some(
        (vals) => vals.size > 1,
      );
      variability = variesWithinSession ? 'request_volatile' : 'session_scoped';
    }
    parameterVariability.push({
      endpointKey: rec.endpointKey,
      location: rec.location,
      name: rec.name,
      variability,
      distinctValuesCount: rec.allValues.size,
      sampleValues: Array.from(rec.allValues).slice(0, 4),
    });
  }

  const corroboratedDependencyIds = dependencies
    .filter((d) => d.observedSessionIds.length > 1)
    .map((d) => d.id);
  const singleSessionDependencyIds = dependencies
    .filter((d) => d.observedSessionIds.length <= 1)
    .map((d) => d.id);

  // Auth differences across sessions
  const authDifferences: AuthSessionDiff[] = [];
  if (sessionIds.length > 1) {
    const schemesByReq = new Map<string, Set<AuthScheme>>();
    for (const flow of authFlows) {
      for (const reqId of flow.requestIds) {
        const set = schemesByReq.get(reqId) ?? new Set<AuthScheme>();
        set.add(flow.scheme);
        schemesByReq.set(reqId, set);
      }
    }
    const schemesPresent = new Map<AuthScheme, Set<string>>();
    for (const entry of keptEntries) {
      const schemes = schemesByReq.get(entry.id);
      if (!schemes) continue;
      for (const scheme of schemes) {
        const set = schemesPresent.get(scheme) ?? new Set<string>();
        set.add(entry.sessionId);
        schemesPresent.set(scheme, set);
      }
    }
    for (const [scheme, presentSet] of schemesPresent.entries()) {
      const presentInSessions = sessionIds.filter((s) => presentSet.has(s));
      const missingInSessions = sessionIds.filter((s) => !presentSet.has(s));
      if (missingInSessions.length > 0) {
        authDifferences.push({ scheme, presentInSessions, missingInSessions });
      }
    }
  }

  // Schema drifts across sessions for shared endpoints
  const schemaDrifts: SchemaDriftEntry[] = [];
  if (sessionIds.length > 1) {
    const baselineSid = sessionIds[0]!;
    for (const ep of endpoints) {
      const epKey = `${ep.method} ${ep.host}${ep.pathTemplate}`;
      const baseEntries = keptEntries.filter(
        (e) => e.sessionId === baselineSid && ep.observedRequestIds.includes(e.id),
      );
      if (baseEntries.length === 0) continue;
      const baseReq = inferSchemaFromJsonSamples(
        'b-req',
        'b-req',
        baseEntries.map((e) => e.requestBody),
      );
      const baseRes = inferSchemaFromJsonSamples(
        'b-res',
        'b-res',
        baseEntries.map((e) => e.responseBody),
      );

      for (const compSid of sessionIds.slice(1)) {
        const compEntries = keptEntries.filter(
          (e) => e.sessionId === compSid && ep.observedRequestIds.includes(e.id),
        );
        if (compEntries.length === 0) continue;
        const compReq = inferSchemaFromJsonSamples(
          'c-req',
          'c-req',
          compEntries.map((e) => e.requestBody),
        );
        const compRes = inferSchemaFromJsonSamples(
          'c-res',
          'c-res',
          compEntries.map((e) => e.responseBody),
        );
        const requestDiff = diffSchemaDefinitions(baseReq, compReq);
        const responseDiff = diffSchemaDefinitions(baseRes, compRes);
        if (requestDiff || responseDiff) {
          schemaDrifts.push({
            endpointKey: epKey,
            baselineSessionId: baselineSid,
            comparedSessionId: compSid,
            ...(requestDiff ? { requestDiff } : {}),
            ...(responseDiff ? { responseDiff } : {}),
          });
        }
      }
    }
  }

  return {
    authFlows,
    lifecycle,
    comparison: {
      sessionIds,
      sharedEndpoints: sharedEndpoints.sort(),
      sessionOnlyEndpoints,
      parameterVariability,
      corroboratedDependencyIds,
      singleSessionDependencyIds,
      authDifferences,
      schemaDrifts,
    },
  };
}
