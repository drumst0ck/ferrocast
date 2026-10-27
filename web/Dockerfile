FROM node:22-alpine AS build
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM node:22-alpine AS deps
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci --omit=dev

FROM node:22-alpine AS run
WORKDIR /app
RUN apk add --no-cache curl
ENV NODE_ENV=production HOST=0.0.0.0 PORT=4321 ASTRO_NODE_AUTOSTART=disabled
COPY --from=deps /app/node_modules ./node_modules
COPY --from=build /app/dist ./dist
COPY drizzle ./drizzle
COPY scripts/migrate.mjs ./scripts/migrate.mjs
COPY scripts/csp-report-handle.mjs ./scripts/csp-report-handle.mjs
COPY scripts/csp-report-store.mjs ./scripts/csp-report-store.mjs
COPY scripts/production-server.mjs ./scripts/production-server.mjs
COPY package.json ./
USER node
EXPOSE 4321
HEALTHCHECK --interval=30s --timeout=3s CMD curl -fsS http://127.0.0.1:4321/healthz/ || exit 1
# Migrations are idempotent and run on every start; if they fail the site still starts (the changelog shows its fallback).
CMD ["sh", "-c", "if [ -n \"$DATABASE_URL\" ]; then node scripts/migrate.mjs || echo 'migrations failed, starting anyway'; fi; exec node scripts/production-server.mjs"]
