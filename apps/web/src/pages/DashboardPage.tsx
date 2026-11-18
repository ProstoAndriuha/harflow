import { Button, Card } from '@harflow/ui';
import type { AnalysisReport, ProjectManifest } from '@harflow/shared-types';
import { HarDropzone } from '../components/HarDropzone.js';
import { useI18n } from '../i18n/index.js';

interface DashboardPageProps {
  manifest: ProjectManifest | null;
  report: AnalysisReport | null;
  onCreateDemo: () => Promise<void>;
  onImportHarFiles: (files: Array<{ sourceName: string; rawText: string }>) => Promise<void>;
}

export function DashboardPage({
  manifest,
  report,
  onCreateDemo,
  onImportHarFiles,
}: DashboardPageProps) {
  const { t } = useI18n();
  const stats = manifest?.statistics;
  const readinessKey = report?.exportReadiness ?? 'not_ready';

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
        </div>
      </div>

      <HarDropzone
        maxBytes={manifest?.settings.maxHarSizeBytes}
        onImportFiles={onImportHarFiles}
      />

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

      {manifest && manifest.sources.length > 0 ? (
        <section className="hf-panel">
          <h2 style={{ marginTop: 0, fontSize: '1rem' }}>
            {t.dashboard.importedSourcesTitle} ({manifest.sources.length})
          </h2>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.5rem' }}>
            {manifest.sources.map((src) => (
              <span key={src.id} className="hf-pill">
                <code className="hf-mono">{src.id}</code> · {src.name} ({src.entryCount} req,{' '}
                {src.redactedSecretsCount} redacted)
              </span>
            ))}
          </div>
        </section>
      ) : null}
    </div>
  );
}
