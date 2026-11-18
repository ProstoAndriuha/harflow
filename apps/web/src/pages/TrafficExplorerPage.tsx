import type { TrafficEntrySummary } from '@harflow/shared-types';
import { useI18n } from '../i18n/index.js';

interface TrafficExplorerPageProps {
  entries: TrafficEntrySummary[];
  selectedEntryId: string | null;
  onSelectEntry: (entryId: string) => void;
}

export function TrafficExplorerPage({
  entries,
  selectedEntryId,
  onSelectEntry,
}: TrafficExplorerPageProps) {
  const { t } = useI18n();
  const selected = entries.find((e) => e.id === selectedEntryId) ?? entries[0] ?? null;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
      <h1 style={{ margin: 0, fontSize: '1.35rem' }}>{t.traffic.title}</h1>

      {entries.length === 0 ? (
        <div className="hf-panel">{t.traffic.empty}</div>
      ) : (
        <>
          <div className="hf-table-wrap">
            <table className="hf-table">
              <thead>
                <tr>
                  <th>{t.traffic.columns.method}</th>
                  <th>{t.traffic.columns.status}</th>
                  <th>{t.traffic.columns.endpoint}</th>
                  <th>{t.traffic.columns.domain}</th>
                  <th>{t.traffic.columns.duration}</th>
                  <th>{t.traffic.columns.size}</th>
                  <th>{t.traffic.columns.type}</th>
                  <th>{t.traffic.columns.session}</th>
                </tr>
              </thead>
              <tbody>
                {entries.map((entry) => (
                  <tr
                    key={entry.id}
                    data-selected={selected?.id === entry.id}
                    onClick={() => onSelectEntry(entry.id)}
                  >
                    <td className="hf-mono">{entry.method}</td>
                    <td>{entry.status}</td>
                    <td className="hf-mono">{entry.endpoint}</td>
                    <td>{entry.domain}</td>
                    <td>{entry.durationMs.toFixed(0)} ms</td>
                    <td>{entry.transferSize} B</td>
                    <td>{entry.category}</td>
                    <td>{entry.sessionName}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {selected ? (
            <section className="hf-panel" aria-label={t.traffic.detailsTitle}>
              <h2 style={{ marginTop: 0, fontSize: '1rem' }}>
                {t.traffic.detailsTitle}:{' '}
                <code className="hf-mono">
                  {selected.method} {selected.endpoint}
                </code>
              </h2>
              <p style={{ margin: '0.4rem 0 0.8rem', color: 'var(--hf-text-muted)' }}>
                <strong>{t.traffic.classificationReason}:</strong>{' '}
                {selected.classificationReason}
              </p>
              <div style={{ fontWeight: 600, marginBottom: '0.4rem' }}>
                {t.traffic.sanitizedHeaders}
              </div>
              <ul style={{ margin: 0, paddingLeft: '1.25rem' }}>
                {selected.sanitizedHeaders.map((h, idx) => (
                  <li key={`${h.name}-${idx}`} className="hf-mono">
                    {h.name}: {h.value}
                  </li>
                ))}
              </ul>
            </section>
          ) : null}
        </>
      )}
    </div>
  );
}
