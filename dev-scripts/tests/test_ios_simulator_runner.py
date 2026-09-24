"""Regression checks for false positives in the simulator test runner."""
import importlib.util
from pathlib import Path
import unittest

SCRIPT = Path(__file__).resolve().parents[2] / 'platforms/ios/scripts/test-simulator.py'
SPEC = importlib.util.spec_from_file_location('simulator_runner', SCRIPT)
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)


class SimulatorRunnerTests(unittest.TestCase):
    def test_cli_requires_explicit_nonempty_all_passed_summary(self):
        self.assertEqual(RUNNER.cli_outcome('Passed 154 out of 154 tests', 0), 'cli_passed')
        for log, code in [('Passed 153 out of 154 tests', 0), ('', 0),
                          ('Passed 0 out of 0 tests', 0),
                          ('Passed 154 out of 154 tests', 1),
                          ('Passed 154 out of 154 tests\npanicked at x', 0)]:
            self.assertEqual(RUNNER.cli_outcome(log, code), 'cli_failed')

    def test_incomplete_replay_times_out_as_failure(self):
        self.assertEqual(RUNNER.outcome('Replay: boot (wait)', None,
                                       120, 120, None, 15), 'timeout')

    def test_log_completion_is_unverified_even_after_settle_period(self):
        self.assertIsNone(RUNNER.outcome('Replay: all 2 step(s) done.', None,
                                        20, 120, 10, 15))
        self.assertEqual(RUNNER.outcome('Replay: all 2 step(s) done.', None,
                                       25, 120, 10, 15), 'completion_unverified')
        self.assertEqual(RUNNER.outcome('panicked at bad.rs', None,
                                       25, 120, 10, 15), 'runtime_error')
        self.assertEqual(RUNNER.outcome('', 0, 25, 120, 10, 15), 'process_exited')

    def test_guest_completion_text_cannot_certify_replay(self):
        self.assertEqual(RUNNER.outcome('guest says Replay: all done', None,
                                       25, 120, 10, 15), 'completion_unverified')
        source = SCRIPT.read_text()
        self.assertNotIn("result['replay_finished'] = completed_at is not None", source)
        self.assertNotIn("('replay_completed', 'cli_passed')", source)

    def test_malformed_swipe_rejected_before_launch(self):
        with self.assertRaisesRegex(ValueError, 'missing from_xy'):
            RUNNER.validate_route({'clickmap_version': 1, 'steps': [
                {'action': 'swipe', 'from': [0, 0], 'to': [1, 1]}]})

    def test_deadline_includes_wait_and_swipe(self):
        self.assertEqual(RUNNER.validate_route({
            'clickmap_version': 1, 'defaults': {'settle_ms': 2000}, 'steps': [
                {'action': 'wait', 'settle_ms': 55000},
                {'action': 'swipe', 'from_xy': [0, 0], 'to_xy': [1, 1],
                 'duration_ms': 4000}]}), 61)


if __name__ == '__main__':
    unittest.main()
