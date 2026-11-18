import type { ConfirmationStatus } from '@harflow/shared-types';

export interface StatusBadgeProps {
  status: ConfirmationStatus;
  label?: string;
}

const STATUS_META: Record<
  ConfirmationStatus,
  { icon: string; defaultLabel: string; colorVar: string; bgVar: string }
> = {
  confirmed: {
    icon: '✓',
    defaultLabel: 'Confirmed',
    colorVar: 'var(--hf-status-confirmed, #22c55e)',
    bgVar: 'rgba(34, 197, 94, 0.14)',
  },
  likely: {
    icon: '~',
    defaultLabel: 'Likely',
    colorVar: 'var(--hf-status-likely, #eab308)',
    bgVar: 'rgba(234, 179, 8, 0.14)',
  },
  unknown: {
    icon: '?',
    defaultLabel: 'Unknown',
    colorVar: 'var(--hf-status-unknown, #94a3b8)',
    bgVar: 'rgba(148, 163, 184, 0.14)',
  },
  rejected: {
    icon: '✕',
    defaultLabel: 'Rejected',
    colorVar: 'var(--hf-status-rejected, #ef4444)',
    bgVar: 'rgba(239, 68, 68, 0.14)',
  },
  user_confirmed: {
    icon: '★',
    defaultLabel: 'User Confirmed',
    colorVar: 'var(--hf-status-user, #3b82f6)',
    bgVar: 'rgba(59, 130, 246, 0.14)',
  },
};

export function StatusBadge({ status, label }: StatusBadgeProps) {
  const meta = STATUS_META[status];
  const text = label ?? meta.defaultLabel;

  return (
    <span
      data-status={status}
      aria-label={`Status: ${text}`}
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: '0.35rem',
        padding: '0.18rem 0.55rem',
        borderRadius: '999px',
        fontSize: '0.78rem',
        fontWeight: 600,
        border: `1px solid ${meta.colorVar}`,
        color: meta.colorVar,
        backgroundColor: meta.bgVar,
      }}
    >
      <span aria-hidden="true">{meta.icon}</span>
      <span>{text}</span>
    </span>
  );
}
