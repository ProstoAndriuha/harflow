import type { Dependency } from '@harflow/shared-types';
import { Button, StatusBadge } from '@harflow/ui';
import { useI18n } from '../i18n/index.js';

interface DependencyGraphPageProps {
  dependencies: Dependency[];
  onConfirm: (dependencyId: string) => Promise<void>;
  onReject: (dependencyId: string) => Promise<void>;
}

export function DependencyGraphPage({
  dependencies,
  onConfirm,
  onReject,
}: DependencyGraphPageProps) {
  const { t } = useI18n();

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
      <h1 style={{ margin: 0, fontSize: '1.35rem' }}>{t.dependencies.title}</h1>

      {dependencies.length === 0 ? (
        <div className="hf-panel">{t.dependencies.empty}</div>
      ) : (
        <div style={{ display: 'flex', flexDirection: 'column', gap: '0.85rem' }}>
          {dependencies.map((dep) => (
            <article key={dep.id} className="hf-panel" data-testid={`dependency-${dep.id}`}>
              <div
                style={{
                  display: 'flex',
                  flexWrap: 'wrap',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  gap: '0.75rem',
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: '0.65rem', flexWrap: 'wrap' }}>
                  <StatusBadge status={dep.status} />
                  <code className="hf-mono">
                    {dep.source.requestId} ({dep.source.jsonPathOrKey}) → {dep.target.requestId} (
                    {dep.target.jsonPathOrKey})
                  </code>
                  <span className="hf-pill">
                    {t.dependencies.confidence}: {(dep.confidence * 100).toFixed(0)}%
                  </span>
                </div>

                <div style={{ display: 'flex', gap: '0.5rem' }}>
                  <Button
                    variant="primary"
                    onClick={() => void onConfirm(dep.id)}
                    aria-label={`${t.dependencies.confirm} ${dep.id}`}
                  >
                    {t.dependencies.confirm}
                  </Button>
                  <Button
                    variant="danger"
                    onClick={() => void onReject(dep.id)}
                    aria-label={`${t.dependencies.reject} ${dep.id}`}
                  >
                    {t.dependencies.reject}
                  </Button>
                </div>
              </div>

              <div style={{ marginTop: '0.75rem', fontSize: '0.85rem' }}>
                <strong>{t.dependencies.evidence}:</strong>
                <ul style={{ margin: '0.35rem 0 0', paddingLeft: '1.25rem' }}>
                  {dep.evidence.map((ev, idx) => (
                    <li key={`${ev.ruleId}-${idx}`}>
                      <code className="hf-mono">{ev.ruleId}</code> — {ev.description} (
                      {ev.requestId})
                    </li>
                  ))}
                </ul>
              </div>
            </article>
          ))}
        </div>
      )}
    </div>
  );
}
