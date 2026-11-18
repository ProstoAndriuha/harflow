import { describe, expect, it } from 'vitest';
import {
  ALL_TRAFFIC_CATEGORIES,
  type AppCommandName,
  type CleanerConfig,
  type SessionComparisonReport,
  type SessionLifecycleReport,
} from '../index.js';

describe('Shared types Phase 2/3 parity', () => {
  it('declares all 13 SRS M04 traffic categories', () => {
    expect(ALL_TRAFFIC_CATEGORIES).toHaveLength(13);
    expect(ALL_TRAFFIC_CATEGORIES).toContain('api');
    expect(ALL_TRAFFIC_CATEGORIES).toContain('authentication');
    expect(ALL_TRAFFIC_CATEGORIES).toContain('static_assets');
  });

  it('supports typed cleaner and session intelligence structures', () => {
    const cleaner: CleanerConfig = {
      keepCategories: ['api', 'authentication'],
      includedDomains: [],
      excludedDomains: ['ads.example.test'],
      keepRequestIds: [],
      excludeRequestIds: [],
      customRules: [],
    };
    const lifecycle: SessionLifecycleReport = {
      authFlows: [],
      loginRequestIds: ['req-1'],
      refreshRequestIds: ['req-2'],
      tokenRotations: [],
      expirationSignals: [],
      autoRefreshEligible: true,
    };
    const comparison: SessionComparisonReport = {
      sessionIds: ['sess-0001'],
      sharedEndpoints: [],
      sessionOnlyEndpoints: [],
      parameterVariability: [],
      corroboratedDependencyIds: [],
      singleSessionDependencyIds: [],
      authDifferences: [],
      schemaDrifts: [],
    };
    const cmd: AppCommandName = 'analysis.start';
    expect(cleaner.excludedDomains).toEqual(['ads.example.test']);
    expect(lifecycle.autoRefreshEligible).toBe(true);
    expect(comparison.sessionIds).toEqual(['sess-0001']);
    expect(cmd).toBe('analysis.start');
  });
});
