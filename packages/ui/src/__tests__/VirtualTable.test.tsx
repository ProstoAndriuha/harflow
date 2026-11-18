import '@testing-library/jest-dom/vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { VirtualTable, type VirtualColumn } from '../VirtualTable.js';

interface TestRow {
  id: string;
  label: string;
}

describe('VirtualTable', () => {
  const columns: Array<VirtualColumn<TestRow>> = [
    { id: 'id', header: 'ID', render: (r) => r.id },
    { id: 'label', header: 'Label', render: (r) => r.label },
  ];

  it('window-virtualizes 10,000 rows cleanly and supports scroll + keyboard navigation', () => {
    const rows: TestRow[] = Array.from({ length: 10000 }, (_, i) => ({
      id: `row-${i}`,
      label: `Entry ${i}`,
    }));
    const onSelect = vi.fn();

    const { container } = render(
      <VirtualTable
        rows={rows}
        columns={columns}
        getRowId={(r) => r.id}
        selectedRowId="row-0"
        onSelectRow={onSelect}
        rowHeight={36}
        viewportHeight={360}
        overscan={4}
      />,
    );

    const initialRendered = container.querySelectorAll('tbody tr[data-row-id]');
    expect(initialRendered.length).toBeLessThan(40);
    expect(screen.getByText('Entry 0')).toBeInTheDocument();

    const viewport = screen.getByTestId('virtual-table-viewport');
    fireEvent.scroll(viewport, { target: { scrollTop: 3600 } });
    expect(screen.getByText('Entry 102')).toBeInTheDocument();

    fireEvent.keyDown(viewport, { key: 'ArrowDown' });
    expect(onSelect).toHaveBeenCalledWith('row-1');
  });
});
