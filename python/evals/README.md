# CAD Eval Harness

Run offline evaluation against the case set:

```bash
python python/evals/run_eval.py \
  --cases-dir python/evals/cases \
  --python-bin python \
  --runner python/runner.py \
  --manufacturing python/manufacturing.py \
  --max-attempts 4
```

To evaluate live generation from an external generator adapter:

```bash
python python/evals/run_eval.py \
  --cases-dir python/evals/cases \
  --generator-cmd "./tools/generate_code.sh {prompt_file} {attempt}" \
  --max-attempts 4
```

`generator-cmd` must print Build123d Python code to stdout.

Output report is written to `python/evals/last_eval_summary.json`.
