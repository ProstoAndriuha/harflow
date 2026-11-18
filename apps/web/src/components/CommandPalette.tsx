import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { Button } from '@harflow/ui';
import { useI18n } from '../i18n/index.js';

export interface PaletteCommand {
  id: string;
  label: string;
  group: string;
  shortcut?: string;
  disabled?: boolean;
  action: () => void | Promise<void>;
}

interface CommandPaletteProps {
  open: boolean;
  commands: PaletteCommand[];
  onClose: () => void;
}

export function CommandPalette({ open, commands, onClose }: CommandPaletteProps) {
  const { t } = useI18n();
  const [query, setQuery] = useState('');
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    if (open) {
      setQuery('');
      setActiveIndex(0);
      inputRef.current?.focus();
    }
  }, [open]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) {
      return commands;
    }
    return commands.filter(
      (c) =>
        c.label.toLowerCase().includes(q) || c.group.toLowerCase().includes(q),
    );
  }, [commands, query]);

  if (!open) {
    return null;
  }

  const clampedIdx =
    filtered.length === 0 ? 0 : Math.min(activeIndex, filtered.length - 1);

  const runCommand = (cmd: PaletteCommand | undefined) => {
    if (!cmd || cmd.disabled) {
      return;
    }
    onClose();
    void cmd.action();
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (filtered.length > 0) {
        setActiveIndex((i) => (i + 1) % filtered.length);
      }
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (filtered.length > 0) {
        setActiveIndex((i) => (i - 1 + filtered.length) % filtered.length);
      }
    } else if (e.key === 'Enter') {
      e.preventDefault();
      runCommand(filtered[clampedIdx]);
    }
  };

  return (
    <div
      className="hf-palette-backdrop"
      data-testid="command-palette-backdrop"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={t.palette.title}
        className="hf-palette-dialog"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={handleKeyDown}
      >
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: '0.5rem' }}>
          <strong>{t.palette.title}</strong>
          <Button onClick={onClose}>{t.palette.close}</Button>
        </div>

        <input
          ref={inputRef}
          type="search"
          className="hf-input"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setActiveIndex(0);
          }}
          placeholder={t.palette.placeholder}
          aria-label={t.palette.placeholder}
        />

        {filtered.length === 0 ? (
          <div style={{ color: 'var(--hf-text-muted)', fontSize: '0.85rem', padding: '0.5rem 0' }}>
            {t.palette.noResults}
          </div>
        ) : (
          <ul
            role="listbox"
            aria-label={t.palette.title}
            style={{
              listStyle: 'none',
              margin: 0,
              padding: 0,
              maxHeight: '280px',
              overflowY: 'auto',
              display: 'flex',
              flexDirection: 'column',
              gap: '0.3rem',
            }}
          >
            {filtered.map((cmd, idx) => {
              const selected = idx === clampedIdx;
              return (
                <li key={cmd.id} role="option" aria-selected={selected}>
                  <button
                    type="button"
                    disabled={cmd.disabled}
                    onClick={() => runCommand(cmd)}
                    style={{
                      width: '100%',
                      textAlign: 'left',
                      display: 'flex',
                      justifyContent: 'space-between',
                      alignItems: 'center',
                      gap: '0.6rem',
                      padding: '0.5rem 0.7rem',
                      borderRadius: '6px',
                      border: `1px solid ${selected ? 'var(--hf-accent)' : 'transparent'}`,
                      backgroundColor: selected
                        ? 'var(--hf-surface-alt)'
                        : 'transparent',
                      color: 'var(--hf-text)',
                      cursor: cmd.disabled ? 'not-allowed' : 'pointer',
                      opacity: cmd.disabled ? 0.5 : 1,
                    }}
                  >
                    <span>
                      <span style={{ fontSize: '0.74rem', color: 'var(--hf-text-muted)', marginRight: '0.5rem' }}>
                        [{cmd.group}]
                      </span>
                      {cmd.label}
                    </span>
                    {cmd.shortcut ? (
                      <kbd className="hf-pill">{cmd.shortcut}</kbd>
                    ) : null}
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </div>
  );
}
