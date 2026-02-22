# Code-Side Mitigations

Issues already handled in CADAI's pipeline, or improvements we can make on the code side. Not training concerns.

---

## Already Fixed

| Issue | Fix | Where |
|-------|-----|-------|
| Hallucinated API calls | `strip_unknown_calls()` introspects build123d namespace + AST-walks user-defined names | `runner.py` |
| User-defined function false positives | `_collect_code_defined_names()` collects `def`, `class`, assignments, imports, for-targets, with-as | `runner.py` |
| Vector attribute case (`v.x` vs `v.X`) | Changed to `v.X` | `manufacturing.py` |
| RunPod 500 errors on parallel gen | Added HTTP 500 to transient retry list with exponential backoff | `retry.rs` (10d496a) |
| Disconnected solids detection | `SPLIT_BODY` exit code 5, triggers post-geometry retry | `runner.py` / `validate.rs` |
| Silent fillet failure | Safe fillet wrapper catches exception and falls back to `pass` | `runner.py` |

## Potential Improvements

| Idea | Impact | Effort |
|------|--------|--------|
| `compile()` pre-check in static validator | Catches syntax errors before burning execution + retry cycles | Low |
| Stderr warning when fillet fallback triggers | User sees "fillets were skipped" instead of wondering why edges are sharp | Low |
| Semantic geometry validation (e.g. "is this hollow?") | Could catch functionally-wrong-but-structurally-valid parts like solid keycap | High — needs part intent from prompt |
