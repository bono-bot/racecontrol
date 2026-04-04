// k6 Pre-Deploy Load Test — Smart Pipes Tier 2
// Tests 3 critical endpoints under 50 concurrent users for 30 seconds
// PASS: error rate < 1%, p95 latency < 500ms
import http from 'k6/http';
import { check, sleep } from 'k6';

export const options = {
  vus: 50,
  duration: '30s',
  thresholds: {
    http_req_failed: ['rate<0.01'],       // <1% error rate
    http_req_duration: ['p(95)<500'],      // p95 < 500ms
  },
};

const BASE = 'http://192.168.31.23:8080';

export default function () {
  // 1. Health endpoint (lightweight)
  const health = http.get(`${BASE}/api/v1/health`);
  check(health, { 'health 200': (r) => r.status === 200 });

  // 2. Fleet health (heavier — reads all pod state)
  const fleet = http.get(`${BASE}/api/v1/fleet/health`);
  check(fleet, { 'fleet 200': (r) => r.status === 200 });

  // 3. Public pricing endpoint (DB read)
  const pricing = http.get(`${BASE}/api/v1/wallet/bonus-tiers`);
  check(pricing, { 'pricing 200': (r) => r.status === 200 });

  sleep(0.5);
}
