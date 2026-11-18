import { useMemo, useState, type FormEvent } from 'react';
import type { Endpoint, SchemaDefinition } from '@harflow/shared-types';
import { Button } from '@harflow/ui';
import { SchemaTreeView } from '../components/SchemaTreeView.js';
import { useI18n } from '../i18n/index.js';

interface TemplateOverrideFormProps {
  endpoint: Endpoint;
  onSave: (endpointId: string, nextTemplate: string) => Promise<void>;
}

function TemplateOverrideForm({ endpoint, onSave }: TemplateOverrideFormProps) {
  const { t } = useI18n();
  const [template, setTemplate] = useState(endpoint.pathTemplate);

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    void onSave(endpoint.id, template);
  };

  return (
    <form
      onSubmit={handleSubmit}
      style={{ display: 'flex', flexWrap: 'wrap', gap: '0.5rem', alignItems: 'center' }}
    >
      <label style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', flex: '1 1 260px' }}>
        <span style={{ fontSize: '0.84rem', fontWeight: 600 }}>{t.endpoints.overrideLabel}:</span>
        <input
          type="text"
          className="hf-input"
          value={template}
          onChange={(e) => setTemplate(e.target.value)}
          placeholder={t.endpoints.overridePlaceholder}
          aria-label={t.endpoints.overrideLabel}
          style={{ flex: 1 }}
        />
      </label>
      <Button type="submit" variant="primary">
        {t.endpoints.applyOverride}
      </Button>
      <Button onClick={() => void onSave(endpoint.id, '')}>
        {t.endpoints.resetOverride}
      </Button>
    </form>
  );
}

interface EndpointsSchemasPageProps {
  endpoints: Endpoint[];
  schemas: SchemaDefinition[];
  selectedEndpointId: string | null;
  onSelectEndpoint: (endpointId: string) => void;
  onOverrideEndpoint: (endpointKey: string, template: string) => Promise<void>;
}

