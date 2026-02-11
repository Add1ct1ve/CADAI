#!/usr/bin/env python3
"""Offline CAD generation evaluation harness.

This harness can evaluate either:
1) reference code shipped in eval cases, or
2) live generated code via an external generator command.

Case schema (JSON):
{
  "id": "whoop_housing",
  "category": "enclosure",
  "prompt": "...",
  "reference_code": "optional cadquery code"
}

Generator command mode:
  --generator-cmd "my-generator --prompt-file {prompt_file} --attempt {attempt}"
The command must print CadQuery Python code to stdout.

Metrics:
- first_pass_success_rate
- success_within_max_attempts_rate
- manifold_pass_rate
- median_time_s
- p95_time_s
"""

from __future__ import annotations

import argparse
import json
import os
import statistics
import subprocess
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple


@dataclass
class AttemptResult:
    success: bool
    manifold: bool
    duration_s: float
    error: Optional[str]


@dataclass
class CaseResult:
    case_id: str
    category: str
    attempts: int
    first_pass_success: bool
    success_within_max_attempts: bool
    manifold_pass: bool
    duration_s: float
    error: Optional[str]


def load_cases(cases_dir: Path) -> List[Dict[str, Any]]:
    files = sorted(cases_dir.glob("*.json"))
    cases = []
    for f in files:
        with f.open("r", encoding="utf-8") as fp:
            data = json.load(fp)
            data.setdefault("id", f.stem)
            data.setdefault("category", "uncategorized")
            cases.append(data)
    return cases


