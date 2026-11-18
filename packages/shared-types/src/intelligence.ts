import type {
  AuthFlow,
  AuthScheme,
  ConfirmationStatus,
  ParameterLocation,
  Transformation,
} from './api-model.js';

export type TrafficCategory =
  | 'api'
  | 'authentication'
  | 'analytics'
  | 'advertising'
  | 'telemetry'
  | 'static_assets'
  | 'images'
  | 'fonts'
  | 'css'
  | 'javascript'
  | 'media'
  | 'documents'
  | 'other';

export const ALL_TRAFFIC_CATEGORIES: readonly TrafficCategory[] = [
  'api',
  'authentication',
  'analytics',
  'advertising',
  'telemetry',
  'static_assets',
  'images',
  'fonts',
  'css',
  'javascript',
  'media',
  'documents',
  'other',
] as const;

export type FilterAction = 'keep' | 'exclude';

export interface CustomCleanerRule {
  id: string;
  domainSuffix?: string;
  pathPrefix?: string;
  method?: string;
  action: FilterAction;
  overrideCategory?: TrafficCategory;
}

export interface CleanerConfig {
  keepCategories: TrafficCategory[];
  includedDomains: string[];
  excludedDomains: string[];
  keepRequestIds: string[];
  excludeRequestIds: string[];
  customRules: CustomCleanerRule[];
}

export interface EntryFilterDecision {
  entryId: string;
  sessionId: string;
  method: string;
  host: string;
  path: string;
  category: TrafficCategory;
  classificationReason: string;
  kept: boolean;
  filterReason: string;
}

export interface CleanerPreview {
  totalEntries: number;
  keptEntries: number;
  excludedEntries: number;
  categoryCounts: Record<string, number>;
  decisions: EntryFilterDecision[];
}

export interface CleanerUpdatePatch {
  keepCategories?: TrafficCategory[];
  excludeDomain?: string;
  includeDomain?: string;
  excludeRequest?: string;
  restoreRequest?: string;
  addCustomRule?: CustomCleanerRule;
  removeCustomRuleId?: string;
  replaceConfig?: CleanerConfig;
}

export interface TokenRotation {
  sessionId: string;
  kind: string;
  fromRequestId: string;
  fromFingerprint: string;
  toRequestId: string;
  toFingerprint: string;
}

export interface ExpirationSignal {
  sessionId: string;
  requestId: string;
  kind: string;
  detail: string;
}

export interface SessionLifecycleReport {
  authFlows: AuthFlow[];
  loginRequestIds: string[];
  refreshRequestIds: string[];
  tokenRotations: TokenRotation[];
  expirationSignals: ExpirationSignal[];
  autoRefreshEligible: boolean;
}

export type VariabilityKind = 'constant' | 'session_scoped' | 'request_volatile';

export interface ParameterVariability {
  endpointKey: string;
  location: ParameterLocation;
  name: string;
  variability: VariabilityKind;
  distinctValuesCount: number;
  sampleValues: string[];
}

export interface SessionEndpointPresence {
  sessionId: string;
  endpointKey: string;
}

export interface AuthSessionDiff {
  scheme: AuthScheme;
  presentInSessions: string[];
  missingInSessions: string[];
}

export interface SchemaDiff {
  addedFields: string[];
  removedFields: string[];
  changedFields: string[];
}

export interface SchemaDriftEntry {
  endpointKey: string;
  baselineSessionId: string;
  comparedSessionId: string;
  requestDiff?: SchemaDiff;
  responseDiff?: SchemaDiff;
}

export interface SessionComparisonReport {
  sessionIds: string[];
  sharedEndpoints: string[];
  sessionOnlyEndpoints: SessionEndpointPresence[];
  parameterVariability: ParameterVariability[];
  corroboratedDependencyIds: string[];
  singleSessionDependencyIds: string[];
  authDifferences: AuthSessionDiff[];
  schemaDrifts: SchemaDriftEntry[];
}

export interface DependencyMappingOverride {
  status?: ConfirmationStatus;
  sourcePath?: string;
  targetPath?: string;
  transformation?: Transformation | null;
}
