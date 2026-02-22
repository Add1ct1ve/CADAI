# Model Iteration Notes — Aiden Build123d 32B

Notes for the next fine-tuning iteration. Structured to avoid overfitting to specific test failures.

## Files

- **test-cases.md** — Specific prompts and results used as a regression suite. These validate whether capability gaps are fixed, but training should NOT be shaped to pass these exact tests.
- **capability-gaps.md** — Broad categories where the model is weak. Training data curation should target these general skills, not individual test failures.
- **code-mitigations.md** — Issues we've already fixed or can fix in CADAI's post-processing pipeline. Not training concerns.
