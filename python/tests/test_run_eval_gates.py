import unittest

from python.evals.run_eval import evaluate_gates


class EvalGateTests(unittest.TestCase):
    def test_passes_when_meeting_thresholds(self):
        summary = {
            "first_pass_success_rate": 60.0,
            "success_within_max_attempts_rate": 90.0,
            "manifold_pass_rate": 97.0,
        }
        failures = evaluate_gates(
            summary=summary,
            baseline_summary=None,
            gate_first_pass=55.0,
            gate_success_within=88.0,
            gate_manifold=95.0,
            gate_max_drop_pp=3.0,
        )
        self.assertEqual(failures, [])

    def test_fails_when_under_threshold(self):
        summary = {
            "first_pass_success_rate": 50.0,
            "success_within_max_attempts_rate": 87.0,
            "manifold_pass_rate": 94.0,
        }
        failures = evaluate_gates(
            summary=summary,
            baseline_summary=None,
            gate_first_pass=55.0,
            gate_success_within=88.0,
            gate_manifold=95.0,
            gate_max_drop_pp=3.0,
        )
        self.assertEqual(len(failures), 3)

    def test_fails_on_regression_over_max_drop(self):
        summary = {
            "first_pass_success_rate": 60.0,
            "success_within_max_attempts_rate": 90.0,
            "manifold_pass_rate": 96.0,
        }
        baseline = {
            "first_pass_success_rate": 64.5,
            "success_within_max_attempts_rate": 94.5,
            "manifold_pass_rate": 99.5,
        }
        failures = evaluate_gates(
            summary=summary,
            baseline_summary=baseline,
            gate_first_pass=55.0,
            gate_success_within=88.0,
            gate_manifold=95.0,
            gate_max_drop_pp=3.0,
        )
        self.assertEqual(len(failures), 3)

    def test_allows_small_regression_within_budget(self):
        summary = {
            "first_pass_success_rate": 60.0,
            "success_within_max_attempts_rate": 90.0,
            "manifold_pass_rate": 96.0,
        }
        baseline = {
            "first_pass_success_rate": 62.0,
            "success_within_max_attempts_rate": 92.0,
            "manifold_pass_rate": 98.0,
        }
        failures = evaluate_gates(
            summary=summary,
            baseline_summary=baseline,
            gate_first_pass=55.0,
            gate_success_within=88.0,
            gate_manifold=95.0,
            gate_max_drop_pp=3.0,
        )
        self.assertEqual(failures, [])


if __name__ == "__main__":
    unittest.main()
