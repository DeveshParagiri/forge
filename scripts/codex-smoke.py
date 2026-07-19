#!/usr/bin/env python3
"""Personal: smoke-test ChatGPT Codex OAuth token against the Codex backend.

Uses ~/.codex/auth.json only (never prints secrets). Exit 0 on success.

  python3 scripts/codex-smoke.py
  python3 scripts/codex-smoke.py --model gpt-5.6-luna
"""

from __future__ import annotations

import argparse
import json
import sys
import urllib.error
import urllib.request
from pathlib import Path


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--model", default="gpt-5.6-sol")
    args = ap.parse_args()

    auth_path = Path.home() / ".codex" / "auth.json"
    if not auth_path.is_file():
        print(f"FAIL: missing {auth_path} — run `codex login`", file=sys.stderr)
        return 1

    auth = json.loads(auth_path.read_text())
    tokens = auth.get("tokens") or {}
    tok = tokens.get("access_token")
    account = tokens.get("account_id")
    if not tok or not account:
        print("FAIL: auth.json has no tokens.access_token / account_id", file=sys.stderr)
        return 1

    body = {
        "model": args.model,
        "stream": True,
        "store": False,
        "input": [
            {
                "role": "user",
                "content": [{"type": "input_text", "text": "Reply with exactly: codex-ok"}],
            }
        ],
    }
    req = urllib.request.Request(
        "https://chatgpt.com/backend-api/codex/responses",
        data=json.dumps(body).encode(),
        headers={
            "Authorization": f"Bearer {tok}",
            "Content-Type": "application/json",
            "Accept": "text/event-stream",
            "ChatGPT-Account-Id": account,
            "OpenAI-Beta": "responses=experimental",
            "originator": "codex_cli_rs",
            "User-Agent": "codex_cli_rs",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=90) as resp:
            raw = resp.read(4000).decode("utf-8", "replace")
            if resp.status != 200 or "response.created" not in raw:
                print(f"FAIL: unexpected response status={resp.status}", file=sys.stderr)
                print(raw[:400], file=sys.stderr)
                return 1
    except urllib.error.HTTPError as e:
        print(f"FAIL: HTTP {e.code}: {e.read()[:400].decode()}", file=sys.stderr)
        return 1
    except Exception as e:
        print(f"FAIL: {type(e).__name__}: {e}", file=sys.stderr)
        return 1

    print(f"OK: Codex backend accepts model={args.model} (token from ~/.codex/auth.json)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
