import '@testing-library/jest-dom/vitest';
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { StatusBadge } from '../StatusBadge.js';

describe('StatusBadge accessibility and semantic states', () => {
  it('renders both non-color icon and text label for every status', () => {
    const statuses = [
      { status: 'confirmed', icon: '✓', text: 'Confirmed' },
      { status: 'likely', icon: '~', text: 'Likely' },
      { status: 'unknown', icon: '?', text: 'Unknown' },
      { status: 'rejected', icon: '✕', text: 'Rejected' },
      { status: 'user_confirmed', icon: '★', text: 'User Confirmed' },
    ] as const;

    for (const item of statuses) {
      const { unmount } = render(<StatusBadge status={item.status} />);
      expect(screen.getByText(item.icon)).toBeInTheDocument();
      expect(screen.getByText(item.text)).toBeInTheDocument();
      unmount();
    }
  });
});
