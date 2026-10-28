import { defineRouteMiddleware } from '@astrojs/starlight/route-data';
import { rewriteStarlightHead } from './lib/docs/fallback-head';
import { hasEsTranslation } from './lib/docs/es-translations';

export const onRequest = defineRouteMiddleware(async (context, next) => {
  await next();
  const route = context.locals.starlightRoute;
  if (!route?.entry) return;

  const { isFallback, entry, head } = route;
  const esAvailable = hasEsTranslation(entry.slug);

  const canonical = head.find((t) => t.tag === 'link' && t.attrs?.rel === 'canonical')?.attrs?.href as
    | string
    | undefined;
  const enCanonical =
    canonical?.replace('/es/docs/', '/docs/') ??
    (entry.slug.startsWith('es/') ? undefined : canonical);

  route.head = rewriteStarlightHead(head, { isFallback, esAvailable, enCanonical });
});
