import { defineConfig } from 'wxt';

export default defineConfig({
  manifest: {
    name: 'Appz Toolbar',
    version: '0.1.0',
    description: 'Developer toolbar for Appz deployments',
    permissions: ['storage', 'alarms', 'scripting'],
    host_permissions: [
      'https://appz.dev/*',
      'https://*.appz.dev/*',
      'https://api.appz.dev/*',
      'https://localhost/*',
    ],
    minimum_chrome_version: '88.0',
  },
});
