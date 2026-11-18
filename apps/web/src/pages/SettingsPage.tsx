import { Button } from '@harflow/ui';
import { useI18n, type Locale } from '../i18n/index.js';
import type { ThemeMode } from '../stores/projectStore.js';

interface SettingsPageProps {
  theme: ThemeMode;
  onToggleTheme: () => void;
  locale: Locale;
  onSetLocale: (locale: Locale) => void;
  adapterMode: string;
}

export function SettingsPage({
  theme,
  onToggleTheme,
  locale,
  onSetLocale,
  adapterMode,
}: SettingsPageProps) {
  const { t } = useI18n();

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
      <h1 style={{ margin: 0, fontSize: '1.35rem' }}>{t.settings.title}</h1>

      <section className="hf-panel" style={{ display: 'flex', flexDirection: 'column', gap: '0.9rem' }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <span>{t.settings.appearance}</span>
          <Button onClick={onToggleTheme}>
            {theme === 'dark' ? t.themeDark : t.themeLight}
          </Button>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <span>{t.settings.language}</span>
          <div style={{ display: 'flex', gap: '0.4rem' }}>
            <Button
              variant={locale === 'en' ? 'primary' : 'secondary'}
              onClick={() => onSetLocale('en')}
            >
              English
            </Button>
            <Button
              variant={locale === 'ru' ? 'primary' : 'secondary'}
              onClick={() => onSetLocale('ru')}
            >
              Русский
            </Button>
          </div>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <span>{t.settings.adapterMode}</span>
          <code className="hf-mono">{adapterMode}</code>
        </div>
      </section>

      <section className="hf-panel">
        <h2 style={{ marginTop: 0, fontSize: '1rem' }}>{t.settings.privacyTitle}</h2>
        <p style={{ margin: 0, color: 'var(--hf-text-muted)' }}>{t.settings.privacyBody}</p>
      </section>

      <section className="hf-panel">
        <h2 style={{ marginTop: 0, fontSize: '1rem' }}>{t.settings.shortcutsTitle}</h2>
        <ul style={{ margin: 0, paddingLeft: '1.2rem' }}>
          {t.settings.shortcutsList.map((item) => (
            <li key={item} className="hf-mono" style={{ fontSize: '0.84rem' }}>
              {item}
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}
