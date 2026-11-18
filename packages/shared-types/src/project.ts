import type { ConfirmationStatus, SessionSource } from './api-model.js';

export interface ProjectSettings {
  retainOriginalHar: boolean;
  maxHarSizeBytes: number;
  offlineOnly: boolean;
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
}
