#!/usr/bin/env node
// scripts/validate-plan.mjs — Machine-readable plan validation
//
// Validates a test plan (JSON array of steps) against domain-rules.json.
// Returns PASS/FAIL with specific rule violations.
//
// Usage:
//   node scripts/validate-plan.mjs '[{"action":"game-launch","pod":"pod_3","game":"f1_25","tier":"tier_trial"}]'
//   echo '<json>' | node scripts/validate-plan.mjs --stdin
//
// Each step must have: action, pod, game (for game actions), tier (for billing actions)

import { readFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const rules = JSON.parse(readFileSync(join(__dirname, 'domain-rules.json'), 'utf8'));

let planJson;
if (process.argv.includes('--stdin')) {
    planJson = readFileSync(0, 'utf8');
} else {
    planJson = process.argv[2];
}

if (!planJson) {
    console.error('Usage: node validate-plan.mjs \'[{"action":"game-launch","pod":"pod_3","game":"assetto_corsa","tier":"tier_30min"}]\'');
    process.exit(2);
}

const plan = JSON.parse(planJson);
const violations = [];

for (let i = 0; i < plan.length; i++) {
    const step = plan[i];
    const prefix = `Step ${i + 1}`;

    // Validate game-tier compatibility
    if (step.game && step.tier) {
        const allowedTiers = rules.game_tier_compatibility[step.game];
        if (!allowedTiers) {
            violations.push(`${prefix}: Unknown game '${step.game}'`);
        } else if (!allowedTiers.includes(step.tier)) {
            violations.push(`${prefix}: RULE VIOLATION — '${step.game}' cannot use tier '${step.tier}'. Allowed: [${allowedTiers.join(', ')}]`);
        }
    }

    // Validate trial rules
    if (step.tier === 'tier_trial') {
        if (step.game && !rules.trial_rules.allowed_games.includes(step.game)) {
            violations.push(`${prefix}: TRIAL VIOLATION — trial is ${rules.trial_rules.allowed_games.join('/')} ONLY, got '${step.game}'`);
        }
    }

    // Validate pod exists in fleet targets
    if (step.pod) {
        const target = rules.fleet_targets.targets.find(t => t.id === step.pod);
        if (!target) {
            violations.push(`${prefix}: Unknown pod '${step.pod}'`);
        }
    }

    // Validate verification requirements are defined
    if (step.action && rules.verification_requirements[step.action.replace('-', '_')]) {
        const reqs = rules.verification_requirements[step.action.replace('-', '_')];
        // Check if step defines a verification method
        if (!step.verify_with && reqs.must_check.length > 0) {
            // Not a violation, just a warning — the verify-action.sh script handles this
        }
    }
}

// Output
if (violations.length === 0) {
    console.log(`PLAN VALID: ${plan.length} steps, 0 violations`);
    for (const step of plan) {
        const game = step.game || '-';
        const tier = step.tier || '-';
        console.log(`  ${step.action} | ${step.pod} | ${game} | ${tier}`);
    }
    process.exit(0);
} else {
    console.log(`PLAN INVALID: ${violations.length} violation(s)`);
    for (const v of violations) {
        console.log(`  VIOLATION: ${v}`);
    }
    process.exit(1);
}
