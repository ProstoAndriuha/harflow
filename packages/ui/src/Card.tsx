import type { ReactNode } from 'react';

export interface CardProps {
  title: string;
  value?: string | number;
  subtitle?: string;
  children?: ReactNode;
}

export function Card({ title, value, subtitle, children }: CardProps) {
  return (
    <section
      style={{
        backgroundColor: 'var(--hf-surface, #161b22)',
        border: '1px solid var(--hf-border, #30363d)',
        borderRadius: '8px',
        padding: '1rem',
        display: 'flex',
        flexDirection: 'column',
        gap: '0.35rem',
      }}
    >
      <div
        style={{
          fontSize: '0.8rem',
          color: 'var(--hf-text-muted, #8b949e)',
          fontWeight: 600,
          textTransform: 'uppercase',
          letterSpacing: '0.04em',
        }}
      >
        {title}
      </div>
      {value !== undefined ? (
        <div style={{ fontSize: '1.65rem', fontWeight: 700, lineHeight: 1.2 }}>
          {value}
        </div>
      ) : null}
      {subtitle ? (
        <div style={{ fontSize: '0.78rem', color: 'var(--hf-text-muted, #8b949e)' }}>
          {subtitle}
        </div>
      ) : null}
      {children}
    </section>
  );
}
