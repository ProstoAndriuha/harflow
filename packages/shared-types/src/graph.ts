import type { ConfirmationStatus, DependencyType, Diagnostic } from './api-model.js';

export type GraphKind =
  | 'request_dependency'
  | 'endpoint'
  | 'auth_flow'
  | 'session_timeline'
  | 'data_flow';

export interface GraphNode {
  id: string;
  label: string;
  nodeType: string;
  method: string | null;
  host: string | null;
}

export interface GraphEdge {
  id: string;
  sourceNodeId: string;
  targetNodeId: string;
  status: ConfirmationStatus;
  confidence: number;
  label: string;
  dependencyType?: DependencyType | null;
  isPaginationLoop?: boolean | null;
}

export interface GraphViewModel {
  schemaVersion: string;
  projectId: string;
  kind: GraphKind;
  nodes: GraphNode[];
  edges: GraphEdge[];
  hasCycles: boolean;
}

export type ExportReadiness = 'not_ready' | 'partial' | 'ready';

export interface AnalysisReport {
  schemaVersion: string;
  projectId: string;
  generatedAt: string;
  totals: {
    requests: number;
    endpoints: number;
    domains: number;
    dependencies: number;
    authFlows: number;
    unknownMappings: number;
  };
  exportReadiness: ExportReadiness;
  diagnostics: Diagnostic[];
}
