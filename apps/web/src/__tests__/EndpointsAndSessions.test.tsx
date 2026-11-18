import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { App } from '../app/App.js';
import { InMemoryApplicationAdapter } from '../services/applicationAdapter.js';

afterEach(() => {
  cleanup();
});

describe('Endpoints, Recursive Schemas & Multi-HAR Session Comparison (P4-04)', () => {
  it('displays clustered endpoints, recursive schema badges, template override, and TS/Pydantic previews', async () => {
    const adapter = new InMemoryApplicationAdapter();
    render(<App adapter={adapter} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Endpoints & Schemas' }));

    const ep2 = await screen.findByTestId('endpoint-ep-0002');
    fireEvent.click(ep2);

    // Request/response schema tree badges
    expect(screen.getAllByText(/required/).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/optional/).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/nullable/).length).toBeGreaterThan(0);
    expect(screen.getAllByText('enum').length).toBeGreaterThan(0);

    // TypeScript & Python Pydantic preview tabs
    const tsButtons = screen.getAllByRole('button', { name: 'TypeScript' });
    fireEvent.click(tsButtons[0]!);
    expect(screen.getByText(/export interface/)).toBeInTheDocument();

    const pyButtons = screen.getAllByRole('button', { name: 'Python Pydantic' });
    fireEvent.click(pyButtons[0]!);
    expect(screen.getByText(/class .*\(BaseModel\):/)).toBeInTheDocument();

    // Override endpoint path template on ep-0003
    fireEvent.click(screen.getByTestId('endpoint-ep-0003'));
    const overrideInput = screen.getByLabelText('Override Path Template');
    fireEvent.change(overrideInput, { target: { value: '/v1/orders/{customId}' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save Template' }));

    await waitFor(() => {
      expect(screen.getByTestId('endpoint-ep-0003')).toHaveTextContent('/v1/orders/{customId}');
    });

    fireEvent.click(screen.getByRole('button', { name: 'Reset' }));
    await waitFor(() => {
      expect(screen.getByTestId('endpoint-ep-0003')).toHaveTextContent('/v1/orders/{orderId}');
    });
  });

  it('displays redacted token rotations, expiration challenges, parameter variability, and schema drift', async () => {
    const adapter = new InMemoryApplicationAdapter();
    render(<App adapter={adapter} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Sessions & Diff' }));

    expect(await screen.findByTestId('session-lifecycle-panel')).toBeInTheDocument();
    expect(screen.getByTestId('auto-refresh-badge')).toHaveTextContent('Auto-Refresh Eligible');
    expect(screen.getByText(/e5f6a7b8/)).toBeInTheDocument();
    expect(screen.getByText(/11223344/)).toBeInTheDocument();
    expect(screen.getByText(/HTTP 401 authorization\/session challenge/)).toBeInTheDocument();

    expect(screen.getByTestId('session-comparison-panel')).toBeInTheDocument();
    expect(screen.getAllByTestId('schema-drift-entry').length).toBeGreaterThan(0);
    expect(screen.getByText('request_volatile')).toBeInTheDocument();
    expect(screen.getByText('session_scoped')).toBeInTheDocument();
  });
});
