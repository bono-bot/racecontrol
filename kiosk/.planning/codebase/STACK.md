# Technology Stack

**Analysis Date:** 2026-03-21

## Languages

**Primary:**
- TypeScript 5.9.3 - All source code (React components, page routes, utilities)
- CSS 4 - Global styles via Tailwind CSS v4
- HTML 5 - via Next.js JSX/TSX templating

**Secondary:**
- JavaScript (Node.js) - Build tooling and dev server

## Runtime

**Environment:**
- Node.js (version not pinned, uses system default)
- Next.js 16.1.6 - Full-stack React framework with server/client components

**Package Manager:**
- npm (version inferred from lock file)
- Lockfile: `package-lock.json` (56786 bytes, present)

## Frameworks

**Core:**
- Next.js 16.1.6 - Server-side rendering, API routes, file-based routing
- React 19.2.3 - UI components and hooks
- React DOM 19.2.3 - DOM rendering for React components

**Styling:**
- Tailwind CSS v4 - Utility-first CSS framework
- PostCSS 4 - CSS processing pipeline (via @tailwindcss/postcss)

**Build/Dev:**
- TypeScript 5.9.3 - Type safety and compilation
- Next.js built-in ESLint integration - Code linting

## Key Dependencies

**Critical:**
- next - 16.1.6 - Full-stack React framework with SSR, incremental static regeneration, API routes
- react - 19.2.3 - UI component library
- react-dom - 19.2.3 - React renderer for browser
- tailwindcss - 4 - Utility CSS framework (essential for styling)
- @tailwindcss/postcss - 4 - PostCSS integration for Tailwind

**Infrastructure:**
- @types/node - 22 - TypeScript definitions for Node.js APIs
- @types/react - 19 - TypeScript definitions for React 19
- @types/react-dom - 19 - TypeScript definitions for React DOM 19

## Configuration

**Environment:**
- API endpoints: Configurable via `NEXT_PUBLIC_API_URL` (default: `window.location` origin)
- WebSocket endpoint: Configurable via `NEXT_PUBLIC_WS_URL` (default: `ws://{hostname}:8080/ws/dashboard`)
- Build target: Standalone (`output: "standalone"` in next.config.ts)
- Base path: `/kiosk` (all routes prefixed)

**Build:**
- `next.config.ts` - Next.js configuration (standalone output, base path redirect)
- `tsconfig.json` - TypeScript compiler options (ES2017 target, strict mode, path aliases)
- `postcss.config.mjs` - PostCSS configuration for Tailwind
- `.gitignore` - Standard Node.js/Next.js exclusions

## Platform Requirements

**Development:**
- Windows 11 Pro (current dev machine: RTX 4070, Rust environment optional)
- Node.js + npm (no pinned version in package.json)
- TypeScript 5.9.3 support
- Terminal/IDE with bash support (Git Bash recommended for cross-platform)

**Production:**
- Deployment target: Racing Point Server (192.168.31.23)
- Port: 3300 (configured in package.json scripts)
- Network: Must reach racecontrol API at http://192.168.31.23:8080
- Must reach WebSocket at ws://192.168.31.23:8080/ws/dashboard
- Browser: Modern Chrome/Edge with WebSocket support (Electron Edge kiosk)

## Scripts

**Development:**
```bash
npm run dev        # Start Next.js dev server on port 3300, bind to 0.0.0.0
npm run build      # Build Next.js app for production
npm run start      # Start production server on port 3300
npm run lint       # Run ESLint on codebase
```

**Deployment:**
- Kiosk service runs on server .23 via Windows scheduled task
- Standalone output enables binary deployment without Node.js on server
- Base path `/kiosk` requires reverse proxy or server routing to `/kiosk`

---

*Stack analysis: 2026-03-21*
