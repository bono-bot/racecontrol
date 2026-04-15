# Proposal: Create a shared "workspace" repo on GitHub

**For:** Uday Singh
**From:** James (with Bono's co-authorship)
**Date:** 2026-04-15
**Ask:** Create one GitHub repo under your account. Add two collaborators. That's it.

---

## The problem in one paragraph

Right now, James and Bono run on two different machines (James on your PC at the venue, Bono on the VPS in the cloud). We each have our own copy of hooks, memory files, scripts, and settings scattered across folders like `~/.claude/hooks/`, `~/.claude/agents/`, and `claude-code-bootstrap/`. There is no single place where "the shared brain" lives. When one of us learns something, the other doesn't automatically know it. When you update a rule for James, Bono drifts. When I fix a bug in a hook, Bono's copy stays broken until I manually push it over. This is slow, error-prone, and the reason you keep seeing us contradict each other.

## The fix

One GitHub repo called **`workspace`** that both James and Bono read and write to. Think of it like a shared Google Drive folder for the two of us — one place for hooks, one place for memory, one place for settings, one place for everything. Whatever lands in that repo gets automatically copied into both of our machines within seconds.

Your analogy from today's conversation: *"Whether I'm on Bono or on James, I can access both and understand both. It's just shared between both of you, and it's easier to find everything in one place."* This is exactly that.

## Why it has to be under your GitHub account

Three reasons:

1. **Neutrality.** If the repo lives on James's machine, it's "James's." If it lives on Bono's VPS, it's "Bono's." Neither of those is true — it's shared. Putting it under your account makes it neutral. Neither AI owns the canonical copy.

2. **Authority.** If James and Bono ever disagree on something big, you're the tiebreaker. The person with the final say should hold admin on the repo. That's you.

3. **Survives us.** If your PC dies or the VPS gets rebuilt, the workspace is still safe in your GitHub account. The two of us are replaceable; your knowledge base shouldn't be.

## What I'm asking you to do

Literally three clicks:

1. Go to github.com → New repository
2. Name: `workspace`. Visibility: **Private**. Owner: your account (or a `racingpoint` organization if you want, your call).
3. Settings → Collaborators → add `james-racingpoint` and `bono-racingpoint` with push access.

That's it. You don't need to write code, run scripts, or install anything. The two of us handle the rest.

## What I'm NOT asking you to do

- You don't need to review every commit. GitHub shows you a list if you ever want to browse.
- You don't need to merge anything. We handle that via automated CI.
- You don't need to write code. We handle all the migration, testing, and file moves.
- You don't need to fix merge conflicts. The CI gate prevents broken code from ever reaching the main branch.

## Safety — how broken changes get stopped before they reach us

Both of us push changes to temporary "work-in-progress" branches first. GitHub Actions runs an automatic safety check on every push — checks that the code is syntactically valid, runs on both Windows and Linux, has no secrets committed, and doesn't exceed size limits. If the check passes, the change merges into the main branch and both machines automatically pull it. If it fails, the change sits on the temporary branch and never reaches either of our live systems. After 24 hours, failed temporary branches auto-delete. The main branch only ever holds verified, working code.

In plain English: **neither of us can break the other. The gate catches it first.**

## Secrets — what's NOT going into the repo

API keys, passwords, and credentials **never** enter the repo. They stay on each machine in a separate folder (`~/.claude-secrets/`) that is explicitly excluded from git. Even if one of us accidentally tries to commit a secret, a pre-commit hook and the CI gate both block it. We've seen real incidents in other contexts (the SSH config corruption of April 8) where treating secrets casually caused 25 minutes of downtime — this is the structural fix.

## Timeline

- **Today (2026-04-15):** Phase 393 (this document) — design and draft. No code moved yet.
- **Phase 394–396:** Resolve existing drift between James and Bono, write architecture docs, initialize the repo skeleton. ~1 week of AI-side work.
- **Phase 397–403:** Gradual migration. One folder at a time. Hooks migrate last because they're highest risk.
- **Phase 404–408:** Cleanup, decommission old scattered locations, final verification.
- **Total:** 16 phases, probably 2–3 weeks of AI work wall-clock. Zero work on your side after the initial repo creation.

## What you get at the end

- One place to look when you ask "what does the AI know?"
- Guaranteed sync between James and Bono — no more "James said X, Bono said Y" drift
- GitHub web UI browse access to the entire shared brain (useful if you want to read what we know)
- A green/red checkmark on every commit showing if that change passed the gate
- A structural fix for the class of bug where one of us knows something the other doesn't

## Your approval

If this makes sense, just reply "ok, create the repo" and specify:

- Should the owner be **your personal GitHub account** (`usingh@racingpoint.in` — whatever username is linked) or a **new `racingpoint` organization** (cleaner long-term but one extra step)?
- Should it be **private** (yes — default recommendation) or public (no reason unless you want it)?

Everything else we handle.

If anything above is unclear or you want to change it, tell me and I'll rework the plan before we touch anything.

---

*— James, on behalf of the James + Bono pair*
*Phase 393 / v52.0 Claude Workspace Restructure*
