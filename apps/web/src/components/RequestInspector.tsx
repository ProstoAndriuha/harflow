import { useState, type KeyboardEvent } from 'react';
import type { TrafficEntrySummary } from '@harflow/shared-types';
import { useI18n } from '../i18n/index.js';

export type InspectorTab =
  | 'overview'
  | 'headers'
  | 'query'
  | 'cookies'
  | 'body'
  | 'response'
  | 'timing'
  | 'security';

const INSPECTOR_TABS: readonly InspectorTab[] = [
  'overview',
  'headers',
  'query',
  'cookies',
  'body',
  'response',
  'timing',
  'security',
] as const;

interface RequestInspectorProps {
  entry: TrafficEntrySummary;
}

export function RequestInspector({ entry }: RequestInspectorProps) {
  const { t } = useI18n();
  const [activeTab, setActiveTab] = useState<InspectorTab>('overview');

  const handleTabKeyDown = (e: KeyboardEvent<HTMLButtonElement>, index: number) => {
    if (e.key === 'ArrowRight') {
      e.preventDefault();
      const next = INSPECTOR_TABS[(index + 1) % INSPECTOR_TABS.length]!;
      setActiveTab(next);
    } else if (e.key === 'ArrowLeft') {
      e.preventDefault();
      const prev =
        INSPECTOR_TABS[(index - 1 + INSPECTOR_TABS.length) % INSPECTOR_TABS.length]!;
      setActiveTab(prev);
    }
  };

  const timingSegments = entry.timings
    ? [
        { key: 'Blocked', ms: entry.timings.blockedMs ?? 0, color: '#94a3b8' },
        { key: 'DNS', ms: entry.timings.dnsMs ?? 0, color: '#38bdf8' },
        { key: 'Connect', ms: entry.timings.connectMs ?? 0, color: '#f59e0b' },
        { key: 'SSL/TLS', ms: entry.timings.sslMs ?? 0, color: '#a855f7' },
        { key: 'Send', ms: entry.timings.sendMs ?? 0, color: '#22c55e' },
        { key: 'Wait (TTFB)', ms: entry.timings.waitMs ?? 0, color: '#3b82f6' },
        { key: 'Receive', ms: entry.timings.receiveMs ?? 0, color: '#ec4899' },
      ].filter((s) => s.ms > 0)
    : [];

  return (
    <section
      className="hf-panel"
      aria-label={t.traffic.detailsTitle}
      data-testid="request-inspector"
      style={{ display: 'flex', flexDirection: 'column', gap: '0.85rem' }}
    >
      <div
        style={{
          display: 'flex',
          flexWrap: 'wrap',
          alignItems: 'center',
          justifyContent: 'space-between',
          gap: '0.6rem',
        }}
      >
        <h2 style={{ margin: 0, fontSize: '1rem' }}>
          {t.traffic.detailsTitle}:{' '}
          <code className="hf-mono">
            {entry.method} {entry.endpoint}
          </code>
        </h2>
        <span className="hf-pill">
          {entry.kept === false
            ? `✕ ${t.traffic.excludedBadge}`
            : `✓ ${t.traffic.keptBadge}`}
        </span>
      </div>

      <div
        role="tablist"
        aria-label={t.traffic.detailsTitle}
        style={{
          display: 'flex',
          flexWrap: 'wrap',
          gap: '0.35rem',
          borderBottom: '1px solid var(--hf-border)',
          paddingBottom: '0.5rem',
        }}
      >
        {INSPECTOR_TABS.map((tab, index) => {
          const selected = activeTab === tab;
          return (
            <button
              key={tab}
              type="button"
              role="tab"
              id={`inspector-tab-${tab}`}
              aria-selected={selected}
              aria-controls={`inspector-panel-${tab}`}
              onClick={() => setActiveTab(tab)}
              onKeyDown={(e) => handleTabKeyDown(e, index)}
              className="hf-pill"
              style={{
                cursor: 'pointer',
                borderColor: selected ? 'var(--hf-accent)' : 'var(--hf-border)',
                backgroundColor: selected
                  ? 'rgba(59, 130, 246, 0.2)'
                  : 'var(--hf-surface-alt)',
                color: 'var(--hf-text)',
              }}
            >
              {t.traffic.tabs[tab]}
            </button>
          );
        })}
      </div>

      <div
        role="tabpanel"
        id={`inspector-panel-${activeTab}`}
        aria-labelledby={`inspector-tab-${activeTab}`}
      >
        {activeTab === 'overview' ? (
          <div style={{ display: 'grid', gap: '0.45rem', fontSize: '0.86rem' }}>
            <div>
              <strong>{t.traffic.inspector.url}:</strong>{' '}
              <code className="hf-mono">{entry.url ?? `https://${entry.domain}${entry.rawPath ?? entry.endpoint}`}</code>
            </div>
            <div>
              <strong>{t.traffic.inspector.status}:</strong> {entry.status} ·{' '}
              <strong>{t.traffic.inspector.mimeType}:</strong> {entry.mimeType || 'unknown'}
            </div>
            <div>
              <strong>{t.traffic.inspector.session}:</strong> {entry.sessionName} ({entry.sessionId})
              {entry.startedDateTime ? ` · ${entry.startedDateTime}` : ''}
            </div>
            <div>
              <strong>{t.traffic.classificationReason}:</strong> {entry.classificationReason}
            </div>
            <div>
              <strong>{t.traffic.filterReason}:</strong>{' '}
              {entry.filterReason ?? t.traffic.keptBadge}
            </div>
          </div>
        ) : null}

        {activeTab === 'headers' ? (
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(260px, 1fr))', gap: '1rem' }}>
            <div>
              <div style={{ fontWeight: 600, marginBottom: '0.4rem' }}>
                {t.traffic.sanitizedHeaders}
              </div>
              {entry.sanitizedHeaders.length === 0 ? (
                <div style={{ color: 'var(--hf-text-muted)' }}>{t.traffic.noItems}</div>
              ) : (
                <ul style={{ margin: 0, paddingLeft: '1.2rem' }}>
                  {entry.sanitizedHeaders.map((h, idx) => (
                    <li key={`${h.name}-${idx}`} className="hf-mono" style={{ fontSize: '0.82rem' }}>
                      {h.name}: {h.value}
                    </li>
                  ))}
                </ul>
              )}
            </div>
            <div>
              <div style={{ fontWeight: 600, marginBottom: '0.4rem' }}>
                {t.traffic.responseHeaders}
              </div>
              {(entry.responseHeaders?.length ?? 0) === 0 ? (
                <div style={{ color: 'var(--hf-text-muted)' }}>{t.traffic.noItems}</div>
              ) : (
                <ul style={{ margin: 0, paddingLeft: '1.2rem' }}>
                  {entry.responseHeaders?.map((h, idx) => (
                    <li key={`${h.name}-${idx}`} className="hf-mono" style={{ fontSize: '0.82rem' }}>
                      {h.name}: {h.value}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </div>
        ) : null}

        {activeTab === 'query' ? (
          <div>
            <div style={{ fontWeight: 600, marginBottom: '0.4rem' }}>
              {t.traffic.queryParams}
            </div>
            {entry.queryParams.length === 0 ? (
              <div style={{ color: 'var(--hf-text-muted)' }}>{t.traffic.noItems}</div>
            ) : (
              <table className="hf-table">
                <thead>
                  <tr>
                    <th>Name</th>
                    <th>Value</th>
                  </tr>
                </thead>
                <tbody>
                  {entry.queryParams.map((q, idx) => (
                    <tr key={`${q.name}-${idx}`}>
                      <td className="hf-mono">{q.name}</td>
                      <td className="hf-mono">{q.value}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        ) : null}

        {activeTab === 'cookies' ? (
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(260px, 1fr))', gap: '1rem' }}>
            <div>
              <div style={{ fontWeight: 600, marginBottom: '0.4rem' }}>
                {t.traffic.requestCookies}
              </div>
              {(entry.requestCookies?.length ?? 0) === 0 ? (
                <div style={{ color: 'var(--hf-text-muted)' }}>{t.traffic.noItems}</div>
              ) : (
                <ul style={{ margin: 0, paddingLeft: '1.2rem' }}>
                  {entry.requestCookies?.map((c, idx) => (
                    <li key={`${c.name}-${idx}`} className="hf-mono" style={{ fontSize: '0.82rem' }}>
                      {c.name}={c.value}
                      {c.httpOnly ? ' · HttpOnly' : ''}
                      {c.secure ? ' · Secure' : ''}
                    </li>
                  ))}
                </ul>
              )}
            </div>
            <div>
              <div style={{ fontWeight: 600, marginBottom: '0.4rem' }}>
                {t.traffic.responseCookies}
              </div>
              {(entry.responseCookies?.length ?? 0) === 0 ? (
                <div style={{ color: 'var(--hf-text-muted)' }}>{t.traffic.noItems}</div>
              ) : (
                <ul style={{ margin: 0, paddingLeft: '1.2rem' }}>
                  {entry.responseCookies?.map((c, idx) => (
                    <li key={`${c.name}-${idx}`} className="hf-mono" style={{ fontSize: '0.82rem' }}>
                      {c.name}={c.value}
                      {c.httpOnly ? ' · HttpOnly' : ''}
                      {c.secure ? ' · Secure' : ''}
                      {c.sameSite ? ` · SameSite=${c.sameSite}` : ''}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </div>
        ) : null}

        {activeTab === 'body' ? (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
            <div style={{ fontSize: '0.84rem' }}>
              <strong>{t.traffic.inspector.bodyPresence}:</strong>{' '}
              <code className="hf-mono">{entry.requestBodyPresence ?? 'missing'}</code>
            </div>
            {entry.requestBody ? (
              <pre className="hf-pre">{entry.requestBody}</pre>
            ) : (
              <div style={{ color: 'var(--hf-text-muted)' }}>{t.traffic.inspector.noBody}</div>
            )}
          </div>
        ) : null}

        {activeTab === 'response' ? (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
            <div style={{ fontSize: '0.84rem', display: 'flex', flexWrap: 'wrap', gap: '0.75rem' }}>
              <span>
                <strong>{t.traffic.inspector.mimeType}:</strong> {entry.mimeType || 'unknown'}
              </span>
              <span>
                <strong>{t.traffic.inspector.harEncoding}:</strong>{' '}
                {entry.harEncoding ?? 'plain'}
              </span>
              <span>
                <strong>{t.traffic.inspector.contentEncoding}:</strong>{' '}
                {entry.contentEncoding ?? 'none'}
              </span>
            </div>
            {entry.responseBody ? (
              <pre className="hf-pre">{entry.responseBody}</pre>
            ) : (
              <div style={{ color: 'var(--hf-text-muted)' }}>{t.traffic.inspector.noBody}</div>
            )}
          </div>
        ) : null}

        {activeTab === 'timing' ? (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '0.6rem' }}>
            <div>
              <strong>{t.traffic.inspector.timingTotal}:</strong> {entry.durationMs.toFixed(1)} ms
            </div>
            {timingSegments.length === 0 ? (
              <div style={{ color: 'var(--hf-text-muted)' }}>{t.traffic.inspector.noTimings}</div>
            ) : (
              <div style={{ display: 'flex', flexDirection: 'column', gap: '0.4rem' }}>
                {timingSegments.map((seg) => {
                  const pct =
                    entry.durationMs > 0
                      ? Math.min(100, Math.max(4, (seg.ms / entry.durationMs) * 100))
                      : 10;
                  return (
                    <div key={seg.key} style={{ display: 'flex', alignItems: 'center', gap: '0.6rem' }}>
                      <span style={{ width: '96px', fontSize: '0.82rem' }}>{seg.key}</span>
                      <div
                        style={{
                          flex: 1,
                          height: '10px',
                          borderRadius: '999px',
                          backgroundColor: 'var(--hf-surface-alt)',
                          overflow: 'hidden',
                        }}
                      >
                        <div
                          style={{
                            width: `${pct}%`,
                            height: '100%',
                            backgroundColor: seg.color,
                          }}
                        />
                      </div>
                      <span className="hf-mono" style={{ width: '64px', fontSize: '0.8rem', textAlign: 'right' }}>
                        {seg.ms} ms
                      </span>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        ) : null}

        {activeTab === 'security' ? (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
            <div>
              <strong>{t.traffic.inspector.redactedCount}:</strong> {entry.redactedCount ?? 0}
            </div>
            {(entry.redactedFields?.length ?? 0) === 0 ? (
              <div style={{ color: 'var(--hf-text-muted)' }}>
                {t.traffic.inspector.noSecretsFound}
              </div>
            ) : (
              <div>
                <div style={{ fontWeight: 600, marginBottom: '0.35rem' }}>
                  {t.traffic.inspector.redactedFields}:
                </div>
                <ul style={{ margin: 0, paddingLeft: '1.2rem' }}>
                  {entry.redactedFields?.map((field) => (
                    <li key={field} className="hf-mono" style={{ fontSize: '0.82rem' }}>
                      {field}
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        ) : null}
      </div>
    </section>
  );
}
