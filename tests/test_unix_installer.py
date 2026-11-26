import hashlib
import os
import stat
import subprocess
import sys
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
        self.write_executable(self.fake_bin / "nvidia-smi", "#!/bin/sh\nexit 1\n")
        self.write_executable(
            self.fake_bin / "curl",
            """#!/bin/sh
set -eu
fail=0
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
        -*)
            case "$1" in *f*) fail=1 ;; esac
            case "$1" in *I*) head=1 ;; esac
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
if [ "$head" -eq 1 ]; then
    status=404
    [ ! -f "$src" ] || status=302
    status=${IVYGREP_FAKE_HEAD_STATUS:-$status}
    printf '%s' "$status"
    [ "$status" != 000 ] || exit 7
    [ "$fail" -eq 0 ] || [ "$status" -lt 400 ] || exit 22
    exit 0
fi
if [ ! -f "$src" ]; then
    exit 22
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
        nvidia_compute: str = "8.6",
        local_archive: Path | None = None,
        extra_env: dict[str, str] | None = None,
    ) -> subprocess.CompletedProcess[str]:
        if nvidia:
            self.write_executable(
                self.fake_bin / "nvidia-smi",
                "#!/bin/sh\n"
                "[ \"${1:-}\" = '-L' ] && echo 'GPU 0: NVIDIA RTX' && exit 0\n"
                f"echo '{nvidia_compute}'\n",
            )
        cuda_lib_dir = self.root / "cuda-lib"
        if cuda_runtime:
            cuda_lib_dir.mkdir()
            for library in (
                "libcuda.so.1",
                "libcublas.so.13",
                "libcublasLt.so.13",
                "libcurand.so.10",
            ):
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
        if local_archive is not None:
            env["IVYGREP_INSTALL_ARCHIVE"] = str(local_archive)
            env["IVYGREP_INSTALL_CHECKSUM"] = f"{local_archive}.sha256"
        env.update(extra_env or {})
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
        self.assertIn("NVIDIA GPU detected, but CUDA 13 runtime is incomplete", result.stdout)
        self.assertIn("libcublas.so.13", result.stdout)
        self.assertIn("libcublasLt.so.13", result.stdout)
        self.assertIn("selected portable archive", result.stdout)
        self.assertIn("linux-x86_64-musl", result.stdout)

    def test_auto_rejects_gpu_unsupported_by_shipped_cuda_build(self) -> None:
        self.make_archive("linux-x86_64-musl")
        self.make_archive("linux-x86_64-cuda")

        result = self.run_installer(
            os_name="Linux",
            arch="x86_64",
            nvidia=True,
            cuda_runtime=True,
            nvidia_compute="6.1",
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("compute capability 6.1 is unsupported", result.stdout)
        self.assertIn("selected portable archive", result.stdout)

    def test_auto_explains_portable_selection_without_nvidia(self) -> None:
        self.make_archive("linux-x86_64-musl")

        result = self.run_installer(os_name="Linux", arch="x86_64")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("no ready NVIDIA CUDA GPU detected", result.stdout)
        self.assertIn("selected portable archive", result.stdout)

    def test_explicit_cuda_fails_when_archive_is_missing(self) -> None:
        self.make_archive("linux-x86_64-musl")

        result = self.run_installer(
            os_name="Linux",
            arch="x86_64",
            accelerator="cuda",
            nvidia=True,
            cuda_runtime=True,
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("requested CUDA archive is not available", result.stderr)

    def test_explicit_cuda_fails_with_exact_missing_runtime(self) -> None:
        self.make_archive("linux-x86_64-cuda")

        result = self.run_installer(
            os_name="Linux",
            arch="x86_64",
            accelerator="cuda",
            nvidia=True,
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("CUDA 13 runtime is incomplete", result.stderr)
        self.assertIn("libcublas.so.13", result.stderr)
        self.assertIn("libcublasLt.so.13", result.stderr)

    def test_explicit_cuda_rejects_unsupported_gpu(self) -> None:
        self.make_archive("linux-x86_64-cuda")

        result = self.run_installer(
            os_name="Linux",
            arch="x86_64",
            accelerator="cuda",
            nvidia=True,
            cuda_runtime=True,
            nvidia_compute="7.5",
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("compute capability 8.0 or newer", result.stderr)
        self.assertIn("detected 7.5", result.stderr)

    def test_local_release_artifact_bypasses_hardware_detection(self) -> None:
        self.make_archive("linux-x86_64-musl")
        archive = self.release_dir / f"ivygrep-{TAG}-linux-x86_64-musl.tar.gz"

        result = self.run_installer(
            os_name="Linux",
            arch="x86_64",
            accelerator="cuda",
            local_archive=archive,
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("ivygrep v9.9.9 linux-x86_64-musl", result.stdout)

    def test_invalid_checksum_sidecar_keeps_the_installed_binary(self) -> None:
        self.make_archive("macos-x86_64")
        # Like the macOS sha256sum, "-c" accepts a file without a valid line.
        self.write_executable(
            self.fake_bin / "sha256sum",
            f"""#!/bin/sh
[ "${{1:-}}" != -c ] || exit 0
exec "{sys.executable}" -c '
import hashlib, sys
print(hashlib.sha256(sys.stdin.buffer.read()).hexdigest() + "  -")
'
""",
        )
        name = f"ivygrep-{TAG}-macos-x86_64.tar.gz"
        sidecar = self.release_dir / f"{name}.sha256"
        valid = sidecar.read_text(encoding="utf-8")
        installed = self.install_dir / "ig"
        installed.write_text("previous", encoding="utf-8")

        for invalid in (
            "",
            "<!doctype html>\n",
            f"{valid.split()[0][:-1]}  {name}\n",
            f"{'0' * 64}  {name}\n",
        ):
            with self.subTest(sidecar=invalid):
                sidecar.write_text(invalid, encoding="utf-8")
                result = self.run_installer(os_name="Darwin", arch="x86_64")
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("SHA-256 verification failed", result.stderr)
                self.assertEqual(installed.read_text(encoding="utf-8"), "previous")

        sidecar.write_text(valid.upper(), encoding="utf-8")
        result = self.run_installer(os_name="Darwin", arch="x86_64")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotEqual(installed.read_text(encoding="utf-8"), "previous")

    def test_failed_accelerator_check_is_not_reported_as_missing(self) -> None:
        self.make_archive("macos-aarch64")
        self.make_archive("macos-aarch64-metal")

        for status, reason in (("503", "not reachable (HTTP 503)"), ("000", "not reachable")):
            with self.subTest(status=status):
                env = {"IVYGREP_FAKE_HEAD_STATUS": status}
                auto = self.run_installer(os_name="Darwin", arch="arm64", extra_env=env)
                self.assertEqual(auto.returncode, 0, auto.stderr)
                self.assertIn(f"Metal archive {reason} for {TAG}", auto.stdout)

                explicit = self.run_installer(
                    os_name="Darwin", arch="arm64", accelerator="metal", extra_env=env
                )
                self.assertNotEqual(explicit.returncode, 0)
                self.assertIn(
                    f"requested Metal archive is {reason} for {TAG}", explicit.stderr
                )

    def test_rosetta_shell_selects_apple_silicon_archive(self) -> None:
        self.make_archive("macos-x86_64")
        self.make_archive("macos-aarch64-metal")

        for translated, target in (("1", "macos-aarch64-metal"), ("0", "macos-x86_64")):
            with self.subTest(translated=translated):
                self.write_executable(
                    self.fake_bin / "sysctl", f"#!/bin/sh\necho {translated}\n"
                )
                result = self.run_installer(os_name="Darwin", arch="x86_64")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn(f"archive ({target})", result.stdout)

    def test_path_advice_names_the_ig_that_runs(self) -> None:
        self.make_archive("linux-x86_64-musl")
        other = self.root / "other"
        other.mkdir()
        self.write_executable(other / "ig", "#!/bin/sh\necho other\n")
        linked = self.root / "linked"
        linked.mkdir()
        (linked / "ig").symlink_to(self.install_dir / "ig")
        base = f"{self.fake_bin}:{os.environ['PATH']}"

        for path, directory, shadowed, add_to_path in (
            (f"{other}:{self.install_dir}:{base}", self.install_dir, True, False),
            (f"{self.install_dir}:{other}:{base}", self.install_dir, False, False),
            (f"{linked}:{self.install_dir}:{base}", self.install_dir, False, False),
            (f"{self.install_dir}:{base}", f"{self.install_dir}/", False, False),
            (f"{self.install_dir}/:{base}", self.install_dir, False, False),
            (f"{other}:{base}", self.install_dir, True, True),
        ):
            with self.subTest(path=path, directory=str(directory)):
                result = self.run_installer(
                    os_name="Linux",
                    arch="x86_64",
                    accelerator="portable",
                    extra_env={"PATH": path, "IVYGREP_INSTALL_DIR": str(directory)},
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                note = f"Another ig comes first on PATH: {other / 'ig'}"
                self.assertEqual(note in result.stdout, shadowed, result.stdout)
                self.assertEqual("Another ig" in result.stdout, shadowed, result.stdout)
                self.assertEqual("to PATH to run" in result.stdout, add_to_path)


if __name__ == "__main__":
    unittest.main()
