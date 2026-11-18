import { useMemo, useState } from 'react';
import {
  ALL_TRAFFIC_CATEGORIES,
  type CleanerConfig,
  type CleanerPreview,
  type CleanerUpdatePatch,
  type TrafficEntrySummary,
} from '@harflow/shared-types';
import { VirtualTable, type VirtualColumn } from '@harflow/ui';
import { RequestInspector } from '../components/RequestInspector.js';
import { SmartCleanerBar } from '../components/SmartCleanerBar.js';
import { useI18n } from '../i18n/index.js';

interface TrafficExplorerPageProps {
  entries: TrafficEntrySummary[];
  selectedEntryId: string | null;
  onSelectEntry: (entryId: string) => void;
  cleanerPreview?: CleanerPreview | null;
  cleanerConfig?: CleanerConfig;
  onUpdateCleaner?: (patch: CleanerUpdatePatch) => Promise<void>;
}

export function TrafficExplorerPage({
  entries,
  selectedEntryId,
  onSelectEntry,
  cleanerPreview = null,
  cleanerConfig,
  onUpdateCleaner,
}: TrafficExplorerPageProps) {
  const { t } = useI18n();
  const [search, setSearch] = useState('');
  const [categoryFilter, setCategoryFilter] = useState('all');
  const [methodFilter, setMethodFilter] = useState('all');
  const [statusFilter, setStatusFilter] = useState('all');
  const [domainFilter, setDomainFilter] = useState('all');
  const [visibilityFilter, setVisibilityFilter] = useState<'all' | 'kept' | 'excluded'>('all');

  const domains = useMemo(
    () => Array.from(new Set(entries.map((e) => e.domain))).sort(),
    [entries],
  );
  const methods = useMemo(
    () => Array.from(new Set(entries.map((e) => e.method))).sort(),
    [entries],
  );

  const filteredEntries = useMemo(() => {
    const q = search.trim().toLowerCase();
    return entries.filter((entry) => {
      if (categoryFilter !== 'all' && entry.category !== categoryFilter) {
        return false;
      }
      if (methodFilter !== 'all' && entry.method !== methodFilter) {
        return false;
      }
      if (domainFilter !== 'all' && entry.domain !== domainFilter) {
        return false;
      }
      if (visibilityFilter === 'kept' && entry.kept === false) {
        return false;
      }
      if (visibilityFilter === 'excluded' && entry.kept !== false) {
        return false;
      }
      if (statusFilter !== 'all') {
        const prefix = statusFilter.slice(0, 1);
        if (!String(entry.status).startsWith(prefix)) {
          return false;
        }
      }
      if (q) {
        const hay =
          `${entry.method} ${entry.endpoint} ${entry.rawPath ?? ''} ${entry.domain} ${entry.classificationReason} ${entry.sessionName}`.toLowerCase();
        if (!hay.includes(q)) {
          return false;
        }
      }
      return true;
    });
  }, [
    entries,
    search,
    categoryFilter,
    methodFilter,
    statusFilter,
    domainFilter,
    visibilityFilter,
  ]);

  const selected =
    filteredEntries.find((e) => e.id === selectedEntryId) ??
    entries.find((e) => e.id === selectedEntryId) ??
    filteredEntries[0] ??
    null;

  const columns: Array<VirtualColumn<TrafficEntrySummary>> = useMemo(
    () => [
      {
        id: 'method',
        header: t.traffic.columns.method,
        width: '80px',
        render: (entry) => <span className="hf-mono">{entry.method}</span>,
      },
      {
        id: 'status',
        header: t.traffic.columns.status,
        width: '72px',
        render: (entry) => <span>{entry.status}</span>,
      },
      {
        id: 'endpoint',
        header: t.traffic.columns.endpoint,
        render: (entry) => (
          <span className="hf-mono">{entry.rawPath ?? entry.endpoint}</span>
        ),
      },
      {
        id: 'domain',
        header: t.traffic.columns.domain,
        render: (entry) => <span>{entry.domain}</span>,
      },
      {
        id: 'duration',
        header: t.traffic.columns.duration,
        width: '88px',
        render: (entry) => <span>{entry.durationMs.toFixed(0)} ms</span>,
      },
      {
        id: 'size',
        header: t.traffic.columns.size,
        width: '84px',
        render: (entry) => <span>{entry.transferSize} B</span>,
      },
      {
        id: 'type',
        header: t.traffic.columns.type,
        width: '120px',
        render: (entry) => <span>{entry.category}</span>,
      },
      {
        id: 'state',
        header: t.traffic.columns.state,
        width: '104px',
        render: (entry) => (
          <span style={{ opacity: entry.kept === false ? 0.6 : 1 }}>
            {entry.kept === false
              ? `✕ ${t.traffic.excludedBadge}`
              : `✓ ${t.traffic.keptBadge}`}
          </span>
        ),
      },
      {
        id: 'session',
        header: t.traffic.columns.session,
        render: (entry) => <span>{entry.sessionName}</span>,
      },
    ],
    [t],
  );

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
      <h1 style={{ margin: 0, fontSize: '1.35rem' }}>{t.traffic.title}</h1>

      {onUpdateCleaner ? (
        <SmartCleanerBar
          preview={cleanerPreview}
          config={cleanerConfig}
          selectedEntry={selected}
          onUpdateCleaner={onUpdateCleaner}
        />
      ) : null}

      {entries.length === 0 ? (
        <div className="hf-panel">{t.traffic.empty}</div>
      ) : (
        <>
          <div
            className="hf-panel"
            style={{
              display: 'flex',
              flexWrap: 'wrap',
              gap: '0.6rem',
              alignItems: 'center',
            }}
          >
            <input
              type="search"
              className="hf-input"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder={t.traffic.searchPlaceholder}
              aria-label={t.traffic.searchPlaceholder}
              style={{ flex: '1 1 220px' }}
            />
            <select
              className="hf-input"
              value={categoryFilter}
              onChange={(e) => setCategoryFilter(e.target.value)}
              aria-label={t.traffic.allCategories}
            >
              <option value="all">{t.traffic.allCategories}</option>
              {ALL_TRAFFIC_CATEGORIES.map((cat) => (
                <option key={cat} value={cat}>
                  {cat}
                </option>
              ))}
            </select>
            <select
              className="hf-input"
              value={methodFilter}
              onChange={(e) => setMethodFilter(e.target.value)}
              aria-label={t.traffic.allMethods}
            >
              <option value="all">{t.traffic.allMethods}</option>
              {methods.map((m) => (
                <option key={m} value={m}>
                  {m}
                </option>
              ))}
            </select>
            <select
              className="hf-input"
              value={statusFilter}
              onChange={(e) => setStatusFilter(e.target.value)}
              aria-label={t.traffic.allStatuses}
            >
              <option value="all">{t.traffic.allStatuses}</option>
              <option value="2xx">2xx</option>
              <option value="3xx">3xx</option>
              <option value="4xx">4xx</option>
              <option value="5xx">5xx</option>
            </select>
            <select
              className="hf-input"
              value={domainFilter}
              onChange={(e) => setDomainFilter(e.target.value)}
              aria-label={t.traffic.allDomains}
            >
              <option value="all">{t.traffic.allDomains}</option>
              {domains.map((d) => (
                <option key={d} value={d}>
                  {d}
                </option>
              ))}
            </select>
            <select
              className="hf-input"
              value={visibilityFilter}
              onChange={(e) =>
                setVisibilityFilter(e.target.value as 'all' | 'kept' | 'excluded')
              }
              aria-label={t.traffic.visibilityAll}
            >
              <option value="all">{t.traffic.visibilityAll}</option>
              <option value="kept">{t.traffic.visibilityKept}</option>
              <option value="excluded">{t.traffic.visibilityExcluded}</option>
            </select>
          </div>

          <VirtualTable
            rows={filteredEntries}
            columns={columns}
            getRowId={(row) => row.id}
            selectedRowId={selected?.id ?? null}
            onSelectRow={onSelectEntry}
            ariaLabel={t.traffic.title}
            emptyMessage={t.traffic.noMatches}
          />

          {selected ? <RequestInspector entry={selected} /> : null}
        </>
      )}
    </div>
  );
}
