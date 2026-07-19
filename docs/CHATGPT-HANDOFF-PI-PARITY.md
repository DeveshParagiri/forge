# Grok Build personal fork — ChatGPT handoff: Pi-parity multi-provider agent

**Audience:** Another coding agent (ChatGPT / Codex / Claude) implementing remaining work.  
**Owner machine:** Dev’s personal `grok-build` fork on branch `dev`.  
**Date context:** 2026-07-18.

This doc is the single handoff. Do **not** invent a new multi-provider architecture from scratch without reading **Upstream updates** and **Already done**.

---

## 0. Paths

Claude Code lives under user-local bins + versioned share:

| Role | Claude | Grok (now aligned) |
|------|--------|---------------------|
| PATH binary | `~/.local/bin/claude` | `~/.grok/bin/grok` (canonical personal build) |
| Versioned payload | `~/.local/share/claude/versions/<ver>` | Compatibility symlinks point directly to `~/.grok/bin/grok` |
| Config / state home | `~/.claude/` | `~/.grok/` |
| Source fork | n/a | `~/.local/share/grok/source` (branch **`dev`**) |
| Rebuild script | n/a | `~/bin/grok-update-from-source` |

Canonical install:

```text
~/.grok/bin/grok               # only installed executable; personal extended build
~/.local/bin/grok              → ~/.grok/bin/grok
~/.local/share/grok/versions/current → ~/.grok/bin/grok
~/.grok/local/grok             → ~/.grok/bin/grok
~/bin/grok                     → ~/.grok/bin/grok
```

Do not install another copy elsewhere. The compatibility paths are direct symlinks only.

---

## 1. Upstream updates (must not break)

### Source of truth

| Item | Value |
|------|--------|
| Upstream repo | https://github.com/xai-org/grok-build |
| Local remote | `upstream` → `https://github.com/xai-org/grok-build.git` |
| Personal branch | `dev` |
| Stock product docs (user guide) | **`~/.grok/docs/user-guide/`** (shipped with Grok; also in-product via help) |
| Official custom models guide | User guide **`11-custom-models.md`** under that tree |
| Auth docs | **`02-authentication.md`** |
| Config docs | **`05-configuration.md`** |

### How to pull upstream safely

```bash
~/bin/grok-update-from-source
# or manually:
cd ~/.local/share/grok/source
git fetch upstream
git checkout dev
git rebase upstream/main
# fix conflicts only in Personal: hooks; re-apply new files if dropped
cargo build -p xai-grok-pager-bin --release
install -m 755 target/release/xai-grok-pager ~/.grok/bin/grok
ln -sfn ~/.grok/bin/grok ~/.local/bin/grok
```

### Patch rules (mandatory)

1. Prefer **new files** for bulk logic (`// Personal:` modules).
2. Touch upstream files only with thin hooks marked `// Personal:` or `/// Personal:`.
3. Never rewrite stock SpaceXAI welcome `Action::Login` cold-start path.
4. After rebase: `rg -n 'Personal:' crates` and run `python3 scripts/codex-smoke.py`.
5. Document every new personal module in `FORK-MAINTENANCE.md`.

### What stock Grok already supports (docs)

From **`~/.grok/docs/user-guide/11-custom-models.md`**:

- `[model.*]` with `base_url`, `api_backend` (`chat_completions` | `responses` | …), `api_key` / `env_key`
- Official OpenAI path is **`api.openai.com` + `OPENAI_API_KEY`** (usage-based), **not** ChatGPT subscription OAuth
- ChatGPT / Codex subscription OAuth is **not** first-class in stock docs

SpaceXAI / Grok subscription: **`grok login`** / welcome login → `~/.grok/auth.json` (see **`02-authentication.md`**).

---

## 2. Product goal (what Dev wants)

### Auth / providers

| Provider | Auth style | Models surface in `/model` |
|----------|------------|----------------------------|
| **SpaceXAI** | Subscription OAuth (`grok login` / `/login spacexai`) | Stock Grok models (`grok-4.5`, etc.) — **include/exclude** list |
| **OpenAI Codex** | ChatGPT Plus/Pro OAuth via **`~/.codex/auth.json`** (reuse Codex CLI login) | Sol / Terra / Luna (and future Codex catalog) — **include/exclude** |
| **OpenRouter** | API key (`OPENROUTER_API_KEY` or `~/.grok/provider_keys.json`) | Curated **latest** OpenRouter model IDs — **include/exclude** |

### Catalog control

- User can **include/exclude** models per provider (config.toml and/or UI).
- `/model` lists only **enabled + authenticated** models (or enabled with “login required” badge).
- Default remains a SpaceXAI model unless user picks otherwise.

