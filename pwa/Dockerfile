FROM node:22-alpine AS deps
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci

FROM node:22-alpine AS builder
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY . .

ARG NEXT_PUBLIC_API_URL=http://localhost:8080
ENV NEXT_PUBLIC_API_URL=${NEXT_PUBLIC_API_URL}

ARG NEXT_PUBLIC_IS_CLOUD=false
ENV NEXT_PUBLIC_IS_CLOUD=${NEXT_PUBLIC_IS_CLOUD}

# Spinal-cord gateway base (PACT-20260424-001 Option B). Server-side env;
# read by next.config.ts rewrites() for /api/customer/:path* proxy to admin.
# Default unset = rewrite points at localhost:3201 (dev convenience).
# Venue: ADMIN_GATEWAY_URL=http://192.168.31.23:3201
# Cloud: ADMIN_GATEWAY_URL=https://admin.racingpoint.cloud
# To activate spinal-cord routing, ALSO set NEXT_PUBLIC_API_URL=/api/customer
# (flips all fetch() calls to go through the Next.js rewrite). Until both envs
# are set, PWA keeps legacy direct-RC behavior.
ARG ADMIN_GATEWAY_URL=http://localhost:3201
ENV ADMIN_GATEWAY_URL=${ADMIN_GATEWAY_URL}

RUN npm run build

FROM node:22-alpine AS runner
WORKDIR /app
ENV NODE_ENV=production
ENV PORT=3100

RUN addgroup --system --gid 1001 nodejs \
    && adduser --system --uid 1001 nextjs

COPY --from=builder /app/public ./public
COPY --from=builder --chown=nextjs:nodejs /app/.next/standalone ./
COPY --from=builder --chown=nextjs:nodejs /app/.next/static ./.next/static

USER nextjs
EXPOSE 3100

CMD ["node", "server.js"]
