"""
scan-module.py — minimal graphify pipeline for code-only corpora.
Runs AST extraction, skips semantic (no LLM), builds graph + clustering + HTML.

Usage:  python scan-module.py <path_to_module>

Writes <path>/graphify-out/{graph.json, graph.html, GRAPH_REPORT.md}
"""
import sys
import json
import io
from pathlib import Path
# Force UTF-8 on stdout — Windows defaults to cp1252 which chokes on arrows/unicode
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8', errors='replace')
sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8', errors='replace')

if len(sys.argv) < 2:
    print("Usage: python scan-module.py <module_path>")
    sys.exit(1)

target = Path(sys.argv[1]).resolve()
if not target.exists():
    print(f"ERROR: {target} does not exist")
    sys.exit(1)

out_dir = target / 'graphify-out'
out_dir.mkdir(exist_ok=True)

print(f"[1/6] Detecting files in {target} ...")
from graphify.detect import detect, save_manifest
detection = detect(target)
(out_dir / '.graphify_detect.json').write_text(json.dumps(detection))

files = detection.get('files', {})
code = files.get('code', [])
docs = files.get('document', [])
papers = files.get('paper', [])
images = files.get('image', [])
video = files.get('video', [])

print(f"   code={len(code)} docs={len(docs)} papers={len(papers)} images={len(images)} video={len(video)}")

# Refuse if non-trivial docs/images/video (needs LLM)
if len(docs) + len(papers) + len(images) + len(video) > 3:
    print(f"   ABORT: corpus has {len(docs)+len(papers)+len(images)+len(video)} non-code files — semantic extraction needed, not free")
    sys.exit(2)

print(f"[2/6] AST extraction on {len(code)} code files ...")
from graphify.extract import extract, collect_files
code_files = []
for f in code:
    p = Path(f)
    code_files.extend(collect_files(p) if p.is_dir() else [p])
if not code_files:
    print("   ABORT: no extractable code files")
    sys.exit(3)
ast_result = extract(code_files)
print(f"   AST: {len(ast_result['nodes'])} nodes, {len(ast_result['edges'])} edges")

# Merge (no semantic) → extract.json
merged = {
    'nodes': ast_result['nodes'],
    'edges': ast_result['edges'],
    'hyperedges': [],
    'input_tokens': 0,
    'output_tokens': 0,
}
(out_dir / '.graphify_extract.json').write_text(json.dumps(merged))

print(f"[3/6] Building graph + clustering ...")
from graphify.build import build_from_json
from graphify.cluster import cluster, score_all
from graphify.analyze import god_nodes, surprising_connections, suggest_questions
from graphify.report import generate
from graphify.export import to_json

G = build_from_json(merged)
if G.number_of_nodes() == 0:
    print("   ERROR: graph empty")
    sys.exit(4)
communities = cluster(G)
cohesion = score_all(G, communities)
gods = god_nodes(G)
surprises = surprising_connections(G, communities)
labels = {cid: f'Community {cid}' for cid in communities}
questions = suggest_questions(G, communities, labels)

print(f"   graph: {G.number_of_nodes()} nodes, {G.number_of_edges()} edges, {len(communities)} communities")

print(f"[4/6] Writing graph.json + GRAPH_REPORT.md ...")
tokens = {'input': 0, 'output': 0}
report_md = generate(G, communities, cohesion, labels, gods, surprises, detection, tokens, str(target), suggested_questions=questions)
(out_dir / 'GRAPH_REPORT.md').write_text(report_md, encoding='utf-8')
to_json(G, communities, str(out_dir / 'graph.json'))

print(f"[5/6] Writing graph.html ...")
from graphify.export import to_html
if G.number_of_nodes() > 5000:
    print(f"   skipping HTML (graph has {G.number_of_nodes()} nodes, over 5000 cap)")
else:
    to_html(G, communities, str(out_dir / 'graph.html'), community_labels=labels or None)

print(f"[6/6] Saving manifest ...")
# Pass explicit manifest_path — graphify's default _MANIFEST_PATH is CWD-relative
# ('graphify-out/manifest.json'), which would land the manifest in whatever dir
# this script was invoked from instead of the target module's graphify-out/.
save_manifest(detection['files'], manifest_path=str(out_dir / 'manifest.json'))

# Clean up temp files
for f in ['.graphify_detect.json', '.graphify_extract.json']:
    p = out_dir / f
    if p.exists():
        p.unlink()

print(f"DONE → {out_dir}")
print(f"  graph.json          {(out_dir/'graph.json').stat().st_size} bytes")
if (out_dir/'graph.html').exists():
    print(f"  graph.html          {(out_dir/'graph.html').stat().st_size} bytes")
print(f"  GRAPH_REPORT.md     {(out_dir/'GRAPH_REPORT.md').stat().st_size} bytes")
