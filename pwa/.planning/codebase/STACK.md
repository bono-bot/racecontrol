# Technology Stack

**Analysis Date:** 2026-03-21

## Languages

**Primary:**
- TypeScript 5.x - Full codebase (src/)
- JavaScript - Build and config files
- CSS - Styling via Tailwind CSS

**Secondary:**
- Bash - Dockerfile (Alpine Linux base)

## Runtime

**Environment:**
- Node.js 22-alpine (containerized via Docker)

**Package Manager:**
- npm 10.x (inferred from Node 22)
- Lockfile: `package-lock.json` present (committed)

## Frameworks

**Core:**
- Next.js 16.1.6 - Full-stack React framework with App Router
  - Output: standalone (Docker-optimized)
  - Port: 3100 (dev and production)

**UI & Components:**
- React 19.2.3 - UI library
- React DOM 19.2.3 - DOM rendering

**Styling:**
- Tailwind CSS 4 - Utility-first CSS framework
- @tailwindcss/postcss 4 - PostCSS plugin for Tailwind
- PostCSS - CSS transformation (via `postcss.config.mjs`)

**Charts & Visualization:**
- Recharts 3.8.0 - React charting library for telemetry graphs

**Notifications:**
- Sonner 2.0.7 - Toast notification system

**Animation:**
- canvas-confetti 1.9.4 - Confetti animation (celebration effects)

**QR Code:**
- html5-qrcode 2.3.8 - QR code scanning for pod check-in

## Key Dependencies

**Critical:**
- Next.js 16.1.6 - Framework (enables SSR, API routes, optimizations)
- React 19.2.3 - Core UI runtime (required by Next.js)

**Type Definitions:**
- @types/react 19 - React type definitions
- @types/react-dom 19 - React DOM type definitions
- @types/node 20 - Node.js type definitions
- @types/canvas-confetti 1.9.0 - Confetti library types

## Configuration

**Environment:**
- `NEXT_PUBLIC_API_URL` - Racing Point server API endpoint (default: `http://localhost:8080/api/v1`)
- `NEXT_PUBLIC_GATEWAY_URL` - Payment gateway endpoint (default: `/api/payments`)
- `PORT` - Server port in production (default: 3100)
- `NODE_ENV` - Environment mode (development/production)

**Build:**
- `tsconfig.json` - TypeScript compiler configuration
  - Target: ES2017
  - Module: ESNext
  - Strict mode enabled
  - Path alias: `@/*` → `./src/*`
- `next.config.ts` - Next.js build configuration
  - Output mode: standalone (no node_modules in output)
- `postcss.config.mjs` - PostCSS configuration for Tailwind
- `.dockerignore` - Docker build exclusions

## Platform Requirements

**Development:**
- Node.js 22+ (for local `npm run dev`)
- npm 10+
- TypeScript 5.x (for type checking)
- Tailwind CSS 4 (for PostCSS)

**Production:**
- Docker with Node.js 22-alpine base image
- Network access to Racing Point server at `NEXT_PUBLIC_API_URL`
- Network access to payment gateway at `NEXT_PUBLIC_GATEWAY_URL`
- Memory: ~150-200MB (lightweight Next.js standalone)
- Port: 3100 (configurable via `PORT` env var)

## Build & Dev Scripts

Located in `package.json`:
- `npm run dev` - Start dev server on port 3100 with hot reload
- `npm run build` - Build standalone production bundle
- `npm start` - Start production server on port 3100
- `npm run lint` - Run ESLint (linting setup not shown in package.json, likely in `.eslintrc`)

## Docker Configuration

**Image:** `node:22-alpine` (minimal, ~165MB base)

**Stages:**
1. `deps` - Install dependencies from lock file
2. `builder` - Build Next.js app (multi-stage optimization)
3. `runner` - Production container with only necessary files
   - User: `nextjs:nodejs` (non-root for security)
   - Working directory: `/app`
   - Expose: port 3100
   - Entrypoint: `node server.js` (Next.js standalone server)

**Build-time env:**
- `NEXT_PUBLIC_API_URL` - Passed at Docker build (ARG with default `http://localhost:8080`)

## Type System

**TypeScript Config Highlights:**
- Strict: true (no implicit any, strict null checks)
- JSX: react-jsx (new JSX transform)
- Module resolution: bundler (Next.js optimized)
- Incremental: true (faster rebuilds)
- Path aliases enabled for cleaner imports (`@/lib`, `@/components`)

---

*Stack analysis: 2026-03-21*
