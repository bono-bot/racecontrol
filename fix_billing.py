import re

with open('C:/Users/bono/racingpoint/racecontrol/crates/racecontrol/src/billing.rs', 'r', encoding='utf-8') as f:
    content = f.read()

changes = []

# 1. Add sim_type field to BillingRateTier
marker1 = '    pub rate_per_min_paise: i64,\n}\n\n/// Default billing rate tiers'
if marker1 in content and 'pub sim_type: Option<rc_common::types::SimType>,' not in content[content.find('pub struct BillingRateTier'):content.find('pub struct BillingRateTier')+400]:
    content = content.replace(marker1,
        '    pub rate_per_min_paise: i64,\n    /// None = universal rate. Some(SimType) = game-specific.\n    pub sim_type: Option<rc_common::types::SimType>,\n}\n\n/// Default billing rate tiers',
        1)
    changes.append("BillingRateTier::sim_type field added")
else:
    changes.append("BillingRateTier::sim_type already present or marker not found")

# 2. Update default_billing_rate_tiers()
old_d = '        BillingRateTier { tier_order: 1, tier_name: "Standard".into(), threshold_minutes: 30, rate_per_min_paise: 2500 },'
if old_d in content:
    content = content.replace(
        '        BillingRateTier { tier_order: 1, tier_name: "Standard".into(), threshold_minutes: 30, rate_per_min_paise: 2500 },\n        BillingRateTier { tier_order: 2, tier_name: "Extended".into(), threshold_minutes: 60, rate_per_min_paise: 2000 },\n        BillingRateTier { tier_order: 3, tier_name: "Marathon".into(), threshold_minutes: 0, rate_per_min_paise: 1500 },',
        '        BillingRateTier { tier_order: 1, tier_name: "Standard".into(), threshold_minutes: 30, rate_per_min_paise: 2500, sim_type: None },\n        BillingRateTier { tier_order: 2, tier_name: "Extended".into(), threshold_minutes: 60, rate_per_min_paise: 2000, sim_type: None },\n        BillingRateTier { tier_order: 3, tier_name: "Marathon".into(), threshold_minutes: 0, rate_per_min_paise: 1500, sim_type: None },',
        1)
    changes.append("default_billing_rate_tiers: sim_type: None added")
else:
    changes.append("default_billing_rate_tiers: already has sim_type or not found")

# 3. Update refresh_rate_tiers()
old_r1 = 'let rows = sqlx::query_as::<_, (i64, String, i64, i64)>(\n        "SELECT tier_order, tier_name, threshold_minutes, rate_per_min_paise\n         FROM billing_rates WHERE is_active = 1 ORDER BY tier_order ASC",'
if old_r1 in content:
    content = content.replace(
        'let rows = sqlx::query_as::<_, (i64, String, i64, i64)>(\n        "SELECT tier_order, tier_name, threshold_minutes, rate_per_min_paise\n         FROM billing_rates WHERE is_active = 1 ORDER BY tier_order ASC",',
        'let rows = sqlx::query_as::<_, (i64, String, i64, i64, Option<String>)>(\n        "SELECT tier_order, tier_name, threshold_minutes, rate_per_min_paise, sim_type\n         FROM billing_rates WHERE is_active = 1 ORDER BY tier_order ASC",',
        1)
    changes.append("refresh_rate_tiers: query updated")
else:
    changes.append("refresh_rate_tiers query: already updated or not found")

old_r2 = '.map(|(order, name, thresh, rate)| BillingRateTier {\n                    tier_order: order as u32,\n                    tier_name: name,\n                    threshold_minutes: thresh as u32,\n                    rate_per_min_paise: rate,\n                })'
if old_r2 in content:
    content = content.replace(old_r2,
        '.map(|(order, name, thresh, rate, sim_str)| {\n                    let sim_type = sim_str.as_deref().and_then(|s| serde_json::from_value(serde_json::Value::String(s.to_string())).ok());\n                    BillingRateTier {\n                        tier_order: order as u32,\n                        tier_name: name,\n                        threshold_minutes: thresh as u32,\n                        rate_per_min_paise: rate,\n                        sim_type,\n                    }\n                })',
        1)
    changes.append("refresh_rate_tiers: map closure updated")
else:
    changes.append("refresh_rate_tiers map: already updated or not found")

# 4. Add get_tiers_for_game before BillingTimer section
btimer_marker = '// ─── BillingTimer ───────────────────────────────────────────────────────────'
if btimer_marker in content and 'fn get_tiers_for_game' not in content:
    get_tiers_fn = '''/// Get tiers for a specific game. Falls back to universal tiers if no game-specific tiers exist.
pub fn get_tiers_for_game<'a>(tiers: &'a [BillingRateTier], sim_type: Option<rc_common::types::SimType>) -> Vec<&'a BillingRateTier> {
    let game_specific: Vec<_> = tiers.iter()
        .filter(|t| sim_type.is_some() && t.sim_type == sim_type)
        .collect();
    if !game_specific.is_empty() {
        game_specific
    } else {
        // Fall back to universal tiers (sim_type = None)
        tiers.iter().filter(|t| t.sim_type.is_none()).collect()
    }
}

'''
    content = content.replace(btimer_marker, get_tiers_fn + btimer_marker, 1)
    changes.append("get_tiers_for_game added")
