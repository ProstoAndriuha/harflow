import '@testing-library/jest-dom/vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { App } from '../app/App.js';
import { InMemoryApplicationAdapter } from '../services/applicationAdapter.js';

describe('HARFlow Web Studio', () => {
  it('loads dashboard metrics, toggles theme & EN/RU locale, and confirms a dependency', async () => {
    const adapter = new InMemoryApplicationAdapter();
    render(<App adapter={adapter} />);

    expect(await screen.findByText('Project Dashboard')).toBeInTheDocument();
    expect(screen.getByTestId('offline-badge')).toHaveTextContent(
      'Offline-Only · Secrets Redacted',
    );
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');

    fireEvent.click(screen.getByRole('button', { name: 'Switch theme' }));
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');

    fireEvent.click(screen.getByRole('button', { name: 'Switch language' }));
    expect(screen.getByText('Обзор проекта')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Switch language' }));
    expect(screen.getByText('Project Dashboard')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Dependency Graph' }));
    expect(await screen.findByTestId('dependency-dep-0002')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Confirm dep-0002' }));
    await waitFor(() => {
      const card = screen.getByTestId('dependency-dep-0002');
      expect(card).toHaveTextContent('User Confirmed');
    });
  });
});
