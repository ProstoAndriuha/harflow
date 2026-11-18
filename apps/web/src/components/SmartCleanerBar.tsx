import { useState, type FormEvent } from 'react';
import {
  ALL_TRAFFIC_CATEGORIES,
  type CleanerConfig,
  type CleanerPreview,
  type CleanerUpdatePatch,
  type FilterAction,
  type TrafficCategory,
  type TrafficEntrySummary,
} from '@harflow/shared-types';
import { Button } from '@harflow/ui';
import { useI18n } from '../i18n/index.js';

interface SmartCleanerBarProps {
  preview: CleanerPreview | null;
  config: CleanerConfig | undefined;
  selectedEntry: TrafficEntrySummary | null;
  onUpdateCleaner: (patch: CleanerUpdatePatch) => Promise<void>;
}

export function SmartCleanerBar({
  preview,
  config,
  selectedEntry,
  onUpdateCleaner,
}: SmartCleanerBarProps) {
  const { t } = useI18n();
  const [showPreview, setShowPreview] = useState(false);
  const [showRuleForm, setShowRuleForm] = useState(false);
  const [domainSuffix, setDomainSuffix] = useState('');
  const [pathPrefix, setPathPrefix] = useState('');
  const [method, setMethod] = useState('');
  const [action, setAction] = useState<FilterAction>('exclude');

  const keepCategories = new Set<TrafficCategory>(
    config?.keepCategories ?? ['api', 'authentication'],
  );

  const toggleCategory = (cat: TrafficCategory) => {
    const next = new Set(keepCategories);
    if (next.has(cat)) {
      next.delete(cat);
    } else {
      next.add(cat);
    }
    void onUpdateCleaner({ keepCategories: Array.from(next) });
  };

  const handleAddRule = (e: FormEvent) => {
    e.preventDefault();
    if (!domainSuffix.trim() && !pathPrefix.trim() && !method.trim()) {
      return;
    }
    const id = `rule-${Date.now()}`;
    void onUpdateCleaner({
      addCustomRule: {
        id,
        ...(domainSuffix.trim() ? { domainSuffix: domainSuffix.trim() } : {}),
        ...(pathPrefix.trim() ? { pathPrefix: pathPrefix.trim() } : {}),
        ...(method.trim() ? { method: method.trim().toUpperCase() } : {}),
        action,
      },
    });
    setDomainSuffix('');
    setPathPrefix('');
    setMethod('');
    setShowRuleForm(false);
  };

  const selectedIsKept = selectedEntry?.kept !== false;
  const domainExcluded = selectedEntry
    ? (config?.excludedDomains ?? []).some(
        (d) => d.toLowerCase() === selectedEntry.domain.toLowerCase(),
      )
    : false;

  return (
    <section
      className="hf-panel"
      aria-label={t.traffic.cleaner.title}
      data-testid="smart-cleaner-bar"
      style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}
    >
      <div
        style={{
          display: 'flex',
          flexWrap: 'wrap',
          alignItems: 'center',
          justifyContent: 'space-between',
          gap: '0.75rem',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '0.6rem', flexWrap: 'wrap' }}>
          <strong>{t.traffic.cleaner.title}</strong>
          <span className="hf-pill" data-testid="cleaner-kept-count">
            ✓ {t.traffic.cleaner.keptCount}: {preview?.keptEntries ?? 0}
          </span>
          <span className="hf-pill" data-testid="cleaner-excluded-count">
            ✕ {t.traffic.cleaner.excludedCount}: {preview?.excludedEntries ?? 0}
          </span>
        </div>

        <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.5rem' }}>
          {selectedEntry ? (
            <>
              <Button
                variant={selectedIsKept ? 'danger' : 'primary'}
                onClick={() =>
                  void onUpdateCleaner(
                    selectedIsKept
                      ? { excludeRequest: selectedEntry.id }
                      : { restoreRequest: selectedEntry.id },
                  )
                }
              >
                {selectedIsKept ? t.traffic.excludeRequest : t.traffic.restoreRequest}
              </Button>
              <Button
                onClick={() =>
                  void onUpdateCleaner(
                    domainExcluded
                      ? { includeDomain: selectedEntry.domain }
                      : { excludeDomain: selectedEntry.domain },
                  )
                }
              >
                {domainExcluded
                  ? `${t.traffic.includeDomain} (${selectedEntry.domain})`
                  : `${t.traffic.excludeDomain} (${selectedEntry.domain})`}
              </Button>
            </>
          ) : null}

          <Button onClick={() => setShowPreview((p) => !p)}>
            {showPreview ? t.traffic.cleaner.hidePreview : t.traffic.cleaner.previewDiff}
          </Button>
          <Button onClick={() => setShowRuleForm((p) => !p)}>
            {t.traffic.cleaner.addRule}
          </Button>
        </div>
      </div>

      <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.4rem', alignItems: 'center' }}>
        <span style={{ fontSize: '0.8rem', color: 'var(--hf-text-muted)' }}>
          {t.traffic.cleaner.categoriesLabel}:
        </span>
        {ALL_TRAFFIC_CATEGORIES.map((cat) => {
          const active = keepCategories.has(cat);
          const count = preview?.categoryCounts[cat] ?? 0;
          return (
            <button
              key={cat}
              type="button"
              aria-pressed={active}
              onClick={() => toggleCategory(cat)}
              className="hf-pill"
              style={{
                cursor: 'pointer',
                borderColor: active ? 'var(--hf-accent)' : 'var(--hf-border)',
                backgroundColor: active
                  ? 'rgba(59, 130, 246, 0.16)'
                  : 'var(--hf-surface-alt)',
                color: 'var(--hf-text)',
              }}
            >
              {cat} ({count})
            </button>
          );
        })}
      </div>

      {showRuleForm ? (
        <form
          onSubmit={handleAddRule}
          style={{
            display: 'flex',
            flexWrap: 'wrap',
            gap: '0.5rem',
            paddingTop: '0.5rem',
            borderTop: '1px solid var(--hf-border)',
          }}
        >
          <input
            type="text"
            className="hf-input"
            value={domainSuffix}
            onChange={(e) => setDomainSuffix(e.target.value)}
            placeholder={t.traffic.cleaner.domainSuffix}
            aria-label={t.traffic.cleaner.domainSuffix}
          />
          <input
            type="text"
            className="hf-input"
            value={pathPrefix}
            onChange={(e) => setPathPrefix(e.target.value)}
            placeholder={t.traffic.cleaner.pathPrefix}
            aria-label={t.traffic.cleaner.pathPrefix}
          />
          <input
            type="text"
            className="hf-input"
            value={method}
            onChange={(e) => setMethod(e.target.value)}
            placeholder="GET / POST"
            aria-label="HTTP Method"
            style={{ width: '110px' }}
          />
          <select
            className="hf-input"
            value={action}
            onChange={(e) => setAction(e.target.value as FilterAction)}
            aria-label="Rule action"
          >
            <option value="exclude">{t.traffic.cleaner.actionExclude}</option>
            <option value="keep">{t.traffic.cleaner.actionKeep}</option>
          </select>
          <Button type="submit" variant="primary">
            {t.traffic.cleaner.applyRule}
          </Button>
        </form>
      ) : null}

      {(config?.customRules.length ?? 0) > 0 ? (
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.45rem', alignItems: 'center' }}>
          <span style={{ fontSize: '0.8rem', color: 'var(--hf-text-muted)' }}>
            {t.traffic.cleaner.customRules}:
          </span>
          {config?.customRules.map((rule) => (
            <span key={rule.id} className="hf-pill">
              <code>
                {rule.action.toUpperCase()} {rule.method ?? '*'} {rule.domainSuffix ?? '*'}
                {rule.pathPrefix ?? ''}
              </code>
              <button
                type="button"
                aria-label={`${t.traffic.cleaner.removeRule} ${rule.id}`}
                onClick={() => void onUpdateCleaner({ removeCustomRuleId: rule.id })}
                style={{
                  background: 'transparent',
                  border: 0,
                  color: 'inherit',
                  cursor: 'pointer',
                  fontWeight: 700,
                }}
              >
                ✕
              </button>
            </span>
          ))}
        </div>
      ) : null}

      {showPreview && preview ? (
        <div
          data-testid="cleaner-preview-panel"
          style={{
            maxHeight: '220px',
            overflowY: 'auto',
            borderTop: '1px solid var(--hf-border)',
            paddingTop: '0.6rem',
          }}
        >
          <table className="hf-table">
            <thead>
              <tr>
                <th>{t.traffic.columns.method}</th>
                <th>{t.traffic.columns.endpoint}</th>
                <th>{t.traffic.columns.type}</th>
                <th>{t.traffic.columns.state}</th>
                <th>{t.traffic.filterReason}</th>
              </tr>
            </thead>
            <tbody>
              {preview.decisions.slice(0, 100).map((d) => (
                <tr key={d.entryId}>
                  <td className="hf-mono">{d.method}</td>
                  <td className="hf-mono">
                    {d.host}
                    {d.path}
                  </td>
                  <td>{d.category}</td>
                  <td>{d.kept ? `✓ ${t.traffic.keptBadge}` : `✕ ${t.traffic.excludedBadge}`}</td>
                  <td>{d.filterReason}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
    </section>
  );
}
