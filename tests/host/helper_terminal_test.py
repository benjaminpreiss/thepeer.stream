"""PTY capture regressions. Synthetic terminal markers only; no live capabilities."""
import os
import subprocess
import sys
import unittest
from unittest.mock import patch

from helper_process_tests import wait_for_terminal


ENTER = b'\x1b[?1049h'
LEAVE = b'\x1b[?1049l'


class TerminalCaptureTests(unittest.TestCase):
    def capture_exited(self, output, output_predicate=False, read_size=65536):
        master, slave = os.openpty()
        capture = bytearray()
        child = None
        read = os.read
        try:
            child = subprocess.Popen(
                [sys.executable, '-c', f'import os; os.write(1, {output!r})'],
                stdin=subprocess.DEVNULL, stdout=slave, stderr=subprocess.DEVNULL,
            )
            # Keep our slave open so macOS does not discard unread output on
            # last-slave close. Linux can retain it, which exposes the CI race.
            # Deterministically model a reader scheduled after the helper exits.
            self.assertEqual(child.wait(timeout=5), 0)
            predicate = (lambda: LEAVE in capture) if output_predicate else (
                lambda: child.poll() is not None)
            with patch('helper_process_tests.os.read',
                       side_effect=lambda fd, size: read(fd, min(size, read_size))):
                wait_for_terminal(master, child, capture, predicate)
            return capture
        finally:
            os.close(slave)
            os.close(master)
            if child is not None and child.poll() is None:
                child.kill()
                child.wait(timeout=5)

    def test_exit_before_wait_still_captures_terminal_restoration(self):
        capture = self.capture_exited(ENTER + LEAVE)
        self.assertTrue(ENTER in capture and LEAVE in capture,
                        'alternate screen not restored')

    def test_exit_drain_reads_all_chunks_before_checking_output_predicate(self):
        capture = self.capture_exited(ENTER + LEAVE, output_predicate=True, read_size=3)
        self.assertEqual(capture, ENTER + LEAVE)

    def test_missing_restoration_is_not_accepted(self):
        with self.assertRaisesRegex(AssertionError, 'terminal expectation failed'):
            self.capture_exited(ENTER, output_predicate=True)


if __name__ == '__main__':
    unittest.main()
