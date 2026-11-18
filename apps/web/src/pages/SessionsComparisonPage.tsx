import type {
  SessionComparisonReport,
  SessionLifecycleReport,
  SessionSource,
} from '@harflow/shared-types';
import { useI18n } from '../i18n/index.js';

interface SessionsComparisonPageProps {
  sources: SessionSource[];
  lifecycle: SessionLifecycleReport | null;
  comparison: SessionComparisonReport | null;
}

export function SessionsComparisonPage({
  sources,
  lifecycle,
  comparison,
}: SessionsComparisonPageProps) {
  const { t } = useI18n();

  if (sources.length === 0 && !lifecycle && !comparison) {
    return (
      <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
        <h1 style={{ margin: 0, fontSize: '1.35rem' }}>{t.sessions.title}</h1>
        <div className="hf-panel">{t.sessions.empty}</div>
      </div>
    );
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '1.15rem' }}>
      <h1 style={{ margin: 0, fontSize: '1.35rem' }}>{t.sessions.title}</h1>

      <section className="hf-panel" data-testid="session-lifecycle-panel" style={{ display: 'flex', flexDirection: 'column', gap: '0.8rem' }}>
        <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', justifyContent: 'space-between', gap: '0.6rem' }}>
          <h2 style={{ margin: 0, fontSize: '1.05rem' }}>{t.sessions.lifecycleTitle}</h2>
          <span
            className="hf-pill"
            data-testid="auto-refresh-badge"
            style={{
              borderColor: lifecycle?.autoRefreshEligible
                ? 'var(--hf-status-confirmed)'
                : 'var(--hf-status-likely)',
            }}
          >
            {lifecycle?.autoRefreshEligible
              ? `✓ ${t.sessions.autoRefreshEligible}`
              : `~ ${t.sessions.manualAuthRequired}`}
          </span>
        </div>

        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))', gap: '0.75rem' }}>
          {(lifecycle?.authFlows ?? []).map((flow) => (
            <div
              key={flow.id}
              style={{
                padding: '0.7rem',
                border: '1px solid var(--hf-border)',
                borderRadius: '6px',
                backgroundColor: 'var(--hf-bg)',
              }}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '0.3rem' }}>
                <strong className="hf-mono">{flow.scheme}</strong>
                <span className="hf-pill">{(flow.confidence * 100).toFixed(0)}%</span>
              </div>
              <div style={{ fontSize: '0.82rem', color: 'var(--hf-text-muted)' }}>
                {flow.description}
              </div>
              <div style={{ fontSize: '0.78rem', marginTop: '0.35rem' }}>
                Requests: <code className="hf-mono">{flow.requestIds.join(', ')}</code>
              </div>
            </div>
          ))}
        </div>

        <div style={{ display: 'flex', flexWrap: 'wrap', gap: '1rem', fontSize: '0.84rem' }}>
          <div>
            <strong>{t.sessions.loginRequests}:</strong>{' '}
            <code className="hf-mono">
              {(lifecycle?.loginRequestIds ?? []).join(', ') || '—'}
            </code>
          </div>
          <div>
            <strong>{t.sessions.refreshRequests}:</strong>{' '}
            <code className="hf-mono">
              {(lifecycle?.refreshRequestIds ?? []).join(', ') || '—'}
            </code>
          </div>
        </div>

        <div>
          <strong style={{ fontSize: '0.9rem' }}>{t.sessions.rotationsTitle}</strong>
          {(lifecycle?.tokenRotations.length ?? 0) === 0 ? (
            <div style={{ color: 'var(--hf-text-muted)', fontSize: '0.84rem', marginTop: '0.25rem' }}>
              {t.sessions.noRotations}
            </div>
          ) : (
            <ul style={{ margin: '0.35rem 0 0', paddingLeft: '1.2rem' }}>
              {lifecycle?.tokenRotations.map((rot, idx) => (
                <li key={`${rot.sessionId}-${idx}`} className="hf-mono" style={{ fontSize: '0.82rem' }}>
                  [{rot.sessionId}] {rot.kind}: {rot.fromRequestId} ({rot.fromFingerprint}) →{' '}
                  {rot.toRequestId} ({rot.toFingerprint})
                </li>
              ))}
            </ul>
          )}
        </div>

        <div>
          <strong style={{ fontSize: '0.9rem' }}>{t.sessions.expirationsTitle}</strong>
          {(lifecycle?.expirationSignals.length ?? 0) === 0 ? (
            <div style={{ color: 'var(--hf-text-muted)', fontSize: '0.84rem', marginTop: '0.25rem' }}>
              {t.sessions.noExpirations}
            </div>
          ) : (
            <ul style={{ margin: '0.35rem 0 0', paddingLeft: '1.2rem' }}>
              {lifecycle?.expirationSignals.map((sig, idx) => (
                <li key={`${sig.requestId}-${idx}`} style={{ fontSize: '0.82rem' }}>
                  <code className="hf-mono">
                    [{sig.sessionId}] {sig.requestId}
                  </code>{' '}
                  — {sig.detail}
                </li>
              ))}
            </ul>
          )}
        </div>
      </section>

      <section className="hf-panel" data-testid="session-comparison-panel" style={{ display: 'flex', flexDirection: 'column', gap: '0.85rem' }}>
        <h2 style={{ margin: 0, fontSize: '1.05rem' }}>{t.sessions.comparisonTitle}</h2>

        <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.5rem' }}>
          {sources.map((src) => (
            <span key={src.id} className="hf-pill">
              <code className="hf-mono">{src.id}</code> · {src.name} ({src.entryCount} req,{' '}
              {src.redactedSecretsCount} redacted)
            </span>
          ))}
        </div>

        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))', gap: '0.85rem', fontSize: '0.84rem' }}>
          <div>
            <strong>{t.sessions.sharedEndpoints} ({comparison?.sharedEndpoints.length ?? 0})</strong>
            <ul style={{ margin: '0.3rem 0 0', paddingLeft: '1.2rem' }}>
              {(comparison?.sharedEndpoints ?? []).map((ep) => (
                <li key={ep} className="hf-mono">
                  {ep}
                </li>
              ))}
            </ul>
          </div>
          <div>
            <strong>
              {t.sessions.sessionOnlyEndpoints} ({comparison?.sessionOnlyEndpoints.length ?? 0})
            </strong>
            <ul style={{ margin: '0.3rem 0 0', paddingLeft: '1.2rem' }}>
              {(comparison?.sessionOnlyEndpoints ?? []).map((item, idx) => (
                <li key={`${item.sessionId}-${idx}`} className="hf-mono">
                  [{item.sessionId}] {item.endpointKey}
                </li>
              ))}
            </ul>
          </div>
        </div>

        <div>
          <strong style={{ fontSize: '0.9rem' }}>{t.sessions.variabilityTitle}</strong>
          {(comparison?.parameterVariability.length ?? 0) === 0 ? (
            <div style={{ color: 'var(--hf-text-muted)', fontSize: '0.84rem', marginTop: '0.25rem' }}>
              {t.sessions.noVariability}
            </div>
          ) : (
            <table className="hf-table" style={{ marginTop: '0.4rem' }}>
              <thead>
                <tr>
                  <th>Endpoint</th>
                  <th>Location</th>
                  <th>Parameter</th>
                  <th>Variability</th>
                  <th>Samples</th>
                </tr>
              </thead>
              <tbody>
                {comparison?.parameterVariability.map((pv, idx) => (
                  <tr key={`${pv.endpointKey}-${pv.name}-${idx}`}>
                    <td className="hf-mono">{pv.endpointKey}</td>
                    <td>{pv.location}</td>
                    <td className="hf-mono">{pv.name}</td>
                    <td>
                      <span className="hf-pill">{pv.variability}</span>
                    </td>
                    <td className="hf-mono">{pv.sampleValues.join(', ')}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>

        <div>
          <strong style={{ fontSize: '0.9rem' }}>{t.sessions.schemaDriftTitle}</strong>
          {(comparison?.schemaDrifts.length ?? 0) === 0 ? (
            <div style={{ color: 'var(--hf-text-muted)', fontSize: '0.84rem', marginTop: '0.25rem' }}>
              {t.sessions.noSchemaDrift}
            </div>
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem', marginTop: '0.4rem' }}>
              {comparison?.schemaDrifts.map((drift, idx) => (
                <div
                  key={`${drift.endpointKey}-${idx}`}
                  data-testid="schema-drift-entry"
                  style={{
                    padding: '0.65rem',
                    border: '1px solid var(--hf-border)',
                    borderRadius: '6px',
                    backgroundColor: 'var(--hf-bg)',
                    fontSize: '0.82rem',
                  }}
                >
                  <div>
                    <code className="hf-mono" style={{ fontWeight: 700 }}>
                      {drift.endpointKey}
                    </code>{' '}
                    ({drift.baselineSessionId} vs {drift.comparedSessionId})
                  </div>
                  {drift.requestDiff ? (
                    <div className="hf-mono" style={{ marginTop: '0.25rem' }}>
                      request: +{drift.requestDiff.addedFields.join(',') || '∅'} / -
                      {drift.requestDiff.removedFields.join(',') || '∅'} / ~
                      {drift.requestDiff.changedFields.join(',') || '∅'}
                    </div>
                  ) : null}
                  {drift.responseDiff ? (
                    <div className="hf-mono" style={{ marginTop: '0.2rem' }}>
                      response: +{drift.responseDiff.addedFields.join(',') || '∅'} / -
                      {drift.responseDiff.removedFields.join(',') || '∅'} / ~
                      {drift.responseDiff.changedFields.join(',') || '∅'}
                    </div>
                  ) : null}
                </div>
              ))}
            </div>
          )}
        </div>
      </section>
    </div>
  );
}
