import { useCallback, useEffect, useMemo, useState } from 'react';
import type { ApplicationAdapter } from '@harflow/shared-types';
import { CommandPalette, type PaletteCommand } from '../components/CommandPalette.js';
import { TopBar } from '../components/TopBar.js';
import { useHarflowStore } from '../hooks/useHarflowStore.js';
import { DICTIONARIES, I18nContext, type Locale } from '../i18n/index.js';
import { DashboardPage } from '../pages/DashboardPage.js';
import { DependencyGraphPage } from '../pages/DependencyGraphPage.js';
import { EndpointsSchemasPage } from '../pages/EndpointsSchemasPage.js';
import { SessionsComparisonPage } from '../pages/SessionsComparisonPage.js';
import { SettingsPage } from '../pages/SettingsPage.js';
import { TrafficExplorerPage } from '../pages/TrafficExplorerPage.js';
import { createApplicationAdapter } from '../services/applicationAdapter.js';
import type { ActiveTab, ThemeMode } from '../stores/projectStore.js';

const ORDERED_TABS: readonly ActiveTab[] = [
  'dashboard',
  'traffic',
  'endpoints',
  'dependencies',
  'sessions',
  'settings',
] as const;

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
    canUndoAction,
    canRedoAction,
    selectEntry,
    selectEndpoint,
    selectDependency,
    setGraphKind,
    loadDemoProject,
    importHarFiles,
    confirmDependency,
    rejectDependency,
    editDependencyMapping,
    updateCleaner,
    setEndpointOverride,
    undo,
    redo,
    exportArtifact,
  } = useHarflowStore(adapter);

  const [activeTab, setActiveTab] = useState<ActiveTab>('dashboard');
  const [theme, setTheme] = useState<ThemeMode>('dark');
  const [locale, setLocale] = useState<Locale>('en');
  const [paletteOpen, setPaletteOpen] = useState(false);

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
  }, [theme]);

  const toggleTheme = useCallback(
    () => setTheme((prev) => (prev === 'dark' ? 'light' : 'dark')),
    [],
  );
  const toggleLocale = useCallback(
    () => setLocale((prev) => (prev === 'en' ? 'ru' : 'en')),
    [],
  );

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const key = e.key.toLowerCase();
      const mod = e.ctrlKey || e.metaKey;

      if (mod && key === 'k') {
        e.preventDefault();
        setPaletteOpen((prev) => !prev);
        return;
      }

      const target = e.target as HTMLElement | null;
      const tag = target?.tagName?.toUpperCase() ?? '';
      if (
        tag === 'INPUT' ||
        tag === 'TEXTAREA' ||
        tag === 'SELECT' ||
        target?.isContentEditable
      ) {
        return;
      }

      if (mod && !e.shiftKey && key === 'z') {
        e.preventDefault();
        void undo();
      } else if ((mod && key === 'y') || (mod && e.shiftKey && key === 'z')) {
        e.preventDefault();
        void redo();
      } else if (e.altKey && ['1', '2', '3', '4', '5', '6'].includes(e.key)) {
        const idx = Number(e.key) - 1;
        const tab = ORDERED_TABS[idx];
        if (tab) {
          e.preventDefault();
          setActiveTab(tab);
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [undo, redo]);

  const t = DICTIONARIES[locale];
  const i18nValue = useMemo(
    () => ({ locale, t, setLocale }),
    [locale, t],
  );

  const paletteCommands = useMemo<PaletteCommand[]>(() => {
    const base: PaletteCommand[] = [
      {
        id: 'nav-dashboard',
        group: 'Navigation',
        label: t.nav.dashboard,
        shortcut: 'Alt+1',
        action: () => setActiveTab('dashboard'),
      },
      {
        id: 'nav-traffic',
        group: 'Navigation',
        label: t.nav.traffic,
        shortcut: 'Alt+2',
        action: () => setActiveTab('traffic'),
      },
      {
        id: 'nav-endpoints',
        group: 'Navigation',
        label: t.nav.endpoints,
        shortcut: 'Alt+3',
        action: () => setActiveTab('endpoints'),
      },
      {
        id: 'nav-dependencies',
        group: 'Navigation',
        label: t.nav.dependencies,
        shortcut: 'Alt+4',
        action: () => setActiveTab('dependencies'),
      },
      {
        id: 'nav-sessions',
        group: 'Navigation',
        label: t.nav.sessions,
        shortcut: 'Alt+5',
        action: () => setActiveTab('sessions'),
      },
      {
        id: 'nav-settings',
        group: 'Navigation',
        label: t.nav.settings,
        shortcut: 'Alt+6',
        action: () => setActiveTab('settings'),
      },
      {
        id: 'action-undo',
        group: 'History',
        label: t.undo,
        shortcut: 'Ctrl+Z',
        disabled: !canUndoAction,
        action: undo,
      },
      {
        id: 'action-redo',
        group: 'History',
        label: t.redo,
        shortcut: 'Ctrl+Y',
        disabled: !canRedoAction,
        action: redo,
      },
      {
        id: 'action-theme',
        group: 'Preferences',
        label: `${t.settings.appearance}: ${theme === 'dark' ? t.themeLight : t.themeDark}`,
        action: toggleTheme,
      },
      {
        id: 'action-locale',
        group: 'Preferences',
        label: `${t.settings.language}: ${locale === 'en' ? 'RU' : 'EN'}`,
        action: toggleLocale,
      },
      {
        id: 'action-demo',
        group: 'Project',
        label: t.dashboard.createDemo,
        action: loadDemoProject,
      },
      {
        id: 'export-svg',
        group: 'Export',
        label: t.dependencies.toolbar.exportSvg,
        action: async () => {
          await exportArtifact('graph_svg', state.graphKind);
        },
      },
      {
        id: 'export-json',
        group: 'Export',
        label: t.dependencies.toolbar.exportJson,
        action: async () => {
          await exportArtifact('graph_json', state.graphKind);
        },
      },
    ];

    const endpointJumps: PaletteCommand[] = (state.model?.endpoints ?? []).map((ep) => ({
      id: `jump-${ep.id}`,
      group: 'Endpoints',
      label: `${ep.method} ${ep.pathTemplate} (${ep.host})`,
      action: () => {
        selectEndpoint(ep.id);
        setActiveTab('endpoints');
      },
    }));

    return [...base, ...endpointJumps];
  }, [
    t,
    canUndoAction,
    canRedoAction,
    undo,
    redo,
    theme,
    toggleTheme,
    locale,
    toggleLocale,
    loadDemoProject,
    exportArtifact,
    state.graphKind,
    state.model?.endpoints,
    selectEndpoint,
  ]);

  return (
    <I18nContext.Provider value={i18nValue}>
      <div className="hf-shell" data-theme={theme}>
        <a href="#hf-main-content" className="hf-skip-link">
          {t.skipToContent}
        </a>

        <TopBar
          projectName={state.manifest?.name ?? 'HARFlow'}
          activeTab={activeTab}
          onSelectTab={setActiveTab}
          theme={theme}
          onToggleTheme={toggleTheme}
          locale={locale}
          onToggleLocale={toggleLocale}
          onOpenPalette={() => setPaletteOpen(true)}
        />

        <main id="hf-main-content" className="hf-main">
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
              onImportHarFiles={importHarFiles}
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

          {activeTab === 'endpoints' ? (
            <EndpointsSchemasPage
              endpoints={state.model?.endpoints ?? []}
              schemas={state.model?.schemas ?? []}
              selectedEndpointId={state.selectedEndpointId}
              onSelectEndpoint={selectEndpoint}
              onOverrideEndpoint={setEndpointOverride}
            />
          ) : null}

          {activeTab === 'dependencies' ? (
            <DependencyGraphPage
              dependencies={state.model?.dependencies ?? []}
              graph={state.graph}
              graphKind={state.graphKind}
              selectedDependencyId={state.selectedDependencyId}
              canUndo={canUndoAction}
              canRedo={canRedoAction}
              onChangeGraphKind={setGraphKind}
              onSelectDependency={selectDependency}
              onConfirm={confirmDependency}
              onReject={rejectDependency}
              onEditMapping={editDependencyMapping}
              onUndo={undo}
              onRedo={redo}
              onExportGraph={(format) => exportArtifact(format, state.graphKind)}
            />
          ) : null}

          {activeTab === 'sessions' ? (
            <SessionsComparisonPage
              sources={state.model?.sources ?? []}
              lifecycle={state.sessionLifecycle}
              comparison={state.sessionComparison}
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

        <CommandPalette
          open={paletteOpen}
          commands={paletteCommands}
          onClose={() => setPaletteOpen(false)}
        />
      </div>
    </I18nContext.Provider>
  );
}
