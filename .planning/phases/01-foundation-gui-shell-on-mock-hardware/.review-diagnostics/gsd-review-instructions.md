## Review Instructions

**Verify against source — do not review the plan text in isolation.** The plans reference real files, configurations, and scripts in this repo.
1. Open the referenced files and check each claim against the actual code.
2. For every strength or concern, cite concrete `path/to/file:line` evidence plus the mechanism.
3. When a plan asserts a mechanism works (a guard, a metric, a test that exercises a path), trace whether it actually does what is claimed — do not take the plan's word for it.
4. If you cannot read the repo (no file access), say so and downgrade that finding to an open question rather than asserting it.

Findings citing `file:line` evidence are weighted far more heavily than impressionistic ones; a review that only restates the plan's own claims has low value.

**Plan coverage is mandatory.** Give every listed id in the Plan Coverage Manifest its own `##`-level section headed with that id verbatim (e.g. `## 01-01`, `## 01-02`, `## 01-03`).

Analyze each plan and provide:
1. **Summary** — One-paragraph assessment
2. **Strengths** — What's well-designed (bullet points citing file:line evidence)
3. **Concerns** — Potential issues, gaps, risks (bullet points with severity: HIGH/MEDIUM/LOW citing file:line evidence)
4. **Suggestions** — Specific improvements (bullet points)
5. **Risk Assessment** — Overall risk level (LOW/MEDIUM/HIGH) with justification

Focus on:
- Missing edge cases or error handling
- Dependency ordering issues across waves
- Scope creep or over-engineering
- Security considerations & least privilege
- Performance implications (memory budget ≤40MB RSS, non-blocking UI thread)
- Whether the plans actually achieve the phase goals and success criteria

Output your review in markdown format.

## Plan Coverage Manifest

Total plans in this review: 3

Plan ids (give each one its own `##`-level section, headed verbatim):
- 01-01
- 01-02
- 01-03
