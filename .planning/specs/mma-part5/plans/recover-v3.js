const fs = require("fs");
const path = require("path");
const data = JSON.parse(fs.readFileSync(path.join(__dirname, "v3.json"), "utf8"));
const raw = data.__raw_content;

// v3's raw begins with:  ```json\n{ plan_id..., strategy..., "commit_order": [ <corrupt>,
// Then mid-first-array: "title": "commit_order": [ <starts a clean second list>, ...
// Then a complete plan with all sections through "open_questions": []\n}\n```
//
// Strategy: locate the SECOND occurrence of 'commit_order' (inside the "title" string of seq 2),
// splice out everything from that point, wrap with plan_id/strategy from the first block.

const first = raw.indexOf('"commit_order"');
const second = raw.indexOf('"commit_order"', first + 1);
if (second < 0) { console.error("NO_SECOND_OCCURRENCE"); process.exit(1); }

// Keep content BEFORE `first` (plan_id + strategy)
// and content FROM `second` onward.
const head = raw.substring(0, first);
const tail = raw.substring(second);

// Stitch: head already ends in "...strategy": "...", — so we append the tail directly.
// But head might end with a comma + newline + spaces before the commit_order key.
// Ensure head ends with ', ' and then prepend tail.
const stitched = head.replace(/,?\s*$/, ", ") + tail;

// Now cut off the trailing ```
const clean = stitched.replace(/```[\s\S]*$/, "");

// Find matching close brace for the root object (brace-balance)
let depth = 0;
let inStr = false;
let esc = false;
let endIdx = -1;
// We need to find the root `{` first
const firstBrace = clean.indexOf("{");
for (let i = firstBrace; i < clean.length; i++) {
  const c = clean[i];
  if (esc) { esc = false; continue; }
  if (c === "\\" && inStr) { esc = true; continue; }
  if (c === '"') { inStr = !inStr; continue; }
  if (inStr) continue;
  if (c === "{") depth++;
  else if (c === "}") {
    depth--;
    if (depth === 0) { endIdx = i; break; }
  }
}
const candidate = endIdx > 0 ? clean.substring(firstBrace, endIdx + 1) : clean;

try {
  const parsed = JSON.parse(candidate);
  parsed.plan_id = (parsed.plan_id || "plan-v3") + "-recovered";
  parsed.__note = "Recovered from malformed JSON — v3 returned duplicated commit_order; second block was complete.";
  fs.writeFileSync(path.join(__dirname, "v3.recovered.json"), JSON.stringify(parsed, null, 2));
  console.log("OK sections:", Object.keys(parsed).join(","));
} catch (e) {
  console.error("PARSE_FAIL:", e.message);
  fs.writeFileSync(path.join(__dirname, "v3.recovered.raw"), candidate);
  process.exit(2);
}
