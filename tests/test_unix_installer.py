import hashlib
import os
import stat
import subprocess
import tarfile
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
TAG = "v9.9.9"


class UnixInstallerTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.release_dir = self.root / "release"
        self.install_dir = self.root / "install"
        self.fake_bin = self.root / "bin"
        self.release_dir.mkdir()
        self.install_dir.mkdir()
        self.fake_bin.mkdir()
        self.write_executable(self.fake_bin / "ldconfig", "#!/bin/sh\nexit 1\n")
        self.write_executable(
            self.fake_bin / "curl",
            """#!/bin/sh
set -eu
head=0
out=""
url=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        -o)
            out=$2
            shift 2
            ;;
        -w)
            shift 2
            ;;
        -*I*)
            head=1
            shift
            ;;
        -*)
            shift
            ;;
        *)
            url=$1
            shift
            ;;
    esac
done
case "$url" in
    */releases/latest)
        printf '%s\\n' "https://github.com/bvolpato/ivygrep/releases/tag/v9.9.9"
        exit 0
        ;;
esac
file=${url##*/}
src="$IVYGREP_FAKE_RELEASE_DIR/$file"
if [ ! -f "$src" ]; then
    exit 22
fi
if [ "$head" -eq 1 ]; then
    exit 0
fi
if [ -z "$out" ]; then
    cat "$src"
else
    cp "$src" "$out"
fi
""",
        )
        self.write_executable(
            self.fake_bin / "uname",
            """#!/bin/sh
case "$1" in
    -s) printf '%s\\n' "$IVYGREP_FAKE_UNAME_S" ;;
    -m) printf '%s\\n' "$IVYGREP_FAKE_UNAME_M" ;;
    *) exit 2 ;;
esac
""",
        )

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def write_executable(self, path: Path, text: str) -> None:
        path.write_text(text, encoding="utf-8")
        path.chmod(path.stat().st_mode | stat.S_IXUSR)

    def make_archive(self, target: str) -> None:
        payload = self.root / "payload" / f"ivygrep-{TAG}-{target}"
        payload.mkdir(parents=True)
        self.write_executable(
            payload / "ig",
            f"#!/bin/sh\nprintf '%s\\n' 'ivygrep {TAG} {target}'\n",
        )
        archive = self.release_dir / f"ivygrep-{TAG}-{target}.tar.gz"
        with tarfile.open(archive, "w:gz") as tar:
            tar.add(payload, arcname=payload.name)
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        (self.release_dir / f"{archive.name}.sha256").write_text(
            f"{digest}  {archive.name}\n",
            encoding="utf-8",
        )

    def run_installer(
        self,
        *,
        os_name: str,
        arch: str,
        accelerator: str = "auto",
        nvidia: bool = False,
        cuda_runtime: bool = False,
    ) -> subprocess.CompletedProcess[str]:
        if nvidia:
            self.write_executable(
                self.fake_bin / "nvidia-smi",
                "#!/bin/sh\n[ \"${1:-}\" = '-L' ] && echo 'GPU 0: NVIDIA RTX' && exit 0\nexit 0\n",
            )
        cuda_lib_dir = self.root / "cuda-lib"
        if cuda_runtime:
            cuda_lib_dir.mkdir()
            for library in ("libcuda.so.1", "libcublas.so.13", "libcurand.so.10"):
                (cuda_lib_dir / library).write_text("", encoding="utf-8")
        env = os.environ.copy()
        env.update(
            {
                "PATH": f"{self.fake_bin}:{env['PATH']}",
                "IVYGREP_ACCELERATOR": accelerator,
                "IVYGREP_CUDA_LIBRARY_PATH": str(cuda_lib_dir),
                "IVYGREP_FAKE_RELEASE_DIR": str(self.release_dir),
                "IVYGREP_FAKE_UNAME_M": arch,
                "IVYGREP_FAKE_UNAME_S": os_name,
                "IVYGREP_INSTALL_DIR": str(self.install_dir),
                "IVYGREP_VERSION": TAG,
            }
        )
        return subprocess.run(
            ["sh", str(ROOT / "install.sh")],
            cwd=ROOT,
            env=env,
            text=True,
            capture_output=True,
            check=False,
        )

    def test_auto_uses_macos_metal_archive_when_present(self) -> None:
        self.make_archive("macos-aarch64")
        self.make_archive("macos-aarch64-metal")

        result = self.run_installer(os_name="Darwin", arch="arm64")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("selected Metal archive", result.stdout)
        self.assertIn("macos-aarch64-metal", result.stdout)

    def test_auto_falls_back_when_accelerator_archive_is_missing(self) -> None:
        self.make_archive("macos-aarch64")

        result = self.run_installer(os_name="Darwin", arch="arm64")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Metal archive not available", result.stdout)
        self.assertIn("macos-aarch64", result.stdout)

    def test_auto_uses_cuda_archive_when_nvidia_gpu_is_present(self) -> None:
        self.make_archive("linux-x86_64-musl")
        self.make_archive("linux-x86_64-cuda")

        result = self.run_installer(
            os_name="Linux",
            arch="x86_64",
            nvidia=True,
            cuda_runtime=True,
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("selected CUDA archive", result.stdout)
        self.assertIn("linux-x86_64-cuda", result.stdout)

    def test_auto_falls_back_when_cuda_runtime_is_missing(self) -> None:
        self.make_archive("linux-x86_64-musl")
        self.make_archive("linux-x86_64-cuda")

        result = self.run_installer(os_name="Linux", arch="x86_64", nvidia=True)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("selected CUDA archive", result.stdout)
        self.assertIn("linux-x86_64-musl", result.stdout)

    def test_explicit_cuda_fails_when_archive_is_missing(self) -> None:
        self.make_archive("linux-x86_64-musl")

        result = self.run_installer(
            os_name="Linux",
            arch="x86_64",
            accelerator="cuda",
            nvidia=True,
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("requested CUDA archive is not available", result.stderr)


if __name__ == "__main__":
    unittest.main()
