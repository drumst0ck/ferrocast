import { defineMiddleware } from 'astro:middleware';

// Canonical host: www.ferrocast.app -> ferrocast.app (301). Only applies to on-demand responses.
export const onRequest = defineMiddleware((context, next) => {
  if (context.isPrerendered) return next();
  const host = context.request.headers.get('host') ?? '';
  if (host.startsWith('www.')) {
    const url = new URL(context.request.url);
    url.host = host.slice(4);
    url.protocol = 'https:';
    return Response.redirect(url.href, 301);
  }
  return next();
});
