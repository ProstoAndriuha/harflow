import type { ChangeEvent } from 'react';
import { Button, Card } from '@harflow/ui';
import type { AnalysisReport, ProjectManifest } from '@harflow/shared-types';
import { useI18n } from '../i18n/index.js';

interface DashboardPageProps {
  manifest: ProjectManifest | null;
  report: AnalysisReport | null;
  onCreateDemo: () => Promise<void>;
  onImportHarText: (sourceName: string, rawText: string) => Promise<void>;
}

export function DashboardPage({
  manifest,
  report,
  onCreateDemo,
  onImportHarText,
}: DashboardPageProps) {
  const { t } = useI18n();
  const stats = manifest?.statistics;
  const readinessKey = report?.exportReadiness ?? 'not_ready';

  const handleFileChange = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) {
      return;
    }
    const reader = new FileReader();
    reader.onload = () => {
      if (typeof reader.result === 'string') {
        void onImportHarText(file.name, reader.result);
      }
    };
    reader.readAsText(file);
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '1.25rem' }}>
      <div
        style={{
          display: 'flex',
          flexWrap: 'wrap',
          alignItems: 'center',
          justifyContent: 'space-between',
          gap: '0.75rem',
        }}
      >
        <h1 style={{ margin: 0, fontSize: '1.35rem' }}>{t.dashboard.title}</h1>
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.6rem', alignItems: 'center' }}>
          <Button variant="primary" onClick={() => void onCreateDemo()}>
            {t.dashboard.createDemo}
          </Button>
          <label className="hf-pill" style={{ cursor: 'pointer' }}>
            <span>{t.dashboard.importHarLabel}</span>
            <input
              type="file"
              accept=".har,.json"
              onChange={handleFileChange}
              style={{ display: 'none' }}
            />
          </label>
        </div>
      </div>

      <div className="hf-grid-cards">
        <Card title={t.dashboard.cards.totalRequests} value={stats?.totalRequests ?? 0} />
        <Card title={t.dashboard.cards.uniqueEndpoints} value={stats?.uniqueEndpoints ?? 0} />
        <Card title={t.dashboard.cards.apiDomains} value={stats?.apiDomains ?? 0} />
        <Card title={t.dashboard.cards.dependencies} value={stats?.dependencies ?? 0} />
        <Card title={t.dashboard.cards.authFlows} value={stats?.authFlows ?? 0} />
        <Card title={t.dashboard.cards.unknownMappings} value={stats?.unknownMappings ?? 0} />
        <Card
          title={t.dashboard.cards.exportReadiness}
          value={t.dashboard.readiness[readinessKey]}
        />
      </div>
    </div>
  );
}
