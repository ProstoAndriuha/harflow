import { describe, expect, it } from 'vitest';
import { DESKTOP_BRIDGE_DESCRIPTOR } from '../index.js';

describe('Desktop bridge descriptor', () => {
  it('declares all 16 core application commands for Tauri v2', () => {
    expect(DESKTOP_BRIDGE_DESCRIPTOR.runtime).toBe('tauri-v2');
    expect(DESKTOP_BRIDGE_DESCRIPTOR.supportedCommands).toHaveLength(16);
    expect(DESKTOP_BRIDGE_DESCRIPTOR.supportedCommands).toContain('dependencies.confirm');
  });
});
