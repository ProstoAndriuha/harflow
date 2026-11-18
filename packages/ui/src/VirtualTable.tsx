import { useCallback, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';

export interface VirtualColumn<T> {
  id: string;
  header: string;
  width?: string;
  render: (row: T) => ReactNode;
}

export interface VirtualTableProps<T> {
  rows: T[];
  columns: Array<VirtualColumn<T>>;
  getRowId: (row: T) => string;
  selectedRowId?: string | null;
  onSelectRow?: (rowId: string) => void;
  rowHeight?: number;
  viewportHeight?: number;
  overscan?: number;
  ariaLabel?: string;
  emptyMessage?: string;
}

export function VirtualTable<T>({
  rows,
  columns,
  getRowId,
  selectedRowId,
  onSelectRow,
  rowHeight = 36,
  viewportHeight = 420,
  overscan = 8,
  ariaLabel = 'Virtualized table',
  emptyMessage = 'No rows',
}: VirtualTableProps<T>) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [scrollTop, setScrollTop] = useState(0);

  const visibleCount = Math.max(1, Math.ceil(viewportHeight / rowHeight));
  const totalHeight = rows.length * rowHeight;

  const { startIndex, endIndex, topSpacer, bottomSpacer } = useMemo(() => {
    if (rows.length === 0) {
      return { startIndex: 0, endIndex: 0, topSpacer: 0, bottomSpacer: 0 };
    }
    const rawStart = Math.floor(scrollTop / rowHeight);
    const start = Math.max(0, rawStart - overscan);
    const end = Math.min(rows.length, rawStart + visibleCount + overscan);
    return {
      startIndex: start,
      endIndex: end,
      topSpacer: start * rowHeight,
      bottomSpacer: Math.max(0, (rows.length - end) * rowHeight),
    };
  }, [rows.length, scrollTop, rowHeight, overscan, visibleCount]);

  const visibleRows = useMemo(
    () => rows.slice(startIndex, endIndex),
    [rows, startIndex, endIndex],
  );

  const ensureRowVisible = useCallback(
    (index: number) => {
      const container = containerRef.current;
      if (!container) {
        return;
      }
      const rowTop = index * rowHeight;
      const rowBottom = rowTop + rowHeight;
      if (rowTop < container.scrollTop) {
        container.scrollTop = rowTop;
        setScrollTop(rowTop);
      } else if (rowBottom > container.scrollTop + viewportHeight) {
        const nextTop = Math.max(0, rowBottom - viewportHeight);
        container.scrollTop = nextTop;
        setScrollTop(nextTop);
      }
    },
    [rowHeight, viewportHeight],
  );

  const handleKeyDown = useCallback(
    (e: KeyboardEvent<HTMLDivElement>) => {
      if (rows.length === 0 || !onSelectRow) {
        return;
      }
      const currentIdx = rows.findIndex((r) => getRowId(r) === selectedRowId);
      let nextIdx = currentIdx;

      if (e.key === 'ArrowDown') {
        e.preventDefault();
        nextIdx = currentIdx < 0 ? 0 : Math.min(rows.length - 1, currentIdx + 1);
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        nextIdx = currentIdx <= 0 ? 0 : currentIdx - 1;
      } else if (e.key === 'Home') {
        e.preventDefault();
        nextIdx = 0;
      } else if (e.key === 'End') {
        e.preventDefault();
        nextIdx = rows.length - 1;
      } else if (e.key === 'Enter' && currentIdx >= 0) {
        e.preventDefault();
        onSelectRow(getRowId(rows[currentIdx]!));
        return;
      } else {
        return;
      }

      const nextRow = rows[nextIdx];
      if (nextRow) {
        onSelectRow(getRowId(nextRow));
        ensureRowVisible(nextIdx);
      }
    },
    [rows, getRowId, selectedRowId, onSelectRow, ensureRowVisible],
  );

  if (rows.length === 0) {
    return (
      <div className="hf-panel" role="status">
        {emptyMessage}
      </div>
    );
  }

  return (
    <div
      ref={containerRef}
      className="hf-table-wrap"
      tabIndex={0}
      role="region"
      aria-label={ariaLabel}
      data-testid="virtual-table-viewport"
      onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
      onKeyDown={handleKeyDown}
      style={{
        height: `${viewportHeight}px`,
        overflowY: 'auto',
        overflowX: 'auto',
        position: 'relative',
      }}
    >
      <table
        className="hf-table"
        aria-rowcount={rows.length}
        data-total-height={totalHeight}
      >
        <thead
          style={{
            position: 'sticky',
            top: 0,
            backgroundColor: 'var(--hf-surface, #161b22)',
            zIndex: 1,
          }}
        >
          <tr>
            {columns.map((col) => (
              <th key={col.id} style={col.width ? { width: col.width } : undefined}>
                {col.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {topSpacer > 0 ? (
            <tr data-spacer="top" aria-hidden="true" style={{ height: `${topSpacer}px` }}>
              <td colSpan={columns.length} style={{ padding: 0, border: 0 }} />
            </tr>
          ) : null}

          {visibleRows.map((row, offset) => {
            const absoluteIdx = startIndex + offset;
            const id = getRowId(row);
            const selected = selectedRowId === id;
            return (
              <tr
                key={id}
                data-row-id={id}
                data-row-index={absoluteIdx}
                data-selected={selected ? 'true' : 'false'}
                aria-selected={selected}
                aria-rowindex={absoluteIdx + 1}
                onClick={() => onSelectRow?.(id)}
                style={{ height: `${rowHeight}px` }}
              >
                {columns.map((col) => (
                  <td key={col.id}>{col.render(row)}</td>
                ))}
              </tr>
            );
          })}

          {bottomSpacer > 0 ? (
            <tr data-spacer="bottom" aria-hidden="true" style={{ height: `${bottomSpacer}px` }}>
              <td colSpan={columns.length} style={{ padding: 0, border: 0 }} />
            </tr>
          ) : null}
        </tbody>
      </table>
    </div>
  );
}