elif 'fn get_tiers_for_game' in content:
    changes.append("get_tiers_for_game already present")
else:
    changes.append("ERROR: BillingTimer section marker not found")

# 5. Update current_cost
old_cc = '    pub fn current_cost(&self, tiers: &[BillingRateTier]) -> SessionCost {\n        compute_session_cost(self.elapsed_seconds, tiers)\n    }'
if old_cc in content:
    content = content.replace(old_cc,
        '    pub fn current_cost(&self, tiers: &[BillingRateTier]) -> SessionCost {\n        let filtered: Vec<BillingRateTier> = get_tiers_for_game(tiers, self.sim_type)\n            .into_iter()\n            .cloned()\n            .collect();\n        compute_session_cost(self.elapsed_seconds, &filtered)\n    }',
        1)
    changes.append("current_cost: uses get_tiers_for_game")
else:
    changes.append("current_cost: already updated or not found")

# 6. Add sim_type to BillingTimer struct
old_bt_end = '    /// Hard maximum session length in seconds (default 10800 = 3 hours)\n    pub max_session_seconds: u32,\n}\n\nimpl BillingTimer {'
if old_bt_end in content:
    content = content.replace(old_bt_end,
        '    /// Hard maximum session length in seconds (default 10800 = 3 hours)\n    pub max_session_seconds: u32,\n    /// Game sim_type for per-game rate lookup. None = use universal rates.\n    pub sim_type: Option<rc_common::types::SimType>,\n}\n\nimpl BillingTimer {',
        1)
    changes.append("BillingTimer::sim_type struct field added")
else:
    changes.append("BillingTimer::sim_type: already present or not found")

# 7. Fix BillingTimer::dummy()
old_dummy_end = '            pause_seconds: 0,\n            max_session_seconds: 1800,\n        }\n    }\n}\n\n// ─── WaitingForGameEntry'
if old_dummy_end in content:
    content = content.replace(old_dummy_end,
        '            pause_seconds: 0,\n            max_session_seconds: 1800,\n            sim_type: None,\n        }\n    }\n}\n\n// ─── WaitingForGameEntry',
        1)
    changes.append("BillingTimer::dummy: sim_type: None added")
else:
    changes.append("BillingTimer::dummy: already has sim_type or not found")

# 8. Add sim_type to WaitingForGameEntry
old_wfe = '    pub group_session_id: Option<String>,\n}\n\n// ─── MultiplayerBillingWait'
if old_wfe in content:
    content = content.replace(old_wfe,
        '    pub group_session_id: Option<String>,\n    /// Game sim_type for per-game rate lookup.\n    pub sim_type: Option<rc_common::types::SimType>,\n}\n\n// ─── MultiplayerBillingWait',
        1)
    changes.append("WaitingForGameEntry::sim_type field added")
else:
    changes.append("WaitingForGameEntry::sim_type: already present or not found")

# 9. Fix defer_billing_start
old_defer = '        attempt: 1,\n        group_session_id: group_session_id.clone(),\n    };\n    if group_session_id.is_some() {'
if old_defer in content:
    content = content.replace(old_defer,
        '        attempt: 1,\n        group_session_id: group_session_id.clone(),\n        sim_type: None,\n    };\n    if group_session_id.is_some() {',
        1)
    changes.append("defer_billing_start: WaitingForGameEntry sim_type: None added")
else:
    changes.append("defer_billing_start: already fixed or not found")

# 10. Fix recovery BillingTimer
old_recov = '            max_session_seconds: allocated_secs,\n        };\n\n        tracing::info!(\n            "Recovered billing session {} for driver {} on pod {} ({}/{}s)",'
if old_recov in content:
    content = content.replace(old_recov,
        '            max_session_seconds: allocated_secs,\n            sim_type: None,\n        };\n\n        tracing::info!(\n            "Recovered billing session {} for driver {} on pod {} ({}/{}s)",',
        1)
    changes.append("Recovery BillingTimer: sim_type: None added")
else:
    changes.append("Recovery BillingTimer: already fixed or not found")

# 11. Fix start_billing_session BillingTimer
old_sbs = '        max_session_seconds: allocated_seconds,\n    };\n\n    let rate_tiers = state.billing.rate_tiers.read().await;'
if old_sbs in content:
    content = content.replace(old_sbs,
        '        max_session_seconds: allocated_seconds,\n        sim_type: None,\n    };\n\n    let rate_tiers = state.billing.rate_tiers.read().await;',
        1)
    changes.append("start_billing_session BillingTimer: sim_type: None added")
else:
    changes.append("start_billing_session BillingTimer: already fixed or not found")

# 12. Fix handle_game_status_update signature
if '    _sim_type: Option<rc_common::types::SimType>,\n    _cmd_tx:' in content:
    content = content.replace(
        '    _sim_type: Option<rc_common::types::SimType>,\n    _cmd_tx:',
        '    sim_type: Option<rc_common::types::SimType>,\n    _cmd_tx:',
        1)
    changes.append("handle_game_status_update: activated sim_type param")
