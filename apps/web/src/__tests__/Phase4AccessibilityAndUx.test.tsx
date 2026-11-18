import '@testing-library/jest-dom/vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { App } from '../app/App.js';
import { DICTIONARIES } from '../i18n/index.js';
import { InMemoryApplicationAdapter } from '../services/applicationAdapter.js';

afterEach(() => {
  cleanup();
});

async function gzipText(raw: string): Promise<Uint8Array> {
  const inputBytes = new TextEncoder().encode(raw);
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      controller.enqueue(inputBytes);
      controller.close();
    },
  }).pipeThrough(
    new CompressionStream('gzip') as ReadableWritablePair<Uint8Array, Uint8Array>,
  );
  const reader = stream.getReader();
  const chunks: Uint8Array[] = [];
  let total = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    if (value) {
      total += value.byteLength;
      chunks.push(value);
    }
  }
  const merged = new Uint8Array(total);
  let offset = 0;
  for (const c of chunks) {
    merged.set(c, offset);
    offset += c.byteLength;
  }
  return merged;
}

function collectDictionaryLeaves(
  obj: unknown,
  prefix = '',
): Array<{ key: string; value: string }> {
  if (typeof obj === 'string') {
    return [{ key: prefix, value: obj }];
  }
  if (Array.isArray(obj)) {
    return obj.flatMap((item, idx) =>
      collectDictionaryLeaves(item, `${prefix}[${idx}]`),
    );
  }
  if (obj !== null && typeof obj === 'object') {
    return Object.entries(obj as Record<string, unknown>)
      .sort(([a], [b]) => a.localeCompare(b))
      .flatMap(([k, v]) =>
        collectDictionaryLeaves(v, prefix ? `${prefix}.${k}` : k),
      );
  }
  return [];
}

describe('Phase 4 Accessibility, Shortcuts, Dropzone & i18n Parity (P4-06)', () => {
  it('opens Command Palette via Ctrl+K, navigates with keyboard, and handles Escape', async () => {
    const adapter = new InMemoryApplicationAdapter();
    render(<App adapter={adapter} />);

    expect(await screen.findByText('Project Dashboard')).toBeInTheDocument();

    fireEvent.keyDown(window, { key: 'k', ctrlKey: true });
    const dialog = screen.getByRole('dialog', { name: 'Command Palette' });
    expect(dialog).toBeInTheDocument();

    const searchBox = screen.getByPlaceholderText(
      'Type a command or jump to a view...',
    );
    fireEvent.change(searchBox, { target: { value: 'Settings' } });
    fireEvent.keyDown(dialog, { key: 'ArrowDown' });
    fireEvent.keyDown(dialog, { key: 'Enter' });

    await waitFor(() => {
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
      expect(screen.getByText('Keyboard Shortcuts')).toBeInTheDocument();
    });

    // Reopen and close with Escape
    fireEvent.keyDown(window, { key: 'k', ctrlKey: true });
    const reopened = screen.getByRole('dialog', { name: 'Command Palette' });
    fireEvent.keyDown(reopened, { key: 'Escape' });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('supports Alt+4 tab switching and global Ctrl+Z / Ctrl+Y undo-redo outside inputs', async () => {
    const adapter = new InMemoryApplicationAdapter();
    render(<App adapter={adapter} />);
    expect(await screen.findByText('Project Dashboard')).toBeInTheDocument();

    fireEvent.keyDown(window, { key: '4', altKey: true });
    expect(await screen.findByTestId('dependency-dep-0002')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Confirm dep-0002' }));
    await waitFor(() => {
      expect(screen.getByTestId('dependency-dep-0002')).toHaveTextContent('User Confirmed');
    });

    // Ctrl+Z inside an input should NOT trigger global undo
    const filterInput = screen.getByPlaceholderText('Filter nodes, paths, or keys...');
    fireEvent.keyDown(filterInput, { key: 'z', ctrlKey: true });
    expect(screen.getByTestId('dependency-dep-0002')).toHaveTextContent('User Confirmed');

    // Global Ctrl+Z / Ctrl+Y triggers undo and redo
    fireEvent.keyDown(window, { key: 'z', ctrlKey: true });
    await waitFor(() => {
      expect(screen.getByTestId('dependency-dep-0002')).toHaveTextContent('Likely');
    });

    fireEvent.keyDown(window, { key: 'y', ctrlKey: true });
    await waitFor(() => {
      expect(screen.getByTestId('dependency-dep-0002')).toHaveTextContent('User Confirmed');
    });
  });

  it('imports multi-HAR (.har + .har.gz) via drag-and-drop and reports malformed inputs safely', async () => {
    const adapter = new InMemoryApplicationAdapter();
    render(<App adapter={adapter} />);

    expect(await screen.findByText(/Imported Session Sources \(2\)/)).toBeInTheDocument();

    const makeHar = (path: string) =>
      JSON.stringify({
        log: {
          version: '1.2',
          entries: [
            {
              time: 15,
              request: {
                method: 'GET',
                url: `https://api.example.test${path}`,
                headers: [{ name: 'Authorization', value: 'Bearer hf_synthetic_drop_tok' }],
                queryString: [],
              },
              response: {
                status: 200,
                content: { size: 48, mimeType: 'application/json', text: '{"ok":true}' },
              },
            },
          ],
        },
      });

    const plainFile = new File([makeHar('/v1/catalog')], 'catalog.har', {
      type: 'application/json',
    });
    const gzBytes = await gzipText(makeHar('/v1/profile'));
    const gzFile = new File([gzBytes as BlobPart], 'profile.har.gz', {
      type: 'application/gzip',
    });

    const dropzone = screen.getByTestId('har-dropzone');
    fireEvent.drop(dropzone, {
      dataTransfer: { files: [plainFile, gzFile] },
    });

    await waitFor(() => {
      expect(screen.getByText(/Imported Session Sources \(4\)/)).toBeInTheDocument();
    });
    // Ensure raw synthetic token never renders unredacted
    expect(document.body.textContent).not.toContain('hf_synthetic_drop_tok');

    // Drop invalid HAR and verify alert banner
    const badFile = new File(['{not valid json'], 'broken.har', {
      type: 'application/json',
    });
    fireEvent.drop(dropzone, { dataTransfer: { files: [badFile] } });
    expect(await screen.findByRole('alert')).toHaveTextContent(/Invalid HAR JSON input/);
  });

  it('maintains 100% EN/RU dictionary key parity and accessible landmarks/themes', async () => {
    const enLeaves = collectDictionaryLeaves(DICTIONARIES.en);
    const ruLeaves = collectDictionaryLeaves(DICTIONARIES.ru);

    expect(enLeaves.map((x) => x.key)).toEqual(ruLeaves.map((x) => x.key));
    for (const item of [...enLeaves, ...ruLeaves]) {
      expect(item.value.trim().length).toBeGreaterThan(0);
    }

    const adapter = new InMemoryApplicationAdapter();
    render(<App adapter={adapter} />);
    expect(await screen.findByRole('banner')).toBeInTheDocument();
    expect(screen.getByRole('navigation', { name: 'Primary navigation' })).toBeInTheDocument();
    expect(screen.getByRole('main')).toBeInTheDocument();
    expect(screen.getByText('Skip to main content')).toHaveAttribute(
      'href',
      '#hf-main-content',
    );
  });
});
