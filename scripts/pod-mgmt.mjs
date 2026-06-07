#!/usr/bin/env node
// pod-mgmt — venue-aware pod management helper (control_node).
//
// Resolves a venue from venue-registry.json to its pods + transport, and exposes
// uniform ops (list / resolve / exec). Transport per venue_type:
//   own  -> tailscale-ssh   (direct key-only SSH; fallback tailscale-rc-sentry)
//   sold -> heart-exec      (via the venue heart's audited pod_exec proxy)
// The rc-sentry / heart channels are RETAINED as audited automated channels +
// fallback (this is a COMPLEMENT to them, not a replacement).
//
// Pairs with the installer venue-type axis (RCA + PR #131). The resolution logic
// below is pure + unit-tested (pod-mgmt.test.mjs); the exec dispatch is
// credential-gated (SSH needs the pod key bootstrapped; rc-sentry/heart need keys).
//
// Usage:
//   pod-mgmt.mjs <venue> list
//   pod-mgmt.mjs <venue> resolve <pod>
//   pod-mgmt.mjs <venue> exec <pod> <command...>
// Registry path: $RP_VENUE_REGISTRY or ../.planning/specs/racecontrol-layer/venue-registry.json

import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

export const DEFAULT_REGISTRY = new URL(
  '../.planning/specs/racecontrol-layer/venue-registry.json',
  import.meta.url,
);

const RC_SENTRY_PORT = 8091;

/** Load + parse the registry JSON. Throws on missing/invalid file. */
export function loadRegistry(path = DEFAULT_REGISTRY) {
  const raw = fs.readFileSync(path, 'utf8');
  const reg = JSON.parse(raw);
  if (!reg || typeof reg.venues !== 'object') {
    throw new Error('registry has no "venues" object');
  }
  return reg;
}

/** Resolve a venue by id. Underscore-prefixed entries are templates/docs — not selectable. */
export function resolveVenue(reg, venueId) {
  if (!venueId) throw new Error('venue id required');
  if (venueId.startsWith('_')) throw new Error(`'${venueId}' is a template, not a selectable venue`);
  const v = reg.venues?.[venueId];
  if (!v) throw new Error(`unknown venue '${venueId}' (known: ${Object.keys(reg.venues).filter((k) => !k.startsWith('_')).join(', ') || 'none'})`);
  return v;
}

/** The primary pod transport for a venue. */
export function resolveTransport(venue) {
  const t = venue.pod_transport;
  if (!t) throw new Error('venue has no pod_transport');
  return t;
}

/** Build a concrete target descriptor for a pod under a given transport. */
function targetFor(venue, podId, transport) {
  const pod = venue.pods?.[podId];
  if (!pod) throw new Error(`unknown pod '${podId}' for venue (have: ${Object.keys(venue.pods || {}).join(', ') || 'none'})`);
  switch (transport) {
    case 'tailscale-ssh': {
      if (!pod.tailnet) throw new Error(`pod '${podId}' has no tailnet address for tailscale-ssh`);
      return { kind: 'ssh', transport, pod: podId, host: pod.tailnet, user: venue.pod_ssh_user || 'ADMIN' };
    }
    case 'tailscale-rc-sentry': {
      if (!pod.tailnet) throw new Error(`pod '${podId}' has no tailnet address for tailscale-rc-sentry`);
      return { kind: 'rc-sentry', transport, pod: podId, url: `http://${pod.tailnet}:${RC_SENTRY_PORT}` };
    }
    case 'heart-exec': {
      const base = venue.venue_heart?.url;
      if (!base) throw new Error('venue has no venue_heart.url for heart-exec');
      return { kind: 'heart', transport, pod: podId, url: `${base.replace(/\/+$/, '')}/pods/${podId}/exec` };
    }
    default:
      throw new Error(`unknown transport '${transport}'`);
  }
}

/** Resolve a pod under the venue's PRIMARY transport. */
export function resolvePodTarget(venue, podId) {
  return targetFor(venue, podId, resolveTransport(venue));
}

/** Resolve a pod under the venue's FALLBACK transport (or primary if none declared). */
export function resolveFallbackTarget(venue, podId) {
  return targetFor(venue, podId, venue.fallback_transport || resolveTransport(venue));
}

/** All pods resolved under the primary transport. */
export function listTargets(venue) {
  return Object.keys(venue.pods || {}).map((podId) => resolvePodTarget(venue, podId));
}

// ── CLI (credential-gated exec; list/resolve are read-only) ──────────────────
function execTarget(target, command) {
  if (target.kind === 'ssh') {
    return spawnSync('ssh', ['-o', 'BatchMode=yes', '-o', 'ConnectTimeout=8', `${target.user}@${target.host}`, command], { encoding: 'utf8' });
  }
  const body = JSON.stringify({ cmd: command, timeout_ms: 25000 });
  const url = target.kind === 'rc-sentry' ? `${target.url}/exec` : target.url;
  const key = process.env.RCSENTRY_SERVICE_KEY || '';
  return spawnSync('curl', ['-s', '--max-time', '35', '-X', 'POST', url, '-H', `X-Service-Key: ${key}`, '-H', 'Content-Type: application/json', '-d', body], { encoding: 'utf8' });
}

function main(argv) {
  const [venueId, op, ...rest] = argv;
  if (!venueId || !op) {
    console.error('usage: pod-mgmt.mjs <venue> <list|resolve|exec> [pod] [command...]');
    process.exit(2);
  }
  const reg = loadRegistry();
  const venue = resolveVenue(reg, venueId);
  if (op === 'list') {
    console.log(`venue=${venueId} type=${venue.venue_type} transport=${resolveTransport(venue)} fallback=${venue.fallback_transport || '(none)'}`);
    for (const t of listTargets(venue)) {
      console.log(`  ${t.pod.padEnd(7)} ${t.kind.padEnd(9)} ${t.host || t.url}`);
    }
  } else if (op === 'resolve') {
    console.log(JSON.stringify({ primary: resolvePodTarget(venue, rest[0]), fallback: resolveFallbackTarget(venue, rest[0]) }, null, 2));
  } else if (op === 'exec') {
    const [pod, ...cmd] = rest;
    const target = resolvePodTarget(venue, pod);
    const command = cmd.join(' ');
    console.error(`[exec] ${target.kind} -> ${target.host || target.url} : ${command}`);
    const r = execTarget(target, command);
    if (r.status !== 0) console.error(`[exec] exit=${r.status} stderr=${(r.stderr || '').trim().slice(0, 300)}`);
    process.stdout.write(r.stdout || '');
    process.exit(r.status ?? 1);
  } else {
    console.error(`unknown op '${op}'`);
    process.exit(2);
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === fs.realpathSync(process.argv[1])) {
  main(process.argv.slice(2));
}
