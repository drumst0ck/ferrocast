import { defineConfig } from 'astro/config';
import node from '@astrojs/node';
import sitemap from '@astrojs/sitemap';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://ferrocast.app',
  base: '/',
  output: 'static',
  adapter: node({ mode: 'standalone' }),
  trailingSlash: 'always',
  integrations: [
    starlight({
      title: 'Ferrocast Docs',
      description: 'Documentation for Ferrocast, a fast keyboard launcher and local file converter for macOS, written in Rust.',
      logo: { src: './public/icon.png', alt: '' },
      favicon: '/favicon.png',
      disable404Route: true, // the site 404 page (src/pages/404.astro) is used
      defaultLocale: 'root',
      locales: {
        root: { label: 'English', lang: 'en' },
        es: { label: 'Español', lang: 'es' },
      },
      social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/drumst0ck/ferrocast' }],
      editLink: { baseUrl: 'https://github.com/drumst0ck/ferrocast/edit/master/web/src/content/docs/' },
      customCss: ['./src/styles/starlight.css'],
      components: {
        Header: './src/components/docs/Header.astro',
        PageTitle: './src/components/docs/PageTitle.astro',
      },
      head: [
        { tag: 'link', attrs: { rel: 'preconnect', href: 'https://fonts.googleapis.com' } },
        { tag: 'link', attrs: { rel: 'preconnect', href: 'https://fonts.gstatic.com', crossorigin: '' } },
        { tag: 'link', attrs: { rel: 'stylesheet', href: 'https://fonts.googleapis.com/css2?family=Stack+Sans+Headline:wght@300&family=Stack+Sans+Text:wght@400;500&display=swap' } },
        { tag: 'meta', attrs: { property: 'og:image', content: 'https://ferrocast.app/og-docs.jpg' } },
        { tag: 'meta', attrs: { name: 'twitter:card', content: 'summary_large_image' } },
      ],
      sidebar: [
        { label: 'Start here', slug: 'docs' },
        { label: 'Getting started', translations: { es: 'Primeros pasos' }, autogenerate: { directory: 'docs/getting-started' } },
        { label: 'Launcher', translations: { es: 'Lanzador' }, autogenerate: { directory: 'docs/launcher' } },
        { label: 'Files', translations: { es: 'Archivos' }, autogenerate: { directory: 'docs/files' } },
        { label: 'Reference', translations: { es: 'Referencia' }, autogenerate: { directory: 'docs/reference' } },
        { label: 'Contributing', translations: { es: 'Contribuir' }, autogenerate: { directory: 'docs/contributing' } },
      ],
    }),
    // Starlight does not add its own sitemap; this is the only sitemap integration.
    sitemap(),
  ],
  devToolbar: { enabled: false },
});
