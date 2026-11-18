import type { AppCommandName } from '@harflow/shared-types';

export interface DesktopBridgeDescriptor {
  readonly runtime: 'tauri-v2';
  readonly phase: 'phase-0-scaffold';
  readonly supportedCommands: readonly AppCommandName[];
}

export const DESKTOP_BRIDGE_DESCRIPTOR: DesktopBridgeDescriptor = {
  runtime: 'tauri-v2',
  phase: 'phase-0-scaffold',
  supportedCommands: [
    'project.create',
    'project.open',
    'project.save',
    'har.import',
    'analysis.start',
    'analysis.cancel',
    'analysis.results',
    'endpoints.list',
    'dependencies.list',
    'dependencies.confirm',
    'dependencies.reject',
    'graph.get',
    'sdk.generate',
    'sdk.preview',
    'validation.mock',
    'export.create',
  ],
};
