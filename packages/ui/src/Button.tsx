import type { ButtonHTMLAttributes } from 'react';

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'danger';
}

export function Button({
  variant = 'secondary',
  style,
  type = 'button',
  ...rest
}: ButtonProps) {
  const palette =
    variant === 'primary'
      ? { bg: '#2563eb', border: '#3b82f6', color: '#ffffff' }
      : variant === 'danger'
        ? { bg: 'rgba(239, 68, 68, 0.16)', border: '#ef4444', color: '#ef4444' }
        : {
            bg: 'var(--hf-surface-alt, #21262d)',
            border: 'var(--hf-border, #30363d)',
            color: 'var(--hf-text, #f0f6fc)',
          };

  return (
    <button
      type={type}
      {...rest}
      style={{
        cursor: rest.disabled ? 'not-allowed' : 'pointer',
        borderRadius: '6px',
        padding: '0.45rem 0.85rem',
        fontSize: '0.85rem',
        fontWeight: 600,
        border: `1px solid ${palette.border}`,
        backgroundColor: palette.bg,
        color: palette.color,
        opacity: rest.disabled ? 0.55 : 1,
        ...style,
      }}
    />
  );
}
