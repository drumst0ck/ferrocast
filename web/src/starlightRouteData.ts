import { defineRouteMiddleware } from '@astrojs/starlight/route-data';
import { hasEsTranslation } from './lib/docs/es-translations';

export const onRequest = defineRouteMiddleware(async (context, next) => {
  await next();
  const route = context.locals.starlightRoute;
  if (!route?.entry) return;

  const { isFallback, entry, head } = route;
  const relSlug = entry.slug.replace(/^(es\/)?docs\/?/, '');
  const esAvailable = hasEsTranslation(entry.slug);

  const canonical = head.find((t) => t.tag === 'link' && t.attrs?.rel === 'canonical')?.attrs?.href as
    | string
    | undefined;
  const enCanonical =
    canonical?.replace('/es/docs/', '/docs/') ??
    (entry.slug.startsWith('es/') ? undefined : canonical);

  const filtered = head.filter((item) => {
    if (item.tag !== 'link' || item.attrs?.rel !== 'alternate') return true;
    const hl = item.attrs.hreflang;
    if (hl === 'x-default') return false;
    if (isFallback) return false;
    if (hl === 'es' && !esAvailable) return false;
    return true;
  });

  if (enCanonical && !isFallback) {
    filtered.push({
      tag: 'link',
      attrs: { rel: 'alternate', hreflang: 'x-default', href: enCanonical },
    });
  }

  if (isFallback) {
    filtered.push({ tag: 'meta', attrs: { name: 'robots', content: 'noindex, follow' } });
  }

  route.head = filtered;
});
