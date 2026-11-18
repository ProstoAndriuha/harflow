import '@testing-library/jest-dom/vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { App } from '../app/App.js';
import { InMemoryApplicationAdapter } from '../services/applicationAdapter.js';

describe('Traffic Explorer, Smart Cleaner & 8-Tab Inspector', () => {
  it('filters traffic, toggles cleaner preview & exclude/restore, and switches all 8 inspector tabs', async () => {
    const adapter = new InMemoryApplicationAdapter();
    render(<App adapter={adapter} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Traffic Explorer' }));

    expect(await screen.findByTestId('smart-cleaner-bar')).toBeInTheDocument();
    expect(screen.getByTestId('cleaner-kept-count')).toHaveTextContent('9');
    expect(screen.getByTestId('cleaner-excluded-count')).toHaveTextContent('1');

    fireEvent.click(screen.getByRole('button', { name: 'Preview Decisions' }));
    expect(screen.getByTestId('cleaner-preview-panel')).toBeInTheDocument();

    // Exclude currently selected request and verify counts update without deleting underlying row
    fireEvent.click(screen.getByRole('button', { name: 'Exclude Request' }));
    await waitFor(() => {
      expect(screen.getByTestId('cleaner-kept-count')).toHaveTextContent('8');
      expect(screen.getByTestId('cleaner-excluded-count')).toHaveTextContent('2');
    });

    fireEvent.click(screen.getByRole('button', { name: 'Restore Request' }));
    await waitFor(() => {
      expect(screen.getByTestId('cleaner-kept-count')).toHaveTextContent('9');
    });

    // Search for orders endpoint
    const searchInput = screen.getByPlaceholderText(
      'Search endpoint, URL, domain, or reason...',
    );
    fireEvent.change(searchInput, { target: { value: '/v1/orders' } });
    expect(screen.queryByText('/collect')).not.toBeInTheDocument();

    // Walk through all 8 inspector tabs
    const tabNames = [
      'Overview',
      'Headers',
      'Query',
      'Cookies',
      'Body',
      'Response',
      'Timing',
      'Security',
    ];
    for (const tabName of tabNames) {
      const tab = screen.getByRole('tab', { name: tabName });
      fireEvent.click(tab);
      expect(tab).toHaveAttribute('aria-selected', 'true');
    }

    // Verify ArrowRight wraps back to Overview from Security
    fireEvent.keyDown(screen.getByRole('tab', { name: 'Security' }), {
      key: 'ArrowRight',
    });
    expect(screen.getByRole('tab', { name: 'Overview' })).toHaveAttribute(
      'aria-selected',
      'true',
    );
  });
});
