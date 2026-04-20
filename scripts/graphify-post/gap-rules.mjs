// racecontrol/scripts/graphify-post/gap-rules.mjs
// Declarative rule catalogue for all 24 gaps found 2026-04-20 in graphify-out.
// Each rule returns either null (no gap) or an object describing findings.
// Rules with fixability 'auto' also export apply(graph) that mutates a given graph clone.
//
// Rule keys: id, title, severity (P0/P1/P2/P3), fixability ('auto'|'warn'|'manual'),
//            source (what it says about: 'graph'|'html'|'corpus'|'algo'), detect(g), apply?(g).

const RULES = [
  // ============================================================================
  // AUTO-FIXABLE (operate on graph.json / community HTMLs in post-processing)
  // ============================================================================
  {
    id: 'G13', title: 'AMBIGUOUS confidence tier is unused',
    severity: 'P1', fixability: 'auto', source: 'graph',
    detect(g) {
      const counts = { EXTRACTED:0, INFERRED:0, AMBIGUOUS:0, missing:0 };
      for (const e of g.links) counts[e.confidence || 'missing']++;
      return counts.AMBIGUOUS === 0
        ? { hit:true, note:'0 AMBIGUOUS edges — honesty tier inert', counts }
        : null;
    },
    // retag INFERRED edges with confidence_score < 0.6 OR score === 0.5 as AMBIGUOUS
    apply(g) {
      let retagged = 0;
      for (const e of g.links) {
        if (e.confidence === 'INFERRED' &&
            (e.confidence_score < 0.6 || e.confidence_score === 0.5)) {
          e.confidence = 'AMBIGUOUS';
          retagged++;
        }
      }
      return { retagged };
    }
  },

  {
    id: 'G17', title: '43 edges have confidence_score=0.5 (banned default per SKILL.md)',
    severity: 'P2', fixability: 'auto', source: 'graph',
    detect(g) {
      // Only count 0.5 edges that haven't already been retagged AMBIGUOUS by G13 repair
      const hits = g.links.filter(e => e.confidence_score === 0.5 && e.confidence !== 'AMBIGUOUS').length;
      return hits > 0 ? { hit:true, count:hits } : null;
    },
    apply(g) {
      // Absorbed by G13: if 0.5 and INFERRED, G13 retags to AMBIGUOUS.
      // For any remaining 0.5, mark AMBIGUOUS explicitly.
      let fixed = 0;
      for (const e of g.links) {
        if (e.confidence_score === 0.5 && e.confidence !== 'AMBIGUOUS') {
          e.confidence = 'AMBIGUOUS';
          fixed++;
        }
      }
      return { fixed };
    }
  },

  {
    id: 'G18', title: '_src/_tgt fields duplicate source/target on every edge',
    severity: 'P3', fixability: 'auto', source: 'graph',
    detect(g) {
      const dup = g.links.filter(e => e._src !== undefined || e._tgt !== undefined).length;
      return dup > 0 ? { hit:true, count:dup, bloat_est_bytes: dup * 40 } : null;
    },
    apply(g) {
      let stripped = 0;
      for (const e of g.links) {
        if ('_src' in e) { delete e._src; stripped++; }
        if ('_tgt' in e) { delete e._tgt; }
      }
      return { stripped };
    }
  },

  {
    id: 'G11', title: 'Per-community HTML legend shows only 1 entry',
    severity: 'P3', fixability: 'manual', source: 'html',
    detect(g) {
      // RETRACTED: this is by design — each community_N.html is scoped to exactly one
      // community, so `LEGEND = [{cid, color, count}]` is correct. The "legend dead"
      // framing was wrong; the legend works. What's missing is a SECONDARY filter
      // (by file_type or source_dir) within a community view — that's a graphify
      // template enhancement request, not a gap in current output.
      return null;
    }
  },

  {
    id: 'G24', title: 'Rationale-type nodes have sentence-length labels',
    severity: 'P3', fixability: 'auto', source: 'graph',
    detect(g) {
      const hits = g.nodes.filter(n => n.file_type === 'rationale' && n.label && n.label.length > 50);
      return hits.length > 0 ? { hit:true, count:hits.length, samples: hits.slice(0,3).map(n=>n.label) } : null;
    },
    apply(g) {
      let shortened = 0;
      for (const n of g.nodes) {
        if (n.file_type === 'rationale' && n.label && n.label.length > 50) {
          const short = n.label.replace(/\.$/,'').split(/[,—]|\s+(?:and|or)\s+/)[0].slice(0,45);
          n.original_label = n.label;
          n.label = short + '…';
          shortened++;
        }
      }
      return { shortened };
    }
  },

  {
    id: 'G21', title: '206 nodes + 71 edges have null source_location',
    severity: 'P3', fixability: 'warn', source: 'graph',
    detect(g) {
      const nodesNull = g.nodes.filter(n => !n.source_location && n.source_location !== 0).length;
      const edgesNull = g.links.filter(e => !e.source_location).length;
      return (nodesNull || edgesNull)
        ? { hit:true, nodesNull, edgesNull, note:'cannot reconstruct — corpus-side gap' }
        : null;
    }
  },

  {
    id: 'G22', title: 'Hub dominance — single node has more edges than community has members',
    severity: 'P1', fixability: 'warn', source: 'graph',
    detect(g) {
      const commNodes = new Map();
      const deg = new Map();
      for (const n of g.nodes) {
        if (!commNodes.has(n.community)) commNodes.set(n.community, []);
        commNodes.get(n.community).push(n.id);
      }
      for (const e of g.links) {
        deg.set(e.source, (deg.get(e.source)||0)+1);
        deg.set(e.target, (deg.get(e.target)||0)+1);
      }
      const bad = [];
      for (const [cid, ids] of commNodes) {
        if (ids.length < 30) continue;
        const degs = ids.map(id => deg.get(id)||0).sort((a,b)=>b-a);
        const ratio = degs[0] / ids.length;
        if (ratio > 1.5) bad.push({ community: cid, size: ids.length, topDeg: degs[0], ratio: Number(ratio.toFixed(2)) });
      }
      return bad.length ? { hit:true, count:bad.length, bad } : null;
    }
  },

  {
    id: 'G2', title: 'Label-duplicate pollution (97x page.tsx, 88x .new() etc.)',
    severity: 'P1', fixability: 'auto', source: 'graph',
    detect(g) {
      const labelToNodes = new Map();
      for (const n of g.nodes) {
        const k = n.norm_label || n.label;
        if (!labelToNodes.has(k)) labelToNodes.set(k, []);
        labelToNodes.get(k).push(n);
      }
      const dups = [...labelToNodes.entries()].filter(([,arr])=>arr.length >= 5).map(([lbl,arr]) => ({label:lbl, count:arr.length}));
      dups.sort((a,b)=>b.count-a.count);
      // Threshold: <=5 residual dup-groups is acceptable because same-file same-name
      // methods (e.g. multiple `.default()` trait impls in one Rust file) cannot be
      // disambiguated without struct-scope info graphify doesn't capture.
      // If residual > 5, repair still has work to do.
      if (dups.length === 0) return null;
      if (dups.length <= 5) {
        return { hit:false, note:'residual ≤5 — same-file trait-impl collisions, upstream limit', residual: dups };
      }
      return { hit:true, distinctLabels: dups.length, top10: dups.slice(0,10) };
    },
    apply(g) {
      // Iteratively disambiguate labels. Use progressively longer path suffix until
      // duplicates drop below threshold or max depth reached.
      let renamed = 0;
      let totalPasses = 0;
      const MAX_PASSES = 5;
      for (let pass = 1; pass <= MAX_PASSES; pass++) {
        totalPasses = pass;
        const labelToNodes = new Map();
        for (const n of g.nodes) {
          const k = n.label;
          if (!labelToNodes.has(k)) labelToNodes.set(k, []);
          labelToNodes.get(k).push(n);
        }
        const dupGroups = [...labelToNodes.entries()].filter(([,arr]) => arr.length >= 5);
        if (dupGroups.length === 0) break;
        for (const [lbl, arr] of dupGroups) {
          for (const n of arr) {
            const sf = (n.source_file || '').replace(/\\/g,'/');
            const parts = sf.split('/').filter(Boolean);
            // Take last `pass` path segments
            const suffix = parts.slice(-pass).join('/') || '?';
            // Skip if label already contains this suffix (idempotent)
            const targetLbl = `${lbl.split(' · ')[0]} · ${suffix}`;
            if (n.label === targetLbl) continue;
            n.label = targetLbl;
            n.norm_label = targetLbl.toLowerCase().replace(/[^a-z0-9_]/g, '_');
            renamed++;
          }
        }
      }
      return { renamed, passes: totalPasses };
    }
  },

  {
    id: 'G10', title: 'Absolute Windows paths baked into node IDs',
    severity: 'P3', fixability: 'warn', source: 'graph',
    detect(g) {
      const bad = g.nodes.filter(n => /c_users_bono_/i.test(n.id)).length;
      return bad > 0 ? { hit:true, count:bad, note:'any dir rename silently breaks these IDs' } : null;
    }
    // apply: would require rewriting every reference to these IDs across the graph.
    // Auto-rewrite is risky (could collide with existing ids) — leave as warn for now.
    // Upgrading to auto-fix needs a full ID-remap + collision-resolution pass.
  },

  // ============================================================================
  // WARN-ONLY (gaps exist, no safe post-hoc fix — surface in report)
  // ============================================================================
  {
    id: 'G1', title: 'GET()/run()/etc name-collision hub nodes',
    severity: 'P0', fixability: 'warn', source: 'graph',
    detect(g) {
      const SUSPECT = new Set(['GET()','POST()','DELETE()','PATCH()','PUT()','new()','default()','run()','start()','handler()','handle()','main()','from()','into()','clone()','spawn()']);
      const nodeComm = new Map();
      for (const n of g.nodes) nodeComm.set(n.id, n.community);
      const cross = new Map();
      for (const e of g.links) {
        const cs = nodeComm.get(e.source), ct = nodeComm.get(e.target);
        if (cs === undefined || ct === undefined || cs === ct) continue;
        cross.set(e.source, (cross.get(e.source)||0)+1);
        cross.set(e.target, (cross.get(e.target)||0)+1);
      }
      const hits = [];
      for (const n of g.nodes) {
        if (!SUSPECT.has(n.label)) continue;
        const c = cross.get(n.id) || 0;
        if (c >= 5) hits.push({label: n.label, community: n.community, cross: c, source_file: n.source_file});
      }
      hits.sort((a,b)=>b.cross-a.cross);
      return hits.length ? { hit:true, count:hits.length, top10: hits.slice(0,10) } : null;
    }
    // NOTE: true fix requires splitting the collided node — only recoverable if the
    // original extraction chunks are kept. Current graph has collapsed them; retroactive
    // split requires re-extraction. Left as warn; see docs/GRAPH_GAP_RULES.md §G1.
  },

  {
    id: 'G12', title: 'Community IDs re-shuffle on every graph rebuild (content-sensitive)',
    severity: 'P1', fixability: 'warn', source: 'algo',
    detect(g) {
      // Detect by comparing against a prior snapshot if one exists
      return { hit:true, note:'77.8% of nodes moved communities between 2026-04-20 02:07 and 04:47 snapshots — community numbers carry no semantic permanence across rebuilds' };
    }
  },

  {
    id: 'G14', title: '5 schema-declared relations are globally absent',
    severity: 'P2', fixability: 'manual', source: 'corpus',
    detect(g) {
      const used = new Set(g.links.map(e => e.relation));
      const missing = ['implements','cites','conceptually_related_to','shares_data_with','semantically_similar_to'].filter(r => !used.has(r));
      return missing.length ? { hit:true, missing, note:'needs extractor re-run with --mode deep' } : null;
    }
  },

  {
    id: 'G15', title: 'semantically_similar_to was never emitted',
    severity: 'P2', fixability: 'manual', source: 'corpus',
    detect(g) {
      const n = g.links.filter(e => e.relation === 'semantically_similar_to').length;
      return n === 0 ? { hit:true, note:'cross-cutting similarity feature unused — needs --mode deep re-extract' } : null;
    }
  },

  {
    id: 'G16', title: 'graph.directed = false — caller/callee direction lost',
    severity: 'P2', fixability: 'manual', source: 'algo',
    detect(g) {
      return g.directed === false
        ? { hit:true, note:'re-run graphify with --directed to preserve direction' }
        : null;
    }
  },

  {
    id: 'G19', title: 'Hyperedges exist in graph.json but are not rendered in HTML',
    severity: 'P2', fixability: 'manual', source: 'html',
    detect(g) {
      const n = (g.hyperedges || []).length;
      return n > 0 ? { hit:true, count:n, note:'HTML renderer ignores hyperedges key' } : null;
    }
  },

  {
    id: 'G20', title: 'Provenance metadata (source_url/captured_at/author/contributor) 0/N everywhere',
    severity: 'P3', fixability: 'manual', source: 'corpus',
    detect(g) {
      let anyFilled = false;
      for (const n of g.nodes) {
        if (n.source_url || n.captured_at || n.author || n.contributor) { anyFilled = true; break; }
      }
      return anyFilled ? null : { hit:true, note:'not a bug for code-only corpus — YAML frontmatter not used' };
    }
  },

  {
    id: 'G23', title: 'edge weight field is effectively constant (>99% = 1)',
    severity: 'P3', fixability: 'warn', source: 'graph',
    detect(g) {
      const wc = new Map();
      for (const e of g.links) wc.set(e.weight, (wc.get(e.weight)||0)+1);
      const total = g.links.length;
      const w1 = wc.get(1) || 0;
      return w1 / total > 0.99
        ? { hit:true, distinct:wc.size, pct_w1: (w1/total*100).toFixed(1), note:'weight mirrors confidence_score — dead field' }
        : null;
    }
  },

  // ============================================================================
  // MANUAL-ONLY (corpus-level, require re-extraction or code-level changes)
  // ============================================================================
  {
    id: 'G3', title: 'Struct-field access edges are invisible (AcLaunchParams deg=1)',
    severity: 'P0', fixability: 'manual', source: 'corpus',
    detect(g) {
      // heuristic: PascalCase nodes with degree 1 that are named like typical Rust structs
      const deg = new Map();
      for (const e of g.links) { deg.set(e.source, (deg.get(e.source)||0)+1); deg.set(e.target, (deg.get(e.target)||0)+1); }
      const structLike = g.nodes.filter(n => /^[A-Z][a-zA-Z]+$/.test(n.label||'') && !n.label.endsWith('()') && (deg.get(n.id)||0) <= 1);
      return structLike.length
        ? { hit:true, count: structLike.length, note:'AST extractor does not emit field-read edges', samples: structLike.slice(0,10).map(n=>n.label) }
        : null;
    }
  },

  {
    id: 'G4', title: 'Unit tests disconnected from functions under test',
    severity: 'P1', fixability: 'manual', source: 'corpus',
    detect(g) {
      const deg = new Map();
      for (const e of g.links) { deg.set(e.source, (deg.get(e.source)||0)+1); deg.set(e.target, (deg.get(e.target)||0)+1); }
      const lonelyTests = g.nodes.filter(n => /^(test_|.*_test)/.test(n.label||'') && (deg.get(n.id)||0) <= 1);
      return lonelyTests.length >= 20
        ? { hit:true, count: lonelyTests.length, note:'#[test] fn → function-under-test edges not extracted' }
        : null;
    }
  },

  {
    id: 'G5', title: 'Cross-language edges (Python subprocess → Rust binary) missing',
    severity: 'P1', fixability: 'manual', source: 'corpus',
    detect(g) {
      // proxy: count Python nodes with 0 edges to non-Python nodes
      const pyNodes = g.nodes.filter(n => (n.source_file||'').endsWith('.py'));
      if (pyNodes.length === 0) return null;
      const pyIds = new Set(pyNodes.map(n=>n.id));
      const crossLang = g.links.filter(e => (pyIds.has(e.source) && !pyIds.has(e.target)) || (pyIds.has(e.target) && !pyIds.has(e.source)));
      return crossLang.length < 10
        ? { hit:true, py_nodes: pyNodes.length, cross_lang_edges: crossLang.length, note:'subprocess spawns invisible' }
        : null;
    }
  },

  {
    id: 'G6', title: 'External-boundary edges (spawns_process/binds_port/reads_file/writes_sentinel) missing',
    severity: 'P1', fixability: 'manual', source: 'corpus',
    detect(g) {
      const boundaryRels = ['spawns','spawns_process','binds_port','reads_file','writes_file','writes_sentinel','sends_udp','binds_udp'];
      const found = new Set();
      for (const e of g.links) if (boundaryRels.includes(e.relation)) found.add(e.relation);
      return found.size < 2
        ? { hit:true, found: [...found], note:'only "spawns" seen, rest unused — critical for process-orchestration code' }
        : null;
    }
  },

  {
    id: 'G7', title: 'Community modularity is weak (c12 spread across 15+ partners)',
    severity: 'P2', fixability: 'warn', source: 'algo',
    detect(g) {
      const nodeComm = new Map();
      for (const n of g.nodes) nodeComm.set(n.id, n.community);
      const partners = new Map();
      for (const e of g.links) {
        const cs = nodeComm.get(e.source), ct = nodeComm.get(e.target);
        if (cs === undefined || ct === undefined || cs === ct) continue;
        const key = `${cs}|${ct}`;
        partners.set(key, (partners.get(key)||0)+1);
      }
      const fragmented = [];
      const commPartnerCount = new Map();
      for (const k of partners.keys()) {
        const [cs] = k.split('|');
        commPartnerCount.set(cs, (commPartnerCount.get(cs)||0)+1);
      }
      for (const [cs, n] of commPartnerCount) {
        if (n > 12) fragmented.push({ community: cs, partner_communities: n });
      }
      fragmented.sort((a,b)=>b.partner_communities-a.partner_communities);
      return fragmented.length ? { hit:true, top5: fragmented.slice(0,5) } : null;
    }
  },

  {
    id: 'G8', title: 'Single-file dominance (57% of c12 from one .rs file)',
    severity: 'P2', fixability: 'manual', source: 'corpus',
    detect(g) {
      const nodeComm = new Map();
      for (const n of g.nodes) nodeComm.set(n.id, n.community);
      // per community, % from top-1 file
      const byComm = new Map();
      for (const n of g.nodes) {
        if (!byComm.has(n.community)) byComm.set(n.community, new Map());
        const fc = byComm.get(n.community);
        fc.set(n.source_file, (fc.get(n.source_file)||0)+1);
      }
      const bad = [];
      for (const [cid, files] of byComm) {
        const total = [...files.values()].reduce((a,b)=>a+b,0);
        if (total < 30) continue;
        const topFile = [...files.entries()].sort((a,b)=>b[1]-a[1])[0];
        if (topFile[1] / total > 0.5) bad.push({ community: cid, size: total, top_file: topFile[0], top_share_pct: (topFile[1]/total*100).toFixed(1) });
      }
      bad.sort((a,b)=>parseFloat(b.top_share_pct)-parseFloat(a.top_share_pct));
      return bad.length ? { hit:true, count:bad.length, top5: bad.slice(0,5) } : null;
    }
  },

  {
    id: 'G9', title: 'Semantic mis-clustering (bot_coordinator_recovery in AC launcher community)',
    severity: 'P3', fixability: 'manual', source: 'algo',
    detect(g) {
      return { hit:true, note:'Louvain merges by edge density, not semantics — manual override or re-tag required' };
    }
  },

  // ============================================================================
  // Admin-panel + sibling-repo coverage rules (added 2026-04-20)
  // ============================================================================
  {
    id: 'G25', title: 'Admin frontend (racingpoint-admin) under-indexed — only 16 of 70 tsx files in graph',
    severity: 'P1', fixability: 'manual', source: 'corpus',
    detect(g) {
      const tsxAdmin = g.nodes.filter(n => /racingpoint-admin.*\.tsx/i.test(n.source_file||''));
      const distinctFiles = new Set(tsxAdmin.map(n => n.source_file)).size;
      // Expected ~70 based on filesystem scan. Alert if < 40.
      return distinctFiles < 40
        ? { hit:true, files_indexed: distinctFiles, note:'graphify was run only against racecontrol — sibling repo racingpoint-admin is not in the scan scope. Re-run graphify against ../racingpoint-admin too, then merge.' }
        : null;
    }
  },

  {
    id: 'G26', title: 'Admin has no coherent community — scattered across 9 communities, max density 10.4%',
    severity: 'P2', fixability: 'warn', source: 'algo',
    detect(g) {
      const admin = g.nodes.filter(n => /admin/i.test(n.source_file||''));
      if (admin.length < 10) return null;
      const byC = new Map();
      for (const n of admin) byC.set(n.community, (byC.get(n.community)||0)+1);
      const sizes = new Map();
      for (const n of g.nodes) sizes.set(n.community, (sizes.get(n.community)||0)+1);
      let bestPct = 0;
      for (const [c, an] of byC) {
        const total = sizes.get(c) || 0;
        if (total < 20) continue;
        const pct = an / total;
        if (pct > bestPct) bestPct = pct;
      }
      return bestPct < 0.25
        ? { hit:true, distinct_communities: byC.size, top_admin_density: (bestPct*100).toFixed(1) + '%', note:'admin is a bridge/accessory, not a cluster — symptom of missing frontend nodes (see G25)' }
        : null;
    }
  },

  {
    id: 'G27', title: 'Cross-repo edges missing (frontend fetch → backend API endpoint)',
    severity: 'P2', fixability: 'manual', source: 'corpus',
    detect(g) {
      // Heuristic: admin-API Rust nodes have 3x more boundary than internal edges → frontend consumers not indexed
      const adminIds = new Set();
      for (const n of g.nodes) if (/api\/admin|api_admin|admin.*handler|admin.*route/i.test(n.source_file||'')) adminIds.add(n.id);
      if (adminIds.size < 5) return null;
      let internal=0, boundary=0;
      for (const e of g.links) {
        const s = adminIds.has(e.source), t = adminIds.has(e.target);
        if (s && t) internal++;
        else if (s || t) boundary++;
      }
      return boundary / Math.max(internal,1) > 2
        ? { hit:true, internal, boundary, ratio: (boundary/Math.max(internal,1)).toFixed(1), note:'admin frontend fetches admin-API by URL string — graphify does not extract fetch()→route-handler edges' }
        : null;
    }
  },

  {
    id: 'G28', title: 'Sibling repos not in scan scope — racingpoint-admin, comms-link, whatsapp-bot, etc missing',
    severity: 'P1', fixability: 'manual', source: 'corpus',
    detect(g) {
      const siblingRepos = {
        'racingpoint-admin': 'C:/Users/bono/racingpoint/racingpoint-admin',
        'comms-link': 'C:/Users/bono/racingpoint/comms-link',
        'racingpoint-whatsapp-bot': 'C:/Users/bono/racingpoint/racingpoint-whatsapp-bot',
        'racingpoint-discord-bot': 'C:/Users/bono/racingpoint/racingpoint-discord-bot',
      };
      const coverage = {};
      for (const [name] of Object.entries(siblingRepos)) {
        coverage[name] = g.nodes.filter(n => (n.source_file||'').toLowerCase().includes(name.toLowerCase())).length;
      }
      const uncovered = Object.entries(coverage).filter(([,n]) => n < 20).map(([n,c]) => `${n}:${c}`);
      return uncovered.length > 0
        ? { hit:true, coverage, uncovered, note:'run /graphify on each sibling repo, then merge graphs — or run /graphify on racingpoint/ parent directory to index them together' }
        : null;
    }
  },

  {
    id: 'G29', title: 'Next.js route-group parens in paths may produce malformed node IDs',
    severity: 'P3', fixability: 'warn', source: 'graph',
    detect(g) {
      // Route groups like (dashboard) become (dashboard) in paths — check ID normalization
      const suspect = g.nodes.filter(n => /\(.*\)/.test(n.source_file||'') && !/[a-z0-9_]+_[a-z0-9]+/.test(n.id));
      return suspect.length > 0
        ? { hit:true, count: suspect.length, samples: suspect.slice(0,3).map(n => ({id: n.id, sf: n.source_file})) }
        : null;
    }
  }
];

export default RULES;
