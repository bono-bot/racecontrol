# Non-Git Folder Catalogue

Phase 170 — 2026-03-23

## Summary

| Folder | Decision | Rationale |
|--------|----------|-----------|
| bat-sandbox | archive | One-off bat/py scripts from March 2026 deploy experiments. No active use, but may have historical reference value. |
| computer-use | archive | Experimental Claude computer-use agent (17K, 3 files). Prototype only, never integrated. |
| glitch-frames | delete | Diagnostic PNG frames from a kiosk glitch investigation (1.8MB, ~39 frames). Investigation resolved, frames have no ongoing value. |
| marketing | keep | 5.8GB of venue photos, videos, hype reel assets, and Instagram strategy docs. Business content Uday must review before any deletion. |
| serve | delete | Empty folder (4K, no files). Likely a leftover stub. |
| voice-assistant | archive | Python voice assistant prototype (9.6MB). Functional code with billing_monitor, STT/TTS, actions — superceded by AI relay approach. Historical reference value. |
| skills | keep | 668MB of Anthropic SDK, awesome-claude-skills, claude-cookbooks, courses, and MCP servers. Active reference material for Claude agent development. |

---

## Details

### bat-sandbox

- **Path:** C:/Users/bono/racingpoint/bat-sandbox
- **Contents:** sim-install-steps.bat/py, test-install.bat/py, test-remove-legacy.bat, test-template.bat, write-bat.py, write-fix-pod6.py, write-install-bat.py, write-nuke-server.py, write-start-rcagent.py
- **Size:** 101K
- **Last modified:** 2026-03-17
- **Decision:** archive
- **Rationale:** These are one-off scripts created during the March 2026 pod deploy experiments (Pod 6 fix, install script generation). The scripts are not referenced by any active process and the problems they solved are resolved. However, they represent hard-won knowledge about pod deployment edge cases. Recommended: compress to deploy-staging/archive/bat-sandbox-2026-03.zip, then delete the original folder.

---

### computer-use

- **Path:** C:/Users/bono/racingpoint/computer-use
- **Contents:** README.md, agent.py (10KB), run.ps1
- **Size:** 17K
- **Last modified:** 2026-03-03
- **Decision:** archive
- **Rationale:** Early experiment with Anthropic computer-use API (early March 2026). Contains a standalone agent.py with screenshot/click automation. Never integrated into any operational system. The approach was superseded by the comms-link relay architecture (v18.0). Small size but has conceptual value as reference. Recommended: compress to deploy-staging/archive/computer-use-2026-03.zip, then delete.

---

### glitch-frames

- **Path:** C:/Users/bono/racingpoint/glitch-frames
- **Contents:** frame_001.png through frame_039.png (approximately), mix of tiny 3.5KB frames and full 50-57KB frames
- **Size:** 1.8M
- **Last modified:** 2026-03-08
- **Decision:** delete
- **Rationale:** Diagnostic screenshots captured during a kiosk glitch investigation (March 8, 2026). The investigation is complete (kiosk API routing fix was applied in v16.1). These frames have no ongoing debugging or business value. Safe to delete immediately — no approval needed as they are purely diagnostic artifacts.

---

### marketing

- **Path:** C:/Users/bono/racingpoint/marketing
- **Contents:** Instagram strategy docs (3 MD files, ~57KB), venue photos, venue videos, reference videos, hype reel assets, ref-frames, ref-fellas, Higgsfield.js video tool, download/send scripts, package.json
- **Size:** 5.8G (bulk is in venue-photos, venue-videos, reference-videos subdirectories)
- **Last modified:** 2026-03-03
- **Decision:** keep
- **Rationale:** Contains irreplaceable venue photography, video assets, and Instagram strategy research. The 5.8GB is predominantly media assets that Uday may need for future marketing campaigns. The strategy documents (instagram-strategy.md, instagram-reels-best-practices-2026.md, instagram-feed-post-research-2026.md) represent researched content worth preserving. DO NOT DELETE without explicit approval from Uday Singh (usingh@racingpoint.in). Consider moving to a dedicated media storage location if disk space becomes a concern.

---

### serve

- **Path:** C:/Users/bono/racingpoint/serve
- **Contents:** Empty (directory only, no files)
- **Size:** 4K
- **Last modified:** 2026-03-08 (directory metadata only)
- **Decision:** delete
- **Rationale:** Completely empty directory. Likely a leftover stub from a project that was planned but never created, or a renamed/moved directory. Zero risk in deleting. Safe to delete immediately.

---

### voice-assistant

- **Path:** C:/Users/bono/racingpoint/voice-assistant
- **Contents:** main.py (14KB), billing_monitor.py, config.yaml, requirements.txt, test_components.py, test_voices.py, watchdog.cmd, plus subdirectories: actions/, audio/, data/, llm/, logs/, models/, stt/, tts/, __pycache__/
- **Size:** 9.6M
- **Last modified:** 2026-03-06
- **Decision:** archive
- **Rationale:** A functional Python voice assistant prototype with billing monitoring integration, speech-to-text, text-to-speech, and action execution. Built in early March 2026, never deployed to production. The architecture (local STT/TTS with action dispatch) was superseded by the Bono relay + Claude API approach. Contains real implementation logic (billing_monitor.py integrates with racecontrol API) that could be reference material for future voice features. Recommended: compress to deploy-staging/archive/voice-assistant-2026-03.zip, then delete the original.

---

### skills

- **Path:** C:/Users/bono/racingpoint/skills
- **Contents:** anthropic-sdk-python/ (official Python SDK), awesome-claude-skills/ (community skill templates), claude-cookbooks/ (Anthropic cookbook examples), courses/ (AI courses), servers/ (MCP server implementations)
- **Size:** 668M
- **Last modified:** 2026-03-03
- **Decision:** keep
- **Rationale:** This is a Claude agent development resource library — cloned references for the Anthropic Python SDK, Claude skill templates, and MCP server implementations. The `claude-cookbooks` subdirectory has its own CLAUDE.md, confirming active use as reference material. The 668MB is primarily SDK source code and examples. This library supports ongoing AI development at Racing Point. Note: this is NOT the same as the `.claude/skills/` directory used by GSD — it is a separate collection of reference repos. Keep in place; no action needed.
