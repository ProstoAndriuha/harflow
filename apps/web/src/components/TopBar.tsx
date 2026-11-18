import { Button } from '@harflow/ui';
import { useI18n, type Locale } from '../i18n/index.js';
import type { ActiveTab, ThemeMode } from '../stores/projectStore.js';

interface TopBarProps {
  projectName: string;
  activeTab: ActiveTab;
  onSelectTab: (tab: ActiveTab) => void;
  theme: ThemeMode;
  onToggleTheme: () => void;
  locale: Locale;
  onToggleLocale: () => void;
}

export function TopBar({
  projectName,
  activeTab,
  onSelectTab,
  theme,
  onToggleTheme,
  locale,
  onToggleLocale,
}: TopBarProps) {
  const { t } = useI18n();

  const tabs: Array<{ id: ActiveTab; label: string }> = [
    { id: 'dashboard', label: t.nav.dashboard },
    { id: 'traffic', label: t.nav.traffic },
    { id: 'dependencies', label: t.nav.dependencies },
    { id: 'settings', label: t.nav.settings },
  ];

  return (
    <header className="hf-topbar">
      <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem', flexWrap: 'wrap' }}>
        <strong style={{ fontSize: '1rem' }}>{t.appName}</strong>
        <span className="hf-pill">{projectName}</span>
        <span className="hf-pill" data-testid="offline-badge">
          {t.offlineBadge}
        </span>
      </div>

      <nav className="hf-nav" aria-label="Primary navigation">
        {tabs.map((tab) => (
          <Button
            key={tab.id}
            variant={activeTab === tab.id ? 'primary' : 'secondary'}
            onClick={() => onSelectTab(tab.id)}
          >
            {tab.label}
          </Button>
        ))}
      </nav>

      <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
        <Button onClick={onToggleLocale} aria-label="Switch language">
          {locale.toUpperCase()}
        </Button>
        <Button onClick={onToggleTheme} aria-label="Switch theme">
          {theme === 'dark' ? t.themeDark : t.themeLight}
        </Button>
      </div>
    </header>
  );
}