else:
    changes.append("handle_game_status_update: sim_type param already active or not found")

# 13. Update AcStatus::Live handler to propagate sim_type
old_live = '            let entry = state.billing.waiting_for_game.write().await.remove(pod_id);\n            if let Some(entry) = entry {'
if old_live in content and 'if sim_type.is_some() {' not in content:
    content = content.replace(old_live,
        '            let entry = state.billing.waiting_for_game.write().await.remove(pod_id);\n            if let Some(mut entry) = entry {\n                // Update sim_type from the GameStatusUpdate message\n                if sim_type.is_some() {\n                    entry.sim_type = sim_type;\n                }\n                let entry = entry;',
        1)
    changes.append("AcStatus::Live: sim_type propagation added")
else:
    changes.append("AcStatus::Live: sim_type already propagated or not found")

# 14. Fix test BillingTimer literals - add sim_type: None to test module
test_start = content.rfind('#[cfg(test)]\nmod tests {')
if test_start > 0:
    test_section = content[test_start:]
    fixed = 0
    # Replace max_session_seconds: N,\n        }; with sim_type: None added
    import re
    def add_sim_type(m):
        global fixed
        # Check if sim_type is already in the BillingTimer block
        before = m.string[:m.start(m.group(0))]
        bt_pos = before.rfind('BillingTimer {')
        if bt_pos >= 0 and 'sim_type:' not in before[bt_pos:]:
            fixed += 1
            return m.group(1) + m.group(2) + '    sim_type: None,\n' + m.group(3)
        return m.group(0)

    # Simple approach: find all BillingTimer struct inits in test section without sim_type
    test_fixed = 0
    while True:
        # Find a BillingTimer { ... } block without sim_type
        bt_pos = test_section.find('BillingTimer {')
        if bt_pos < 0:
            break
        # Find end of block (find matching })
        brace_count = 0
        block_start = test_section.find('{', bt_pos)
        i = block_start
        while i < len(test_section):
            if test_section[i] == '{':
                brace_count += 1
            elif test_section[i] == '}':
                brace_count -= 1
                if brace_count == 0:
                    break
            i += 1
        block = test_section[bt_pos:i+1]
        if 'sim_type:' not in block and 'max_session_seconds:' in block:
            # Find the last field before }
            last_comma = block.rfind(',\n')
            if last_comma > 0:
                indent = block[block.rfind('\n', 0, i-bt_pos):]
                indent = indent.split('\n')[1] if '\n' in indent else '            '
                # Remove leading content from indent
                insert = block[:last_comma+2] + '            sim_type: None,\n' + block[last_comma+2:]
                test_section = test_section[:bt_pos] + insert + test_section[bt_pos+len(block):]
                test_fixed += 1
            else:
                break
        else:
            # Skip this one - move past it
            test_section = test_section[:bt_pos] + test_section[bt_pos:].replace('BillingTimer {', 'BillingTimer{DONE{', 1)

    # Restore DONE markers
    test_section = test_section.replace('BillingTimer{DONE{', 'BillingTimer {')
    changes.append(f"Fixed {test_fixed} test BillingTimer literals")
    content = content[:test_start] + test_section

# Also fix test WaitingForGameEntry inits
test_start2 = content.rfind('#[cfg(test)]\nmod tests {')
if test_start2 > 0:
    test_section2 = content[test_start2:]
    wfe_fixed = 0
    while True:
        wfe_pos = test_section2.find('WaitingForGameEntry {')
        if wfe_pos < 0:
            break
        brace_count = 0
        block_start = test_section2.find('{', wfe_pos)
        i = block_start
        while i < len(test_section2):
            if test_section2[i] == '{':
                brace_count += 1
            elif test_section2[i] == '}':
                brace_count -= 1
                if brace_count == 0:
                    break
            i += 1
        block = test_section2[wfe_pos:i+1]
        if 'sim_type:' not in block and 'group_session_id:' in block:
            last_comma = block.rfind(',\n')
            if last_comma > 0:
                insert = block[:last_comma+2] + '                sim_type: None,\n' + block[last_comma+2:]
                test_section2 = test_section2[:wfe_pos] + insert + test_section2[wfe_pos+len(block):]
                wfe_fixed += 1
            else:
                break
        else:
            test_section2 = test_section2[:wfe_pos] + test_section2[wfe_pos:].replace('WaitingForGameEntry {', 'WaitingForGameEntry{DONE{', 1)
    test_section2 = test_section2.replace('WaitingForGameEntry{DONE{', 'WaitingForGameEntry {')
    changes.append(f"Fixed {wfe_fixed} test WaitingForGameEntry literals")
    content = content[:test_start2] + test_section2

print("Changes applied:")
for c in changes:
    print(" -", c)

with open('C:/Users/bono/racingpoint/racecontrol/crates/racecontrol/src/billing.rs', 'w', encoding='utf-8') as f:
    f.write(content)
print("Done. File length:", len(content))
