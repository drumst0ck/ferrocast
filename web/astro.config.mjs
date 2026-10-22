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
      customCss: ['./src/styles/fonts.css', './src/styles/starlight.css'],
      components: {
        Header: './src/components/docs/Header.astro',
        SkipLink: './src/components/docs/SkipLink.astro',
        PageTitle: './src/components/docs/PageTitle.astro',
      },
      head: [
        {
          tag: 'script',
          content: `(function(w,d,s,l,i){w[l]=w[l]||[];w[l].push({'gtm.start':new Date().getTime(),event:'gtm.js'});var f=d.getElementsByTagName(s)[0],j=d.createElement(s),dl=l!='dataLayer'?'&l='+l:'';j.async=true;j.src='https://www.googletagmanager.com/gtm.js?id='+i+dl;f.parentNode.insertBefore(j,f);})(window,document,'script','dataLayer','GTM-KC23GXFQ');`,
        },
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
