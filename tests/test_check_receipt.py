"""Black-box checks for the local, single-command receipt wrapper."""

import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(os.environ.get(
    "CHECK_RECEIPT_UNDER_TEST",
    Path(__file__).resolve().parents[1] / "scripts" / "check_receipt.py"
))


class CheckReceiptTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.repo = self.base / "candidate"
        self.repo.mkdir()
        self.git("init", "-q")
        (self.repo / "source.txt").write_text("committed\n")
        self.git("add", "source.txt")
        self.git("-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                 "commit", "-qm", "initial")

    def git(self, *arguments):
        return subprocess.run(["git", "-C", str(self.repo), *arguments],
                              capture_output=True, check=True)

    def run_receipt(self, output, *command, extra=(), environment=None, cwd=None):
        return subprocess.run([sys.executable, str(SCRIPT), "--output", str(output),
                               "--cwd", str(self.repo if cwd is None else cwd),
                               *extra, "--", *command],
                              capture_output=True, text=True, env=environment)

    def receipt(self, output):
        return json.loads((output / "receipt.json").read_text())

    def test_success_records_literal_arguments_and_artifact_hashes(self):
        output = self.base / "receipt"
        script = ("import json, os, pathlib, sys; "
                  "out = pathlib.Path(sys.argv[1]); "
                  "started = json.loads((out / 'receipt.json').read_text()); "
                  "print(json.dumps({'argv': sys.argv[2:], "
                  "'env': os.environ['RECEIPT_TEST_TAG'], "
                  "'started': started['status'], "
                  "'pid': os.getpid(), 'group': os.getpgrp()})); "
                  "(out / 'result.txt').write_text('result bytes\\n'); "
                  "print('diagnostic', file=sys.stderr)")
        environment = os.environ.copy()
        environment["INHERITED_PRIVATE_TEST"] = "do-not-record-this-value"
        result = self.run_receipt(output, sys.executable, "-c", script, str(output),
                                  "two words", "$(literal)",
                                  extra=("--env", "RECEIPT_TEST_TAG=explicit value"),
                                  environment=environment)
        self.assertEqual(result.returncode, 0, result.stderr)
        receipt = self.receipt(output)
        self.assertEqual(receipt["schema_version"], 1)
        self.assertEqual(receipt["status"], "passed")
        self.assertEqual(receipt["child_returncode"], 0)
        self.assertIsInstance(receipt["child_pid"], int)
        self.assertEqual(receipt["process_group_id"], receipt["child_pid"])
        self.assertIsNotNone(receipt["child_started_at"])
        self.assertIsNotNone(receipt["child_ended_at"])
        self.assertGreaterEqual(receipt["child_duration_seconds"], 0)
        self.assertFalse(receipt["source_changed"])
        self.assertEqual(receipt["env_overrides"], {"RECEIPT_TEST_TAG": "explicit value"})
        self.assertNotIn("do-not-record-this-value", json.dumps(receipt))
        self.assertEqual(receipt["cwd"], str(self.repo))
        self.assertEqual(receipt["inputs_before"], receipt["inputs_after"])
        self.assertEqual(receipt["inputs_before"]["head"], self.git("rev-parse", "HEAD").stdout.decode().strip())
        self.assertEqual(receipt["inputs_before"]["committed_tree"],
                         self.git("rev-parse", "HEAD^{tree}").stdout.decode().strip())
        stdout = (output / "stdout.bin").read_bytes()
        printed = json.loads(stdout)
        self.assertEqual(printed["argv"], ["two words", "$(literal)"])
        self.assertEqual(printed["env"], "explicit value")
        self.assertEqual(printed["started"], "incomplete")
        self.assertEqual(printed["pid"], receipt["child_pid"])
        self.assertEqual(printed["group"], receipt["process_group_id"])
        self.assertEqual((output / "stderr.bin").read_bytes(), b"diagnostic\n")
        for name, expected in (("stdout.bin", stdout), ("result.txt", b"result bytes\n")):
            self.assertEqual(receipt["artifacts"][name]["size"], len(expected))
            self.assertEqual(receipt["artifacts"][name]["sha256"], hashlib.sha256(expected).hexdigest())
        source = (self.repo / "source.txt").read_bytes()
        self.assertEqual(receipt["inputs_before"]["working_files"]["source.txt"]["sha256"],
                         hashlib.sha256(source).hexdigest())
        self.assertIsNotNone(receipt["ended_at"])
        self.assertGreaterEqual(receipt["duration_seconds"], 0)

    def test_nonzero_exit_keeps_raw_failure_and_exact_child_status(self):
        output = self.base / "failure"
        result = self.run_receipt(output, sys.executable, "-c",
                                  "import sys; print('failed'); print('why', file=sys.stderr); sys.exit(7)")
        self.assertNotEqual(result.returncode, 0)
        receipt = self.receipt(output)
        self.assertEqual(receipt["status"], "command_failed")
        self.assertEqual(receipt["child_returncode"], 7)
        self.assertFalse(receipt["source_changed"])
        self.assertEqual((output / "stdout.bin").read_bytes(), b"failed\n")
        self.assertEqual((output / "stderr.bin").read_bytes(), b"why\n")

    def test_zero_exit_with_changed_tracked_source_is_rejected(self):
        output = self.base / "mutation"
        result = self.run_receipt(output, sys.executable, "-c",
                                  "from pathlib import Path; Path('source.txt').write_text('changed\\n')")
        self.assertNotEqual(result.returncode, 0)
        receipt = self.receipt(output)
        self.assertEqual(receipt["child_returncode"], 0)
        self.assertEqual(receipt["status"], "inputs_changed")
        self.assertTrue(receipt["source_changed"])
        self.assertNotEqual(receipt["inputs_before"]["working_files"],
                            receipt["inputs_after"]["working_files"])

    @unittest.skipUnless(os.name == "posix", "symlink snapshot uses POSIX files")
    def test_snapshot_distinguishes_deleted_file_and_replacement_symlink(self):
        deleted = self.base / "deleted"
        result = self.run_receipt(deleted, sys.executable, "-c",
                                  "from pathlib import Path; Path('source.txt').unlink()")
        self.assertNotEqual(result.returncode, 0)
        receipt = self.receipt(deleted)
        self.assertEqual(receipt["status"], "inputs_changed")
        self.assertEqual(receipt["inputs_before"]["working_files"]["source.txt"]["kind"], "file")
        self.assertEqual(receipt["inputs_after"]["working_files"]["source.txt"]["kind"], "missing")

        replaced = self.base / "replaced"
        script = ("from pathlib import Path; "
                  "Path('new.txt').write_text('new input'); "
                  "Path('source.txt').symlink_to('new.txt')")
        result = self.run_receipt(replaced, sys.executable, "-c", script)
        self.assertNotEqual(result.returncode, 0)
        receipt = self.receipt(replaced)
        self.assertEqual(receipt["status"], "inputs_changed")
        self.assertEqual(receipt["inputs_after"]["working_files"]["source.txt"]["kind"], "symlink")
        self.assertEqual(receipt["inputs_after"]["working_files"]["source.txt"]["target"], "new.txt")
        self.assertIn("new.txt", receipt["inputs_after"]["untracked"])

    def test_gitlink_is_rejected_before_command_dispatch(self):
        nested = self.repo / "subproject"
        nested.mkdir()
        subprocess.run(["git", "-C", str(nested), "init", "-q"], check=True, capture_output=True)
        (nested / "nested.txt").write_text("nested committed\n")
        subprocess.run(["git", "-C", str(nested), "add", "nested.txt"],
                       check=True, capture_output=True)
        subprocess.run(["git", "-C", str(nested), "-c", "user.name=Test",
                        "-c", "user.email=test@example.invalid", "commit", "-qm", "nested"],
                       check=True, capture_output=True)
        self.git("add", "subproject")
        self.assertTrue(self.git("ls-files", "--stage", "subproject").stdout.startswith(b"160000 "))
        (nested / "nested.txt").write_text("nested dirty\n")
        marker = self.base / "command-ran"
        output = self.base / "gitlink"
        script = ("from pathlib import Path; import sys; "
                  "Path('subproject/nested.txt').write_text('changed by command'); "
                  "Path(sys.argv[1]).write_text('ran')")
        result = self.run_receipt(output, sys.executable, "-c", script, str(marker))
        self.assertNotEqual(result.returncode, 0)
        receipt = self.receipt(output)
        self.assertEqual(receipt["status"], "snapshot_error")
        self.assertIn("gitlink", receipt["error"])
        self.assertIsNone(receipt["child_pid"])
        self.assertFalse(marker.exists())
        self.assertEqual((nested / "nested.txt").read_text(), "nested dirty\n")

    def test_untracked_nested_git_repo_is_rejected_before_command_dispatch(self):
        nested = self.repo / "untracked_project"
        nested.mkdir()
        subprocess.run(["git", "-C", str(nested), "init", "-q"], check=True, capture_output=True)
        (nested / "nested.txt").write_text("nested committed\n")
        subprocess.run(["git", "-C", str(nested), "add", "nested.txt"],
                       check=True, capture_output=True)
        subprocess.run(["git", "-C", str(nested), "-c", "user.name=Test",
                        "-c", "user.email=test@example.invalid", "commit", "-qm", "nested"],
                       check=True, capture_output=True)
        self.assertIn(b"untracked_project/\0",
                      self.git("ls-files", "--others", "--exclude-standard", "-z").stdout)
        (nested / "nested.txt").write_text("nested dirty\n")
        marker = self.base / "command-ran"
        output = self.base / "untracked-nested"
        script = ("from pathlib import Path; import sys; "
                  "Path('untracked_project/nested.txt').write_text('changed by command'); "
                  "Path(sys.argv[1]).write_text('ran')")
        result = self.run_receipt(output, sys.executable, "-c", script, str(marker))
        self.assertNotEqual(result.returncode, 0)
        receipt = self.receipt(output)
        self.assertEqual(receipt["status"], "snapshot_error")
        self.assertIn("source directory", receipt["error"])
        self.assertIsNone(receipt["child_pid"])
        self.assertFalse(marker.exists())
        self.assertEqual((nested / "nested.txt").read_text(), "nested dirty\n")

    def test_checkout_with_trailing_space_uses_its_own_source_snapshot(self):
        spaced_repo = self.base / "candidate "
        subprocess.run(["git", "clone", "-q", str(self.repo), str(spaced_repo)],
                       check=True, capture_output=True)
        output = self.base / "spaced-root"
        result = self.run_receipt(output, sys.executable, "-c",
                                  "from pathlib import Path; Path('source.txt').write_text('changed\\n')",
                                  cwd=spaced_repo)
        self.assertNotEqual(result.returncode, 0)
        receipt = self.receipt(output)
        self.assertEqual(receipt["checkout"], str(spaced_repo))
        self.assertEqual(receipt["cwd"], str(spaced_repo))
        self.assertEqual(receipt["child_returncode"], 0)
        self.assertEqual(receipt["status"], "inputs_changed")
        self.assertTrue(receipt["source_changed"])
        self.assertEqual((self.repo / "source.txt").read_text(), "committed\n")

    def test_dirty_unchanged_candidate_passes_and_index_only_change_fails(self):
        (self.repo / "source.txt").write_text("dirty candidate\n")
        (self.repo / "untracked.txt").write_text("input artifact\n")
        output = self.base / "dirty"
        result = self.run_receipt(output, sys.executable, "-c", "print('okay')")
        self.assertEqual(result.returncode, 0, result.stderr)
        receipt = self.receipt(output)
        self.assertEqual(receipt["status"], "passed")
        self.assertIn("untracked.txt", receipt["inputs_before"]["untracked"])
        self.assertEqual(receipt["inputs_before"], receipt["inputs_after"])

        staged = self.base / "staged"
        result = self.run_receipt(staged, "git", "add", "source.txt")
        self.assertNotEqual(result.returncode, 0)
        receipt = self.receipt(staged)
        self.assertEqual(receipt["status"], "inputs_changed")
        self.assertEqual(receipt["inputs_before"]["working_files"],
                         receipt["inputs_after"]["working_files"])
        self.assertNotEqual(receipt["inputs_before"]["index"], receipt["inputs_after"]["index"])

    def test_existing_output_refused_without_running_command(self):
        output = self.base / "same"
        first = self.run_receipt(output, sys.executable, "-c", "print('first')")
        self.assertEqual(first.returncode, 0, first.stderr)
        original = (output / "receipt.json").read_bytes()
        marker = self.base / "ran-again"
        second = self.run_receipt(output, sys.executable, "-c",
                                  "from pathlib import Path; import sys; Path(sys.argv[1]).write_text('ran')",
                                  str(marker))
        self.assertNotEqual(second.returncode, 0)
        self.assertFalse(marker.exists())
        self.assertEqual((output / "receipt.json").read_bytes(), original)

    @unittest.skipUnless(sys.platform.startswith("linux"), "process-group cleanup uses Linux")
    def test_timeout_keeps_failure_receipt_and_stops_child_group(self):
        output = self.base / "timeout"
        pid_file = self.base / "grandchild.pid"
        script = ("import pathlib, subprocess, sys, time; "
                  "child = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(30)']); "
                  "pathlib.Path(sys.argv[1]).write_text(str(child.pid)); "
                  "print('started', flush=True); time.sleep(30)")
        result = self.run_receipt(output, sys.executable, "-c", script, str(pid_file),
                                  extra=("--timeout", "1"))
        self.assertEqual(result.returncode, 124, result.stderr)
        receipt = self.receipt(output)
        self.assertEqual(receipt["status"], "timeout")
        self.assertNotEqual(receipt["child_returncode"], 0)
        self.assertEqual((output / "stdout.bin").read_bytes(), b"started\n")
        self.assertEqual(receipt["artifacts"]["stdout.bin"]["size"], len(b"started\n"))
        pid = int(pid_file.read_text())
        proc_state = Path(f"/proc/{pid}/stat")
        if proc_state.exists():
            self.assertEqual(proc_state.read_text().split()[2], "Z", "grandchild still runs")


if __name__ == "__main__":
    unittest.main()
