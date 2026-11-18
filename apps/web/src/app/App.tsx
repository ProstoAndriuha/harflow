import { useEffect, useMemo, useState } from 'react';
import type { ApplicationAdapter } from '@harflow/shared-types';
import { TopBar } from '../components/TopBar.js';
import { useHarflowStore } from '../hooks/useHarflowStore.js';
import { DICTIONARIES, I18nContext, type Locale } from '../i18n/index.js';
import { DashboardPage } from '../pages/DashboardPage.js';
import { DependencyGraphPage } from '../pages/DependencyGraphPage.js';
import { SettingsPage } from '../pages/SettingsPage.js';
import { TrafficExplorerPage } from '../pages/TrafficExplorerPage.js';
import { createApplicationAdapter } from '../services/applicationAdapter.js';
import type { ActiveTab, ThemeMode } from '../stores/projectStore.js';

export interface AppProps {
  adapter?: ApplicationAdapter;
}

export function App({ adapter: providedAdapter }: AppProps) {
  const adapter = useMemo(
    () => providedAdapter ?? createApplicationAdapter(),
    [providedAdapter],
  );
  const {
    state,
    selectEntry,
    loadDemoProject,
    importHarText,
    confirmDependency,
    rejectDependency,
    updateCleaner,
  } = useHarflowStore(adapter);

  const [activeTab, setActiveTab] = useState<ActiveTab>('dashboard');
  const [theme, setTheme] = useState<ThemeMode>('dark');
  const [locale, setLocale] = useState<Locale>('en');

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
  }, [theme]);

  const toggleTheme = () => setTheme((prev) => (prev === 'dark' ? 'light' : 'dark'));
  const toggleLocale = () => setLocale((prev) => (prev === 'en' ? 'ru' : 'en'));

  const i18nValue = useMemo(
    () => ({ locale, t: DICTIONARIES[locale], setLocale }),
    [locale],
  );

  return (
    <I18nContext.Provider value={i18nValue}>
      <div className="hf-shell" data-theme={theme}>
        <TopBar
          projectName={state.manifest?.name ?? 'HARFlow'}
          activeTab={activeTab}
          onSelectTab={setActiveTab}
          theme={theme}
          onToggleTheme={toggleTheme}
          locale={locale}
          onToggleLocale={toggleLocale}
        />

        <main className="hf-main">
          {state.error ? (
            <div className="hf-panel" role="alert" style={{ borderColor: '#ef4444' }}>
              {state.error}
            </div>
          ) : null}

          {activeTab === 'dashboard' ? (
            <DashboardPage
              manifest={state.manifest}
              report={state.report}
              onCreateDemo={loadDemoProject}
              onImportHarText={importHarText}
            />
          ) : null}

          {activeTab === 'traffic' ? (
            <TrafficExplorerPage
              entries={state.entries}
              selectedEntryId={state.selectedEntryId}
              onSelectEntry={selectEntry}
              cleanerPreview={state.cleanerPreview}
              cleanerConfig={state.manifest?.settings.cleaner}
              onUpdateCleaner={updateCleaner}
            />
          ) : null}

          {activeTab === 'dependencies' ? (
            <DependencyGraphPage
              dependencies={state.model?.dependencies ?? []}
              onConfirm={confirmDependency}
              onReject={rejectDependency}
            />
          ) : null}

          {activeTab === 'settings' ? (
            <SettingsPage
              theme={theme}
              onToggleTheme={toggleTheme}
              locale={locale}
              onSetLocale={setLocale}
              adapterMode={adapter.mode}
            />
          ) : null}
        </main>
      </div>
    </I18nContext.Provider>
  );
}
