import { defineConfig } from 'astro/config';
import node from '@astrojs/node';
import sitemap from '@astrojs/sitemap';

export default defineConfig({
  site: 'https://ferrocast.app',
  base: '/',
  output: 'static',
  adapter: node({ mode: 'standalone' }),
  trailingSlash: 'always',
  integrations: [sitemap()],
  devToolbar: { enabled: false },
});
