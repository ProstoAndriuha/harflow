import '@testing-library/jest-dom/vitest';
import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { GraphKind } from '@harflow/shared-types';
import { useHarflowStore } from '../hooks/useHarflowStore.js';
import { InMemoryApplicationAdapter } from '../services/applicationAdapter.js';

describe('Phase 4 Store, Undo/Redo & InMemoryApplicationAdapter Parity', () => {
  it('computes Phase 2/3 cleaner, schemas, all 5 graph kinds, and session intelligence', async () => {
    const adapter = new InMemoryApplicationAdapter();
    const res = await adapter.invoke('analysis.results', {
      projectId: 'proj-demo-0001',
    });

    expect(res.cleanerPreview?.totalEntries).toBe(10);
    expect(res.cleanerPreview?.keptEntries).toBe(9);
    expect(res.cleanerPreview?.excludedEntries).toBe(1);
    expect(res.entries.some((e) => e.category === 'analytics' && e.kept === false)).toBe(
      true,
    );

    expect(res.model.schemas.length).toBeGreaterThanOrEqual(4);
    expect(
      res.model.endpoints.some((e) => e.pagination?.style === 'cursor'),
    ).toBe(true);

    expect(res.sessionLifecycle?.tokenRotations.length).toBeGreaterThanOrEqual(1);
    expect(res.sessionLifecycle?.expirationSignals.length).toBeGreaterThanOrEqual(1);
    expect(res.sessionLifecycle?.autoRefreshEligible).toBe(true);
    expect(res.sessionComparison?.schemaDrifts.length).toBeGreaterThanOrEqual(1);
    expect(res.sessionComparison?.parameterVariability.length).toBeGreaterThanOrEqual(1);

    const kinds: GraphKind[] = [
      'endpoint',
      'request_dependency',
      'auth_flow',
      'session_timeline',
      'data_flow',
    ];
    for (const kind of kinds) {
      const g = await adapter.invoke('graph.get', {
        projectId: res.manifest.id,
        kind,
      });
      expect(g.kind).toBe(kind);
      expect(g.nodes.length).toBeGreaterThan(0);
    }

    const svg = await adapter.invoke('export.create', {
      projectId: res.manifest.id,
      format: 'graph_svg',
      graphKind: 'endpoint',
    });
    expect(svg.content).toContain('<svg');
  });

  it('supports undo/redo across dependency edits, cleaner actions, and endpoint overrides', async () => {
    const adapter = new InMemoryApplicationAdapter();
    const { result } = renderHook(() => useHarflowStore(adapter));

    await waitFor(() => {
      expect(result.current.state.loading).toBe(false);
    });

    const initialDep = result.current.state.model?.dependencies.find(
      (d) => d.id === 'dep-0002',
    );
    expect(initialDep?.status).toBe('likely');

    await act(async () => {
      await result.current.editDependencyMapping('dep-0002', {
        status: 'user_confirmed',
        sourcePath: '$.data.orderId',
      });
    });
    expect(result.current.canUndoAction).toBe(true);
    expect(
      result.current.state.model?.dependencies.find((d) => d.id === 'dep-0002')?.source
        .jsonPathOrKey,
    ).toBe('$.data.orderId');

    await act(async () => {
      await result.current.undo();
    });
    expect(
      result.current.state.model?.dependencies.find((d) => d.id === 'dep-0002')?.status,
    ).toBe('likely');
    expect(
      result.current.state.model?.dependencies.find((d) => d.id === 'dep-0002')?.source
        .jsonPathOrKey,
    ).toBe('$.orderId');
    expect(result.current.canRedoAction).toBe(true);

    await act(async () => {
      await result.current.redo();
    });
    expect(
      result.current.state.model?.dependencies.find((d) => d.id === 'dep-0002')?.status,
    ).toBe('user_confirmed');

    const beforeKept = result.current.state.cleanerPreview?.keptEntries ?? 0;
    await act(async () => {
      await result.current.updateCleaner({ restoreRequest: 'sess-0001-req-0005' });
    });
    expect(result.current.state.cleanerPreview?.keptEntries).toBe(beforeKept + 1);

    await act(async () => {
      await result.current.undo();
    });
    expect(result.current.state.cleanerPreview?.keptEntries).toBe(beforeKept);

    await act(async () => {
      await result.current.setEndpointOverride('ep-0003', '/v1/orders/{customOrderId}');
    });
    expect(
      result.current.state.model?.endpoints.find((e) => e.id === 'ep-0003')?.pathTemplate,
    ).toBe('/v1/orders/{customOrderId}');

    await act(async () => {
      await result.current.undo();
    });
    expect(
      result.current.state.model?.endpoints.find((e) => e.id === 'ep-0003')?.pathTemplate,
    ).toBe('/v1/orders/{orderId}');
  });

  it('handles camelCase auth keys, format-suffixed paths, terminal slugs, skip pagination, and hypermedia URLs', async () => {
    const adapter = new InMemoryApplicationAdapter();
    await adapter.invoke('project.create', { name: 'Real Parity' });
    const har = JSON.stringify({
      log: {
        version: '1.2',
        entries: [
          {
            time: 10,
            request: {
              method: 'POST',
              url: 'https://api.real.test/auth/login',
              headers: [{ name: 'Content-Type', value: 'application/json' }],
              queryString: [],
              postData: {
                mimeType: 'application/json',
                text: JSON.stringify({ username: 'demo', password: 'hf_synthetic_login_pw' }),
              },
            },
            response: {
              status: 200,
              content: {
                mimeType: 'application/json',
                text: JSON.stringify({
                  accessToken: 'hf_synthetic_opaque_access_1',
                  refreshToken: 'hf_synthetic_opaque_refresh_1',
                }),
              },
            },
          },
          {
            time: 10,
            request: {
              method: 'GET',
              url: 'https://api.real.test/products?limit=5&skip=5',
              headers: [{ name: 'Authorization', value: 'Bearer hf_synthetic_opaque_access_1' }],
              queryString: [
                { name: 'limit', value: '5' },
                { name: 'skip', value: '5' },
              ],
            },
            response: {
              status: 200,
              content: {
                mimeType: 'application/json',
                text: JSON.stringify({
                  items: [50538240],
                  evolution_chain: { url: 'https://api.real.test/api/v2/evolution-chain/10/' },
                }),
              },
            },
          },
          {
            time: 10,
            request: {
              method: 'GET',
              url: 'https://api.real.test/v0/item/50538240.json',
              headers: [],
              queryString: [],
            },
            response: {
              status: 200,
              content: { mimeType: 'application/json', text: '{"id":50538240}' },
            },
          },
          {
            time: 10,
            request: {
              method: 'GET',
              url: 'https://api.real.test/api/v2/evolution-chain/10/',
              headers: [],
              queryString: [],
            },
            response: {
              status: 200,
              content: { mimeType: 'application/json', text: '{"id":10}' },
            },
          },
          {
            time: 10,
            request: { method: 'GET', url: 'https://api.real.test/api/v2/pokemon/pikachu', headers: [], queryString: [] },
            response: { status: 200, content: { mimeType: 'application/json', text: '{"name":"pikachu"}' } },
          },
          {
            time: 10,
            request: { method: 'GET', url: 'https://api.real.test/api/v2/pokemon/charizard', headers: [], queryString: [] },
            response: { status: 200, content: { mimeType: 'application/json', text: '{"name":"charizard"}' } },
          },
          {
            time: 10,
            request: { method: 'GET', url: 'https://api.real.test/api/v2/pokemon/bulbasaur', headers: [], queryString: [] },
            response: { status: 200, content: { mimeType: 'application/json', text: '{"name":"bulbasaur"}' } },
          },
        ],
      },
    });

    const imported = await adapter.invoke('har.import', {
      projectId: 'proj-demo-0001',
      sourceName: 'real.har',
      rawText: har,
    });
    const analysis = await adapter.invoke('analysis.results', {
      projectId: imported.manifest.id,
    });

    const serialized = JSON.stringify(analysis);
    expect(serialized).not.toContain('hf_synthetic_opaque_access_1');
    expect(serialized).not.toContain('hf_synthetic_opaque_refresh_1');
    expect(serialized).not.toContain('hf_synthetic_login_pw');

    expect(
      analysis.model.endpoints.some((e) => e.pathTemplate === '/v0/item/{itemId}.json'),
    ).toBe(true);
    expect(
      analysis.model.endpoints.some((e) => e.pathTemplate === '/api/v2/pokemon/{pokemonId}'),
    ).toBe(true);
    expect(
      analysis.model.endpoints.find((e) => e.pathTemplate === '/products')?.pagination?.style,
    ).toBe('offset_limit');
    expect(
      analysis.model.dependencies.some(
        (d) =>
          d.source.jsonPathOrKey === '$.accessToken' &&
          d.target.jsonPathOrKey === 'Authorization',
      ),
    ).toBe(true);
    expect(
      analysis.model.dependencies.some(
        (d) =>
          d.source.jsonPathOrKey === '$.items[0]' &&
          d.target.jsonPathOrKey === 'itemId',
      ),
    ).toBe(true);
    expect(
      analysis.model.dependencies.some(
        (d) =>
          d.source.jsonPathOrKey === '$.evolution_chain.url' &&
          d.target.jsonPathOrKey === 'evolutionChainId',
      ),
    ).toBe(true);
  });
});
