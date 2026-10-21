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
ENV NODE_ENV=production HOST=0.0.0.0 PORT=4321
COPY --from=deps /app/node_modules ./node_modules
COPY --from=build /app/dist ./dist
COPY drizzle ./drizzle
COPY scripts/migrate.mjs ./scripts/migrate.mjs
COPY package.json ./
USER node
EXPOSE 4321
HEALTHCHECK --interval=30s --timeout=3s CMD wget -qO- http://127.0.0.1:4321/healthz/ || exit 1
CMD ["node", "./dist/server/entry.mjs"]
