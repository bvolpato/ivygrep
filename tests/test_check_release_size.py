import subprocess
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "check_release_size.py"


class ReleaseSizeTest(unittest.TestCase):
    def test_accepts_binary_under_budget(self):
        with tempfile.TemporaryDirectory() as temp:
            binary = Path(temp) / "ig"
            binary.write_bytes(b"x" * 1024)
            subprocess.run(
                ["python3", str(SCRIPT), str(binary), "--max-mib", "1"],
                check=True,
                stdout=subprocess.PIPE,
                text=True,
            )

    def test_rejects_binary_over_budget(self):
        with tempfile.TemporaryDirectory() as temp:
            binary = Path(temp) / "ig"
            binary.write_bytes(b"x" * 2048)
            result = subprocess.run(
                ["python3", str(SCRIPT), str(binary), "--max-mib", "0.001"],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )
            self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
