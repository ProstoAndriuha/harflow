export type ConfirmationStatus =
  | 'confirmed'
  | 'likely'
  | 'unknown'
  | 'rejected'
  | 'user_confirmed';

export type ParameterLocation = 'path' | 'query' | 'header' | 'cookie';

export type ValueLocation =
  | 'path'
  | 'query'
  | 'header'
  | 'cookie'
  | 'body'
  | 'set_cookie'
  | 'response_header'
  | 'response_body';

export type DependencyType =
  | 'response_to_header'
  | 'response_to_query'
  | 'response_to_body'
  | 'response_to_path'
  | 'set_cookie_to_cookie'
  | 'pagination_chain'
  | 'session_chain'
  | 'derived_value';

export type Transformation =
  | 'identity'
  | 'url_encoding'
  | 'base64'
  | 'prefix_suffix'
  | 'json_extract'
  | 'custom';

export type AuthScheme =
  | 'bearer'
  | 'cookie_session'
  | 'csrf'
  | 'oauth_like'
  | 'api_key'
  | 'unknown';

export type DiagnosticSeverity = 'info' | 'warning' | 'error';

export interface SessionSource {
  id: string;
  name: string;
  harVersion?: string;
  importedAt: string;
  entryCount: number;
  redactedSecretsCount: number;
}

export interface EndpointParameter {
  name: string;
  location: ParameterLocation;
  inferredType: string;
  required: boolean;
  observedCount: number;
}

export type PaginationStyle = 'cursor' | 'page_number' | 'offset_limit';

export interface PaginationInfo {
  style: PaginationStyle;
  queryParams: string[];
  responseNextField?: string | null;
}

export interface Endpoint {
  id: string;
  operationId?: string;
  method: string;
  host: string;
  pathTemplate: string;
  parameters: EndpointParameter[];
  requestSchemaId: string | null;
  responseSchemaId: string | null;
  statusCodes: number[];
  statusDistribution?: Record<string, number>;
  pagination?: PaginationInfo | null;
  confidence: number;
  category: string;
  observedRequestIds: string[];
}

export interface SchemaField {
  name: string;
  inferredType: string;
  required: boolean;
  nullable: boolean;
  observedCount: number;
  unionTypes?: string[];
  enumValues?: string[];
  itemType?: string | null;
  nestedFields?: SchemaField[];
  typeCounts?: Record<string, number>;
}

export interface SchemaDefinition {
  id: string;
  name: string;
  rootType: string;
  itemType?: string | null;
  fields: SchemaField[];
  observationCount: number;
}

export interface ValueReference {
  sessionId: string;
  requestId: string;
  location: ValueLocation;
  jsonPathOrKey: string;
}

export interface Evidence {
  sessionId: string;
  requestId: string;
  ruleId: string;
  description: string;
  observedAt: string;
}

export interface Dependency {
  id: string;
  source: ValueReference;
  target: ValueReference;
  type: DependencyType;
  transformation?: Transformation | null;
  confidence: number;
  status: ConfirmationStatus;
  evidence: Evidence[];
  observedSessionIds: string[];
}

export interface AuthFlow {
  id: string;
  scheme: AuthScheme;
  description: string;
  requestIds: string[];
  confidence: number;
}

export interface Workflow {
  id: string;
  name: string;
  stepEndpointIds: string[];
}

export interface Diagnostic {
  code: string;
  severity: DiagnosticSeverity;
  message: string;
  relatedEntityIds: string[];
}

export interface ModelMetadata {
  projectName: string;
  generatedAt: string;
  generatorVersion: string;
  totalRequests: number;
  excludedRequests?: number;
}

export interface APIModel {
  schemaVersion: string;
  projectId: string;
  sources: SessionSource[];
  endpoints: Endpoint[];
  schemas: SchemaDefinition[];
  dependencies: Dependency[];
  authFlows: AuthFlow[];
  workflows: Workflow[];
  diagnostics: Diagnostic[];
  metadata: ModelMetadata;
}

export function isValidApiModel(value: unknown): value is APIModel {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const obj = value as Record<string, unknown>;
  return (
    typeof obj.schemaVersion === 'string' &&
    /^\d+\.\d+\.\d+$/.test(obj.schemaVersion) &&
    typeof obj.projectId === 'string' &&
    obj.projectId.length > 0 &&
    Array.isArray(obj.sources) &&
    Array.isArray(obj.endpoints) &&
    Array.isArray(obj.schemas) &&
    Array.isArray(obj.dependencies) &&
    Array.isArray(obj.authFlows) &&
    Array.isArray(obj.workflows) &&
    Array.isArray(obj.diagnostics) &&
    typeof obj.metadata === 'object' &&
    obj.metadata !== null
  );
}
