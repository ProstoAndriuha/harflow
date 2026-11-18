import '@testing-library/jest-dom/vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { App } from '../app/App.js';
import { InMemoryApplicationAdapter } from '../services/applicationAdapter.js';

describe('Interactive Dependency Graph Studio (P4-03)', () => {
  it('switches 5 graph perspectives, zooms/fits, filters, edits mapping with undo/redo, and exports SVG/JSON', async () => {
    const adapter = new InMemoryApplicationAdapter();
    render(<App adapter={adapter} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Dependency Graph' }));
    expect(await screen.findByTestId('dependency-graph-canvas')).toBeInTheDocument();

    // Verify all 5 perspectives
    const perspectives = [
      'Request Dependency',
      'Auth Flow',
      'Session Timeline',
      'Data Flow',
      'Endpoint Graph',
    ];
    for (const label of perspectives) {
      const btn = screen.getByRole('button', { name: label });
      fireEvent.click(btn);
      await waitFor(() => {
        expect(btn).toHaveAttribute('aria-pressed', 'true');
      });
    }

    // Zoom controls
    const canvas = screen.getByTestId('dependency-graph-canvas');
    expect(canvas).toHaveAttribute('data-zoom', '1.00');
    fireEvent.click(screen.getByRole('button', { name: 'Zoom In' }));
    expect(canvas).toHaveAttribute('data-zoom', '1.20');
    fireEvent.click(screen.getByRole('button', { name: 'Reset Layout' }));
    expect(canvas).toHaveAttribute('data-zoom', '1.00');

    // Select dep-0002, edit mapping, then undo & redo
    fireEvent.click(screen.getByTestId('dependency-dep-0002'));
    const sourceInput = screen.getByLabelText('Source Path / Key');
    fireEvent.change(sourceInput, { target: { value: '$.payload.orderId' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save Mapping' }));

    await waitFor(() => {
      expect(screen.getByTestId('dependency-dep-0002')).toHaveTextContent('$.payload.orderId');
      expect(screen.getByTestId('dependency-dep-0002')).toHaveTextContent('User Confirmed');
    });

    fireEvent.click(screen.getByRole('button', { name: /Undo/i }));
    await waitFor(() => {
      expect(screen.getByTestId('dependency-dep-0002')).toHaveTextContent('$.orderId');
    });

    fireEvent.click(screen.getByRole('button', { name: /Redo/i }));
    await waitFor(() => {
      expect(screen.getByTestId('dependency-dep-0002')).toHaveTextContent('$.payload.orderId');
    });

    // Export SVG & JSON
    fireEvent.click(screen.getByRole('button', { name: 'Export SVG' }));
    expect(await screen.findByTestId('graph-export-status')).toHaveTextContent(
      'graph-endpoint.svg',
    );
    fireEvent.click(screen.getByRole('button', { name: 'Export JSON' }));
    expect(await screen.findByTestId('graph-export-status')).toHaveTextContent(
      'graph-endpoint.json',
    );
  });
});