### Mid-session provider switch

Must work **without** full context loss:

- Keep user / assistant text / tool calls.
- **Do not** replay foreign `reasoning` + `encrypted_content` (`rs_*`) across backends (Codex 400 decrypt).
- Match **Pi** transform semantics (see §4).

### Quality bar

- Sol chat works end-to-end (tools + reasoning streams eventually).
- No false `empty_response` retries that duplicate assistant text.
- Upstream rebase still possible.

---

## 3. Already done on this machine (do not redo blindly)

| Area | Status | Notes |
|------|--------|--------|
| Exaforge theme, Shift+Tab = effort | Done | Personal commits |
| `[provider.codex]` + Sol/Terra/Luna in `~/.grok/config.toml` | Done | `auth = "codex"` |
| `[provider.openrouter]` + a few models | Done | Needs include/exclude + fresher list |
| Read `~/.codex/auth.json` for token + account id | Done | `provider_auth.rs` |
| Never use Grok OIDC on third-party bases | Partial | `reconstruct_full_config` + gate |
| Codex body sanitize (Pi-like whitelist) | Done | `sanitize_body_for_codex_backend` in sampler |
| `/login` multi-provider picker | Done | SpaceXAI / Codex / OpenRouter |
| Codex smoke script | Done | `scripts/codex-smoke.py` |
| Binary install | Done | One extended binary at `~/.grok/bin/grok`; alternate paths are symlinks |
| Mid-session history transform on provider switch | **Not done** | Causes encrypted_content 400 |
| empty_response retry false positives on Codex | **Not done** | Duplicates / “Retrying attempt 2” |
| Formal include/exclude catalog UI | **Not done** | Config-only today |
| OpenRouter “latest models” curated pack | Partial | Expand + document |

Verified smoke:

```bash
python3 ~/.local/share/grok/source/scripts/codex-smoke.py
# OK: Codex backend accepts model=gpt-5.6-sol
```

Known good Codex contract (live probes):

- `store: false`, `stream: true` required  
- **Forbidden:** `temperature`, `top_p`, `max_output_tokens`, `truncation`, `stream_tool_calls`, `metadata`, `background`  
- System content → **`instructions`**, not `role: system` in `input` (Pi style)  
- Tools + reasoning **allowed**  
- Encrypted reasoning only valid if **minted by same Codex account/session**

---

## 4. How Pi does multi-provider (in depth) — implement against this

Reference implementation: https://github.com/earendil-works/pi  
Installed package on this machine: `@earendil-works/pi-coding-agent` / `@earendil-works/pi-ai`.

### 4.1 Auth model

| Concept | Pi |
|---------|-----|
| Command | `/login`, `/logout` |
| Subscriptions (OAuth) | ChatGPT Codex, Claude Pro/Max, Copilot, xAI, … |
| API keys | env or `~/.pi/agent/auth.json` |
| Docs | https://pi.dev/docs/latest/providers |

ChatGPT path:

- OAuth → store `type/access/refresh/expires/accountId` under `openai-codex`
- Requires ChatGPT Plus/Pro
- Official “Codex for OSS” stance from OpenAI

### 4.2 Dedicated Codex transport (critical)

Pi does **not** reuse a generic OpenAI client for ChatGPT sub.

| Piece | Pi |
|-------|-----|
| Provider id | `openai-codex` |
| Base URL | `https://chatgpt.com/backend-api` |
| Endpoint | `…/codex/responses` |
| Module | `openai-codex-responses` (SSE + **WebSocket** auto) |
| Headers | `Authorization: Bearer <jwt>`, `chatgpt-account-id`, `OpenAI-Beta: responses=experimental`, `originator: pi` |
| Body | `store:false`, `stream:true`, `instructions`, `input`, `tools`, `reasoning: { effort, summary: "auto" }`, `include: ["reasoning.encrypted_content"]` |
| Streams | Maps `reasoning_*` deltas + `function_call_arguments.*` + `output_text.*` into UI |

**Grok must keep a Codex-specific sanitize (or dedicated client), not dump full xAI CreateResponse JSON at Codex.**

### 4.3 Model switch mid-session (context not wiped)

Pi `setModel`:

- Does **not** clear the session transcript.
- Saves provider/model as defaults.
- On **next** request, transforms history:

From `transform-messages` / Responses conversion (behavior):

| Block | Same provider+api+model | Different model/provider |
|-------|-------------------------|---------------------------|
| User text | keep | keep |
| Assistant text | keep | keep (plain text) |
| Tool calls | keep (IDs maybe normalized) | keep with cross-provider ID fixes |
| Thinking + **signature / encrypted** | replay | **drop** or convert to plain text; **never** replay foreign crypto |
| Redacted thinking | only if same model | drop |