def run_cmd(args: List[str], cwd: Optional[Path] = None) -> Tuple[int, str, str]:
    p = subprocess.Popen(
        args,
        cwd=str(cwd) if cwd else None,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    out, err = p.communicate()
    return p.returncode, out, err


def run_generator(
    generator_cmd: str,
    prompt: str,
    attempt: int,
    workdir: Path,
) -> Tuple[Optional[str], Optional[str]]:
    with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as tf:
        tf.write(prompt)
        prompt_file = tf.name

    cmd = (
        generator_cmd.replace("{prompt_file}", prompt_file)
        .replace("{attempt}", str(attempt))
    )

    rc, out, err = run_cmd(["bash", "-lc", cmd], cwd=workdir)
    os.unlink(prompt_file)

    if rc != 0:
        return None, f"generator exit {rc}: {err.strip()[:400]}"

    code = out.strip()
    if not code:
        return None, "generator returned empty output"

    return code, None


def execute_code(
    python_bin: str,
    runner_py: Path,
    manufacturing_py: Path,
    code: str,
    workdir: Path,
) -> AttemptResult:
    start = time.time()
    with tempfile.TemporaryDirectory() as td:
        td_path = Path(td)
        code_file = td_path / "input.py"
        stl_file = td_path / "output.stl"
        code_file.write_text(code, encoding="utf-8")

        rc, out, err = run_cmd(
            [python_bin, str(runner_py), str(code_file), str(stl_file)],
            cwd=workdir,
        )
        if rc != 0:
            return AttemptResult(
                success=False,
                manifold=False,
                duration_s=time.time() - start,
                error=f"runner exit {rc}: {(err or out).strip()[:500]}",
            )

        rc2, out2, err2 = run_cmd(
            [python_bin, str(manufacturing_py), "mesh_check", str(code_file)],
            cwd=workdir,
        )
        if rc2 != 0:
            return AttemptResult(
                success=False,
                manifold=False,
                duration_s=time.time() - start,
                error=f"mesh_check exit {rc2}: {(err2 or out2).strip()[:500]}",
            )

        try:
            parsed = json.loads(out2.strip())
        except Exception as ex:
            return AttemptResult(
                success=False,
                manifold=False,
                duration_s=time.time() - start,
                error=f"mesh_check parse error: {ex}",
            )

        watertight = bool(parsed.get("watertight", False))
        winding = bool(parsed.get("winding_consistent", False))
        degenerate = int(parsed.get("degenerate_faces", 999999))
        euler = int(parsed.get("euler_number", 0))
        manifold = watertight and winding and degenerate == 0 and euler == 2

        return AttemptResult(
            success=True,
            manifold=manifold,
            duration_s=time.time() - start,
            error=None if manifold else f"non-manifold: {parsed.get('issues', [])}",
        )


def summarize(results: List[CaseResult]) -> Dict[str, Any]:
    if not results:
        return {
            "total_cases": 0,
            "evaluated_cases": 0,
            "first_pass_success_rate": 0.0,
            "success_within_max_attempts_rate": 0.0,
            "manifold_pass_rate": 0.0,
            "median_time_s": 0.0,
            "p95_time_s": 0.0,
        }

    first = sum(1 for r in results if r.first_pass_success)
    success = sum(1 for r in results if r.success_within_max_attempts)
    manifold = sum(1 for r in results if r.manifold_pass)
    times = [r.duration_s for r in results]
    times_sorted = sorted(times)

    def pct(values: List[float], p: float) -> float:
        if not values:
            return 0.0
        idx = int(round((len(values) - 1) * p))
        idx = max(0, min(idx, len(values) - 1))
        return values[idx]

    return {
        "total_cases": len(results),
        "evaluated_cases": len(results),
        "first_pass_success_rate": round(first / len(results) * 100, 2),
        "success_within_max_attempts_rate": round(success / len(results) * 100, 2),
        "manifold_pass_rate": round(manifold / len(results) * 100, 2),
        "median_time_s": round(statistics.median(times) if times else 0.0, 3),
        "p95_time_s": round(pct(times_sorted, 0.95), 3),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cases-dir", default="python/evals/cases")
    parser.add_argument("--python-bin", default="python")
    parser.add_argument("--runner", default="python/runner.py")
    parser.add_argument("--manufacturing", default="python/manufacturing.py")
    parser.add_argument("--generator-cmd", default=None)
    parser.add_argument("--max-attempts", type=int, default=4)
    parser.add_argument("--out", default="python/evals/last_eval_summary.json")
    args = parser.parse_args()

    workdir = Path.cwd()
    cases = load_cases(Path(args.cases_dir))

    print(f"Loaded {len(cases)} eval cases from {args.cases_dir}")

    results: List[CaseResult] = []

    for case in cases:
        case_id = str(case.get("id", "unknown"))
        category = str(case.get("category", "uncategorized"))
        prompt = str(case.get("prompt", "")).strip()
        reference_code = case.get("reference_code")

        if not prompt:
            print(f"[SKIP] {case_id}: empty prompt")
            continue

        start_case = time.time()
        first_pass_success = False
        success_within = False
        manifold_pass = False
        last_error: Optional[str] = None
        attempts_used = 0

        for attempt in range(1, max(1, args.max_attempts) + 1):
            attempts_used = attempt

            if args.generator_cmd:
                code, gen_err = run_generator(args.generator_cmd, prompt, attempt, workdir)
                if gen_err:
                    last_error = gen_err
                    continue
            else:
                if not reference_code:
                    last_error = "no generator command and no reference_code"
                    break
                code = str(reference_code)

            attempt_result = execute_code(
                args.python_bin,
                Path(args.runner),
                Path(args.manufacturing),
                code,
                workdir,
            )

            if attempt == 1 and attempt_result.success and attempt_result.manifold:
                first_pass_success = True

            if attempt_result.success and attempt_result.manifold:
                success_within = True
                manifold_pass = True
                last_error = None
                break

            last_error = attempt_result.error

        duration = time.time() - start_case

        result = CaseResult(
            case_id=case_id,
            category=category,
            attempts=attempts_used,
            first_pass_success=first_pass_success,
            success_within_max_attempts=success_within,
            manifold_pass=manifold_pass,
            duration_s=duration,
            error=last_error,
        )
        results.append(result)

        status = "PASS" if success_within else "FAIL"
        print(f"[{status}] {case_id} ({category}) attempts={attempts_used} time={duration:.2f}s")

    summary = summarize(results)
    detailed = {
        "summary": summary,
        "results": [r.__dict__ for r in results],
    }

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(detailed, indent=2), encoding="utf-8")

    print("\nEvaluation summary")
    print(json.dumps(summary, indent=2))
    print(f"\nSaved detailed report to {out_path}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
