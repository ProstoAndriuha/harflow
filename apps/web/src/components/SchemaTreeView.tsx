import { useState } from 'react';
import type { SchemaDefinition, SchemaField } from '@harflow/shared-types';
import { useI18n } from '../i18n/index.js';

interface SchemaTreeViewProps {
  title: string;
  schema: SchemaDefinition | null;
}

function toPascalCase(raw: string): string {
  return raw
    .split(/[^a-zA-Z0-9]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join('') || 'GeneratedModel';
}

function fieldToTsType(field: SchemaField): string {
  let base = 'unknown';
  switch (field.inferredType) {
    case 'integer':
    case 'number':
      base = 'number';
      break;
    case 'boolean':
      base = 'boolean';
      break;
    case 'string':
    case 'uuid':
    case 'datetime':
      base = 'string';
      break;
    case 'enum':
      base =
        field.enumValues && field.enumValues.length > 0
          ? field.enumValues.map((v) => `'${v}'`).join(' | ')
          : 'string';
      break;
    case 'union':
      base =
        field.unionTypes && field.unionTypes.length > 0
          ? field.unionTypes.map((u) => (u === 'integer' ? 'number' : u)).join(' | ')
          : 'unknown';
      break;
    case 'array':
      base = field.nestedFields
        ? `Array<{ ${field.nestedFields.map((f) => `${f.name}${f.required ? '' : '?'}: ${fieldToTsType(f)}`).join('; ')} }>`
        : `${field.itemType === 'integer' ? 'number' : (field.itemType ?? 'unknown')}[]`;
      break;
    case 'object':
      base = field.nestedFields
        ? `{ ${field.nestedFields.map((f) => `${f.name}${f.required ? '' : '?'}: ${fieldToTsType(f)}`).join('; ')} }`
        : 'Record<string, unknown>';
      break;
    default:
      base = 'unknown';
  }
  return field.nullable ? `${base} | null` : base;
}

function fieldToPyType(field: SchemaField): string {
  let base = 'Any';
  switch (field.inferredType) {
    case 'integer':
      base = 'int';
      break;
    case 'number':
      base = 'float';
      break;
    case 'boolean':
      base = 'bool';
      break;
    case 'string':
      base = 'str';
      break;
    case 'uuid':
      base = 'UUID';
      break;
    case 'datetime':
      base = 'datetime';
      break;
    case 'enum':
      base =
        field.enumValues && field.enumValues.length > 0
          ? `Literal[${field.enumValues.map((v) => `"${v}"`).join(', ')}]`
          : 'str';
      break;
    case 'union':
      base =
        field.unionTypes && field.unionTypes.length > 0
          ? field.unionTypes.map((u) => (u === 'integer' ? 'int' : u === 'string' ? 'str' : 'Any')).join(' | ')
          : 'Any';
      break;
    case 'array':
      base = `list[${field.itemType === 'integer' ? 'int' : field.itemType === 'string' ? 'str' : 'dict[str, Any]'}]`;
      break;
    case 'object':
      base = 'dict[str, Any]';
      break;
    default:
      base = 'Any';
  }
  if (field.nullable || !field.required) {
    return `${base} | None${!field.required ? ' = None' : ''}`;
  }
  return base;
}

function renderTypeScriptPreview(schema: SchemaDefinition): string {
  const typeName = toPascalCase(schema.name);
  if (schema.rootType === 'array') {
    return `export type ${typeName} = Array<{\n${schema.fields.map((f) => `  ${f.name}${f.required ? '' : '?'}: ${fieldToTsType(f)};`).join('\n')}\n}>;`;
  }
  return `export interface ${typeName} {\n${schema.fields.map((f) => `  ${f.name}${f.required ? '' : '?'}: ${fieldToTsType(f)};`).join('\n')}\n}`;
}

function renderPydanticPreview(schema: SchemaDefinition): string {
  const className = toPascalCase(schema.name);
  const lines = schema.fields.map((f) => `    ${f.name}: ${fieldToPyType(f)}`);
  return `class ${className}(BaseModel):\n${lines.length > 0 ? lines.join('\n') : '    pass'}`;
}

function FieldRow({
  field,
  totalObs,
  depth = 0,
}: {
  field: SchemaField;
  totalObs: number;
  depth?: number;
}) {
  const { t } = useI18n();
  return (
    <div
      style={{
        marginLeft: `${depth * 14}px`,
        padding: '0.4rem 0.55rem',
        borderLeft: depth > 0 ? '2px solid var(--hf-border)' : undefined,
        display: 'flex',
        flexDirection: 'column',
        gap: '0.3rem',
      }}
    >
      <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: '0.4rem' }}>
        <code className="hf-mono" style={{ fontWeight: 700 }}>
          {field.name}
        </code>
        <span className="hf-pill">{field.inferredType}</span>
        {field.itemType ? <span className="hf-pill">items: {field.itemType}</span> : null}
        <span
          className="hf-pill"
          style={{
            borderColor: field.required ? 'var(--hf-status-confirmed)' : 'var(--hf-status-likely)',
          }}
        >
          {field.required ? t.endpoints.requiredBadge : t.endpoints.optionalBadge} (
          {field.observedCount}/{totalObs})
        </span>
        {field.nullable ? (
          <span className="hf-pill" style={{ borderColor: 'var(--hf-accent)' }}>
            {t.endpoints.nullableBadge}
          </span>
        ) : null}
        {field.enumValues && field.enumValues.length > 0 ? (
          <span className="hf-mono" style={{ fontSize: '0.78rem', color: 'var(--hf-text-muted)' }}>
            [{field.enumValues.join(', ')}]
          </span>
        ) : null}
        {field.unionTypes && field.unionTypes.length > 0 ? (
          <span className="hf-mono" style={{ fontSize: '0.78rem', color: 'var(--hf-text-muted)' }}>
            ({field.unionTypes.join(' | ')})
          </span>
        ) : null}
      </div>

      {field.nestedFields && field.nestedFields.length > 0 ? (
        <div style={{ display: 'flex', flexDirection: 'column', gap: '0.2rem' }}>
          {field.nestedFields.map((child) => (
            <FieldRow
              key={`${field.name}.${child.name}`}
              field={child}
              totalObs={field.observedCount}
              depth={depth + 1}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}

export function SchemaTreeView({ title, schema }: SchemaTreeViewProps) {
  const { t } = useI18n();
  const [mode, setMode] = useState<'tree' | 'typescript' | 'pydantic'>('tree');

  return (
    <section
      className="hf-panel"
      style={{ display: 'flex', flexDirection: 'column', gap: '0.6rem' }}
    >
      <div
        style={{
          display: 'flex',
          flexWrap: 'wrap',
          alignItems: 'center',
          justifyContent: 'space-between',
          gap: '0.5rem',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
          <strong>{title}</strong>
          {schema ? (
            <span className="hf-pill">
              {schema.rootType} · {schema.observationCount} {t.endpoints.observations}
            </span>
          ) : null}
        </div>

        {schema ? (
          <div style={{ display: 'flex', gap: '0.35rem' }}>
            {(['tree', 'typescript', 'pydantic'] as const).map((tab) => (
              <button
                key={tab}
                type="button"
                className="hf-pill"
                aria-pressed={mode === tab}
                onClick={() => setMode(tab)}
                style={{
                  cursor: 'pointer',
                  borderColor: mode === tab ? 'var(--hf-accent)' : 'var(--hf-border)',
                  color: 'var(--hf-text)',
                }}
              >
                {t.endpoints.previewTabs[tab]}
              </button>
            ))}
          </div>
        ) : null}
      </div>

      {!schema ? (
        <div style={{ color: 'var(--hf-text-muted)', fontSize: '0.84rem' }}>
          {t.endpoints.noSchema}
        </div>
      ) : mode === 'tree' ? (
        <div style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
          {schema.fields.map((field) => (
            <FieldRow
              key={field.name}
              field={field}
              totalObs={schema.observationCount}
            />
          ))}
        </div>
      ) : mode === 'typescript' ? (
        <pre className="hf-pre">{renderTypeScriptPreview(schema)}</pre>
      ) : (
        <pre className="hf-pre">{renderPydanticPreview(schema)}</pre>
      )}
    </section>
  );
}