Pi comment (paraphrased): *redacted/opaque thinking only valid for same model — drop cross-model to avoid API errors.*

**Grok target:** On provider change (SpaceXAI ↔ Codex ↔ OpenRouter), strip non-portable reasoning items; keep user/assistant/tools. Optional toast: “Cleared non-portable reasoning for new provider.”

### 4.4 `/model` and scoped models

| Command | Behavior |
|---------|----------|
| `/model` | Full picker of available models (auth + catalog) |
| `/scoped-models` | Subset for Ctrl+P cycle |
| `--models` CLI | Restrict cycle set |

Grok target: include/exclude lists per provider + `/model` shows only enabled entries (with auth status).

### 4.5 Other Pi features (for optional parity backlog)

Depth is for prioritization; not all are required for Dev’s MVP.

| Feature | How Pi does it | Grok stock? | Priority for Dev |
|---------|----------------|-------------|------------------|
| `/login` multi-provider | Interactive OAuth/API key picker | Personal partial | **P0** polish |
| Mid-session model switch | History transform, no wipe | Broken on reasoning crypto | **P0** |
| Codex tools + reasoning streams | Dedicated event map | Partial | **P0** |
| empty_response handling | N/A / different | False retries on Codex | **P0** |
| `/scoped-models` | Enable cycle subset | No | **P1** = include/exclude |
| OpenRouter catalog | API key provider + models.json | Partial config | **P1** |
| `/tree` session branching | JSONL tree | Different session model | P2 |
| `/fork` `/clone` | New session file | Has fork worktree | P2 |
| Extensions / packages | TS packages | Plugins/skills | P2 |
| Compaction | Manual + auto | Yes (stock) | — |
| Themes | Hot-reload | Themes (personal Exaforge theme) | — |
| Transport SSE/WS for Codex | auto WS+SSE | SSE only | P2 (WS optional) |

---

## 5. Spec: include/exclude models + `/model`

### 5.1 Config shape (proposal)

Extend `~/.grok/config.toml` (personal; document in `FORK-MAINTENANCE.md`):

```toml
# --- SpaceXAI (subscription OAuth; stock models filtered) ---
[catalog.spacexai]
# empty include = all stock selectable models
include = ["grok-4.5", "grok-4.5-build"]   # example
# exclude = ["some-internal-model"]

# --- OpenAI Codex (ChatGPT OAuth via ~/.codex) ---
[catalog.openai_codex]
include = ["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna"]
# exclude = []

# --- OpenRouter (API key) ---
[catalog.openrouter]
# Prefer include list of full OpenRouter IDs
include = [
  "openrouter/auto",
  "anthropic/claude-sonnet-4",
  "anthropic/claude-opus-4",
  "openai/gpt-5.2",
  "openai/gpt-5.6-sol",
  "google/gemini-2.5-pro",
  "google/gemini-2.5-flash",
  "deepseek/deepseek-r1",
  "qwen/qwen3-coder",
]
# exclude wins over include if both set
# exclude = []

[provider.openrouter]
name = "OpenRouter"
base_url = "https://openrouter.ai/api/v1"
api_backend = "chat_completions"
auth = "openrouter"   # OPENROUTER_API_KEY + ~/.grok/provider_keys.json
```

**Resolution order for `/model` listing:**

1. For each provider pack that is **enabled** (section present or feature flag).
2. If auth missing → still show models with status `login required` **or** hide until login (pick one; recommend show with badge).
3. Apply `include` (if non-empty) then `exclude`.
4. Sort: SpaceXAI defaults first, then Codex, then OpenRouter (or group by provider in UI).

### 5.2 `/login` (already roughly present)

```text
/login                 → picker: SpaceXAI | OpenAI Codex | OpenRouter
/login spacexai        → stock OAuth
/login codex           → status of ~/.codex; spawn `codex login` if missing
/login openrouter      → freeform API key → ~/.grok/provider_keys.json
```

### 5.3 OpenRouter “latest models” starter list (refresh from openrouter.ai/models)

Treat as **curated defaults**, not exhaustive. Implementers should prefer live `GET https://openrouter.ai/api/v1/models` when key present, with this fallback include list:

| OpenRouter id (examples, mid‑2026) | Notes |
|------------------------------------|--------|
| `openrouter/auto` | Router |
| `anthropic/claude-sonnet-4` | Coding workhorse |
| `anthropic/claude-opus-4` / fable if available | Heavy |
| `openai/gpt-5.2` | GPT via OR |
| `openai/gpt-5.6-sol` / `terra` / `luna` | If listed on OR |
| `google/gemini-2.5-pro` | Long context |
| `google/gemini-2.5-flash` | Fast |
| `deepseek/deepseek-r1` | Reasoning |
| `qwen/qwen3-coder` | Coding |
| `meta-llama/llama-4-maverick` | If available |