export function EndpointsSchemasPage({
  endpoints,
  schemas,
  selectedEndpointId,
  onSelectEndpoint,
  onOverrideEndpoint,
}: EndpointsSchemasPageProps) {
  const { t } = useI18n();
  const [search, setSearch] = useState('');
  const [hostFilter, setHostFilter] = useState('all');

  const hosts = useMemo(
    () => Array.from(new Set(endpoints.map((e) => e.host))).sort(),
    [endpoints],
  );

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    return endpoints.filter((ep) => {
      if (hostFilter !== 'all' && ep.host !== hostFilter) {
        return false;
      }
      if (q) {
        const hay = `${ep.method} ${ep.host} ${ep.pathTemplate} ${ep.operationId ?? ''}`.toLowerCase();
        if (!hay.includes(q)) {
          return false;
        }
      }
      return true;
    });
  }, [endpoints, search, hostFilter]);

  const selected =
    filtered.find((e) => e.id === selectedEndpointId) ??
    endpoints.find((e) => e.id === selectedEndpointId) ??
    filtered[0] ??
    null;

  const schemaMap = useMemo(
    () => new Map(schemas.map((s) => [s.id, s])),
    [schemas],
  );

  const requestSchema = selected?.requestSchemaId
    ? (schemaMap.get(selected.requestSchemaId) ?? null)
    : null;
  const responseSchema = selected?.responseSchemaId
    ? (schemaMap.get(selected.responseSchemaId) ?? null)
    : null;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
      <h1 style={{ margin: 0, fontSize: '1.35rem' }}>{t.endpoints.title}</h1>

      {endpoints.length === 0 ? (
        <div className="hf-panel">{t.endpoints.empty}</div>
      ) : (
        <div className="hf-split-grid">
          <aside className="hf-panel" style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
            <input
              type="search"
              className="hf-input"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder={t.endpoints.searchPlaceholder}
              aria-label={t.endpoints.searchPlaceholder}
            />
            <select
              className="hf-input"
              value={hostFilter}
              onChange={(e) => setHostFilter(e.target.value)}
              aria-label={t.endpoints.allHosts}
            >
              <option value="all">{t.endpoints.allHosts}</option>
              {hosts.map((h) => (
                <option key={h} value={h}>
                  {h}
                </option>
              ))}
            </select>

            <div style={{ display: 'flex', flexDirection: 'column', gap: '0.45rem' }}>
              {filtered.map((ep) => {
                const active = selected?.id === ep.id;
                return (
                  <button
                    key={ep.id}
                    type="button"
                    data-testid={`endpoint-${ep.id}`}
                    aria-pressed={active}
                    onClick={() => onSelectEndpoint(ep.id)}
                    style={{
                      textAlign: 'left',
                      padding: '0.6rem 0.7rem',
                      borderRadius: '6px',
                      border: `1px solid ${active ? 'var(--hf-accent)' : 'var(--hf-border)'}`,
                      backgroundColor: active
                        ? 'var(--hf-surface-alt)'
                        : 'var(--hf-bg)',
                      color: 'var(--hf-text)',
                      cursor: 'pointer',
                      display: 'flex',
                      flexDirection: 'column',
                      gap: '0.25rem',
                    }}
                  >
                    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '0.4rem' }}>
                      <code className="hf-mono" style={{ fontWeight: 700 }}>
                        {ep.method} {ep.pathTemplate}
                      </code>
                      <span className="hf-pill">{ep.observedRequestIds.length}x</span>
                    </div>
                    <div style={{ fontSize: '0.76rem', color: 'var(--hf-text-muted)' }}>
                      {ep.host} · {ep.statusCodes.join(', ')}
                      {ep.pagination ? ` · pagination:${ep.pagination.style}` : ''}
                    </div>
                  </button>
                );
              })}
            </div>
          </aside>

          {selected ? (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem', minWidth: 0 }}>
              <section className="hf-panel" style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
                <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: '0.6rem' }}>
                  <code className="hf-mono" style={{ fontSize: '1.05rem', fontWeight: 700 }}>
                    {selected.method} {selected.host}
                    {selected.pathTemplate}
                  </code>
                  <span className="hf-pill">
                    {(selected.confidence * 100).toFixed(0)}%
                  </span>
                  {selected.pagination ? (
                    <span className="hf-pill">
                      {t.endpoints.paginationTitle}: {selected.pagination.style} (
                      {selected.pagination.queryParams.join(', ')})
                    </span>
                  ) : null}
                </div>

                {selected.statusDistribution ? (
                  <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.4rem', alignItems: 'center' }}>
                    <span style={{ fontSize: '0.82rem', fontWeight: 600 }}>
                      {t.endpoints.statusDistribution}:
                    </span>
                    {Object.entries(selected.statusDistribution).map(([code, count]) => (
                      <span key={code} className="hf-pill">
                        HTTP {code}: {count}
                      </span>
                    ))}
                  </div>
                ) : null}

                <TemplateOverrideForm
                  key={`${selected.id}:${selected.pathTemplate}`}
                  endpoint={selected}
                  onSave={onOverrideEndpoint}
                />
              </section>

              <section className="hf-panel">
                <h2 style={{ marginTop: 0, fontSize: '0.98rem' }}>
                  {t.endpoints.parametersTitle}
                </h2>
                {selected.parameters.length === 0 ? (
                  <div style={{ color: 'var(--hf-text-muted)', fontSize: '0.84rem' }}>
                    {t.endpoints.noParams}
                  </div>
                ) : (
                  <table className="hf-table">
                    <thead>
                      <tr>
                        <th>Name</th>
                        <th>Location</th>
                        <th>Type</th>
                        <th>Required</th>
                        <th>Count</th>
                      </tr>
                    </thead>
                    <tbody>
                      {selected.parameters.map((param) => (
                        <tr key={`${param.location}:${param.name}`}>
                          <td className="hf-mono">{param.name}</td>
                          <td>{param.location}</td>
                          <td className="hf-mono">{param.inferredType}</td>
                          <td>
                            {param.required
                              ? t.endpoints.requiredBadge
                              : t.endpoints.optionalBadge}
                          </td>
                          <td>{param.observedCount}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
              </section>

              <SchemaTreeView
                title={t.endpoints.requestSchema}
                schema={requestSchema}
              />
              <SchemaTreeView
                title={t.endpoints.responseSchema}
                schema={responseSchema}
              />
            </div>
          ) : null}
        </div>
      )}
    </div>
  );
}
