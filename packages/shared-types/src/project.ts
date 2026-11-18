import type { ConfirmationStatus, SessionSource } from './api-model.js';
import type { CleanerConfig, DependencyMappingOverride } from './intelligence.js';

export interface ProjectSettings {
  retainOriginalHar: boolean;
  maxHarSizeBytes: number;
  offlineOnly: boolean;
  cleaner?: CleanerConfig;
  endpointOverrides?: Record<string, string>;
}

export interface ProjectStatistics {
  totalRequests: number;
  uniqueEndpoints: number;
  apiDomains: number;
  dependencies: number;
  authFlows: number;
  unknownMappings: number;
}

export interface ExportMetadata {
  lastExportedAt: string | null;
  lastFormats: string[];
}

export interface ProjectManifest {
  schemaVersion: string;
  id: string;
  name: string;
  createdAt: string;
  updatedAt: string;
  sources: SessionSource[];
  settings: ProjectSettings;
  statistics: ProjectStatistics;
  generatorVersions: Record<string, string>;
  exportMetadata: ExportMetadata;
  userOverrides?: Record<string, ConfirmationStatus>;
  dependencyEdits?: Record<string, DependencyMappingOverride>;
}

export type FieldPresence = 'missing' | 'null' | 'empty' | 'value';

export interface TrafficCookieSummary {
  name: string;
  value: string;
  httpOnly?: boolean;
  secure?: boolean;
  sameSite?: string;
}

export interface TrafficTimingsSummary {
  blockedMs?: number;
  dnsMs?: number;
  connectMs?: number;
  sslMs?: number;
  sendMs?: number;
  waitMs?: number;
  receiveMs?: number;
}

export interface TrafficEntrySummary {
  id: string;
  sessionId: string;
  sessionName: string;
  method: string;
  status: number;
  endpoint: string;
  domain: string;
  durationMs: number;
  transferSize: number;
  category: string;
  classificationReason: string;
  sanitizedHeaders: Array<{ name: string; value: string }>;
  queryParams: Array<{ name: string; value: string }>;
  url?: string;
  rawPath?: string;
  startedDateTime?: string;
  mimeType?: string;
  requestCookies?: TrafficCookieSummary[];
  responseHeaders?: Array<{ name: string; value: string }>;
  responseCookies?: TrafficCookieSummary[];
  requestBody?: string | null;
  requestBodyPresence?: FieldPresence;
  responseBody?: string | null;
  contentEncoding?: string | null;
  harEncoding?: string | null;
  timings?: TrafficTimingsSummary;
  redactedCount?: number;
  redactedFields?: string[];
  kept?: boolean;
  filterReason?: string;
}