User can expand `catalog.openrouter.include` without code changes.

### 5.4 `/model` behavior

- Lists **display name**, **provider**, **auth status** (✓ / login / key missing).
- Selecting a model sets session model + sampling config (existing ACP `set_session_model` path).
- On **provider change** (base_url / auth pack differs): run **history transform** (§4.3) before next turn.

---

## 6. Implementation backlog for ChatGPT (ordered)

### P0 — Correctness

1. **Provider-switch history transform**  
   When `provider(old) != provider(new)`: strip all `ConversationItem::Reasoning` (and any input items with `encrypted_content` / `rs_*`) from session chat state. Keep user/assistant/tool. Log toast.

2. **Codex empty_response retries**  
   If stream already emitted user-visible text, do not classify as `no_visible_content` and retry (fixes duplicate “Hi!” + “Retrying attempt 2”).

3. **Ensure `/model` catalog** is driven by `catalog.*` include/exclude + auth, not only ad-hoc `[model.*]` blocks.

4. **Regression tests:**  
   - sanitize body unit tests (exist)  
   - switch Grok→Sol with prior reasoning → no encrypted_content in next request  
   - `scripts/codex-smoke.py` still green  

### P1 — Catalog UX

5. Config schema for `catalog.spacexai|openai_codex|openrouter`  
6. Optional slash: `/models enable|disable <id>` writing config  
7. Live OpenRouter model fetch when key present (cache under `~/.grok/cache/`)  
8. Document in `FORK-MAINTENANCE.md` + link to stock **11-custom-models.md**

### P2 — Polish

9. Codex WebSocket transport (Pi) — optional  
10. Richer reasoning summary UI from Codex events  
11. Multi-account Codex (Pi multi-pass style)  

---

## 7. Files to know (personal fork)

| Path | Role |
|------|------|
| `~/.local/share/grok/source/FORK-MAINTENANCE.md` | Rebase rules + personal architecture |
| `~/.local/share/grok/source/docs/CHATGPT-HANDOFF-PI-PARITY.md` | **This handoff** |
| `crates/.../xai-grok-shell/src/agent/provider_auth.rs` | Codex/OpenRouter credentials |
| `crates/.../xai-grok-shell/src/agent/config.rs` | Provider packs, resolve_credentials |
| `crates/.../xai-grok-sampler/src/client.rs` | `sanitize_body_for_codex_backend` |
| `crates/.../xai-grok-pager/src/app/dispatch/provider_login.rs` | `/login` picker |
| `crates/.../session/acp_session_impl/sampler_turn.rs` | Third-party no OIDC |
| `~/.grok/config.toml` | User models/providers |
| `~/.grok/docs/user-guide/` | **Stock Grok documentation** |
| `scripts/codex-smoke.py` | Live Codex token check |

---

## 8. Acceptance criteria

- [ ] `~/.local/bin/grok` runs personal build (Claude-like path).  
- [ ] `~/bin/grok-update-from-source` still rebases onto upstream without discarding personal modules.  
- [ ] SpaceXAI, Codex, OpenRouter each configurable for include/exclude.  
- [ ] `/model` shows only allowed models with auth state.  
- [ ] Mid-session switch SpaceXAI ↔ Sol does **not** 400 on encrypted reasoning; prior user/assistant/tool context remains.  
- [ ] Sol “hi” does not double-print via empty_response retries.  
- [ ] `python3 scripts/codex-smoke.py` exits 0.  
- [ ] FORK-MAINTENANCE.md updated for any new files.

---

## 9. Links (bookmark for implementer)

| Resource | URL / path |
|----------|------------|
| Upstream Grok Build | https://github.com/xai-org/grok-build |
| Stock user guide (local) | `~/.grok/docs/user-guide/` especially `02-authentication.md`, `05-configuration.md`, **`11-custom-models.md`** |
| Pi repo | https://github.com/earendil-works/pi |
| Pi providers docs | https://pi.dev/docs/latest/providers |
| OpenRouter models | https://openrouter.ai/models |
| OpenAI Codex OSS note | https://developers.openai.com/community/codex-for-oss (linked from Pi docs) |

---

## 10. Explicit non-goals

- Do not replace SpaceXAI subscription with OpenRouter for default.  
- Do not claim stock Grok supports ChatGPT OAuth without personal patches.  
- Do not force-push or rewrite published personal history.  
- Do not install system packages without user approval (per Dev env rules).

---

*End of handoff. Implement P0 first; paste this file into ChatGPT as the project brief.*
