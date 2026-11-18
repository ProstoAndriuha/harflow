import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { isValidApiModel } from '../index.js';

const here = dirname(fileURLToPath(import.meta.url));
const fixturePath = resolve(
  here,
  '../../../../fixtures/expected-output/minimal-api-model.json',
);

describe('APIModel runtime contract', () => {
  it('validates minimal-api-model.json fixture', () => {
    const parsed: unknown = JSON.parse(readFileSync(fixturePath, 'utf8'));
    expect(isValidApiModel(parsed)).toBe(true);
  });

  it('rejects malformed objects missing schemaVersion', () => {
    expect(isValidApiModel({ projectId: 'x' })).toBe(false);
    expect(isValidApiModel(null)).toBe(false);
  });
});
