export const en = {
  appName: 'HARFlow Studio',
  offlineBadge: 'Offline-Only · Secrets Redacted',
  themeDark: 'Dark',
  themeLight: 'Light',
  nav: {
    dashboard: 'Dashboard',
    traffic: 'Traffic Explorer',
    dependencies: 'Dependency Graph',
    settings: 'Settings',
  },
  dashboard: {
    title: 'Project Dashboard',
    createDemo: 'Load Synthetic Demo Project',
    importHarLabel: 'Import Local HAR (.har / .json)',
    cards: {
      totalRequests: 'Total Requests',
      uniqueEndpoints: 'Unique Endpoints',
      apiDomains: 'API Domains',
      dependencies: 'Dependencies',
      authFlows: 'Auth Flows',
      unknownMappings: 'Unknown Mappings',
      exportReadiness: 'Export Readiness',
    },
    readiness: {
      not_ready: 'Not Ready',
      partial: 'Partial',
      ready: 'Ready',
    },
  },
  traffic: {
    title: 'Traffic Explorer',
    empty: 'No sanitized entries loaded yet.',
    detailsTitle: 'Sanitized Request Inspector',
    classificationReason: 'Classification Reason',
    sanitizedHeaders: 'Sanitized Headers',
    columns: {
      method: 'Method',
      status: 'Status',
      endpoint: 'Endpoint',
      domain: 'Domain',
      duration: 'Duration',
      size: 'Size',
      type: 'Type',
      session: 'Session',
    },
  },
  dependencies: {
    title: 'Dependency Graph & Evidence Inspector',
    confirm: 'Confirm',
    reject: 'Reject',
    confidence: 'Confidence',
    evidence: 'Evidence',
    empty: 'No dependencies discovered yet.',
  },
  settings: {
    title: 'Settings',
    appearance: 'Appearance',
    language: 'Interface Language',
    privacyTitle: 'Privacy & Offline Enforcement',
    privacyBody:
      'All HAR parsing and secret redaction run locally in memory or the Rust core. Raw Authorization, Cookie, Set-Cookie, and API key values are stripped before persistence.',
    adapterMode: 'Application Adapter Mode',
  },
};

export type TranslationDict = typeof en;
