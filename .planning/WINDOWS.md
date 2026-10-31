---
schema_version: 1
open_count: 2
waived_count: 0
fixed_count: 0
total_count: 2
last_updated: 2026-10-31T17:35:27.771Z
---

# Broken Windows Ledger

> Cross-phase defect register. With `workflow.windows_enforce` enabled, `/gsd-ship` blocks while `open_count > 0`.
> Waive with `gsd-tools windows waive <id> "<reason>"` (reason required).
> Mark fixed with `gsd-tools windows fixed <id>`.

| id | phase | kind | file | line | description | status | reason | recorded_at | resolved_at |
|----|-------|------|------|------|-------------|--------|--------|-------------|-------------|
| 1 | 01 | stub | ui/app.slint |  | Centred placeholder Text in AppWindow; the shell, header, status strip and six screens replace it in plan 01-02 (intentional, plan-specified). | open |  | 2026-10-31T17:35:26.553Z |  |
| 2 | 01 | unrun-verify | .github/workflows/ci.yml |  | CI workflow has not executed on GitHub Actions yet; every gate was reproduced locally on 2026-10-31. | open |  | 2026-10-31T17:35:27.771Z |  |

````json
[
  {
    "id": 1,
    "kind": "stub",
    "phase": "01",
    "file": "ui/app.slint",
    "line": null,
    "description": "Centred placeholder Text in AppWindow; the shell, header, status strip and six screens replace it in plan 01-02 (intentional, plan-specified).",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-10-31T17:35:26.553Z",
    "resolved_at": null,
    "milestone": null
  },
  {
    "id": 2,
    "kind": "unrun-verify",
    "phase": "01",
    "file": ".github/workflows/ci.yml",
    "line": null,
    "description": "CI workflow has not executed on GitHub Actions yet; every gate was reproduced locally on 2026-10-31.",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-10-31T17:35:27.771Z",
    "resolved_at": null,
    "milestone": null
  }
]
````
