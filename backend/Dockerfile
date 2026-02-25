# ── Build stage ──────────────────────────────────────────────────────────────
FROM node:20-alpine AS deps
WORKDIR /app
COPY package.json package-lock.json ./
# Install production dependencies only.
RUN npm ci --omit=dev

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM node:20-alpine AS runtime
WORKDIR /app

# Copy production node_modules from the build stage.
COPY --from=deps /app/node_modules ./node_modules

# Copy application source and the OpenAPI spec it references.
COPY server.js ./
COPY ../docs/static/openapi.json ./docs/static/openapi.json

ENV NODE_ENV=production
ENV PORT=3000

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
  CMD wget -qO- http://localhost:3000/health || exit 1

CMD ["node", "server.js"]
