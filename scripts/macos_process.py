"""Process counters on macOS through libproc, for harnesses that read `/proc` on Linux.

The quantities are the closest macOS equivalents, not the same measurements.
`METRICS` states each derivation, and the harnesses copy it into their reports.
A missing or inaccessible process raises `ProcessLookupError`. It never reads as zero.
"""

from __future__ import annotations

import ctypes
import ctypes.util
import struct
from typing import Any

METRICS = {
    "rss_bytes": "ri_resident_size from proc_pid_rusage",
    "rss_anon_bytes": "ri_phys_footprint from proc_pid_rusage: dirty and compressed memory. "
                      "Linux reports resident anonymous pages only.",
    "threads": "pti_threadnum from proc_pidinfo(PROC_PIDTASKINFO)",
    "fds": "entries returned by proc_pidinfo(PROC_PIDLISTFDS)",
    "cpu": "ri_user_time + ri_system_time from proc_pid_rusage",
    "disk_io": "ri_diskio_bytesread and ri_diskio_byteswritten from proc_pid_rusage",
    "wakeups": "ri_pkg_idle_wkups + ri_interrupt_wkups for the whole process. "
               "Linux counts voluntary context switches of each thread.",
    "not_measured": "PSS, file-backed RSS, and watcher counts. File watchers use FSEvents, which holds no descriptors.",
}

RUSAGE_INFO_V4 = 4
PROC_PIDLISTFDS, PROC_PIDTBSDINFO, PROC_PIDTASKINFO = 1, 3, 4
CTL_KERN, KERN_PROCARGS2 = 1, 49
FD_INFO_SIZE = 8  # sizeof(struct proc_fdinfo)


class RUsageInfoV4(ctypes.Structure):
    _fields_ = [("ri_uuid", ctypes.c_uint8 * 16)] + [(name, ctypes.c_uint64) for name in (
        "ri_user_time", "ri_system_time", "ri_pkg_idle_wkups", "ri_interrupt_wkups", "ri_pageins",
        "ri_wired_size", "ri_resident_size", "ri_phys_footprint", "ri_proc_start_abstime",
        "ri_proc_exit_abstime", "ri_child_user_time", "ri_child_system_time", "ri_child_pkg_idle_wkups",
        "ri_child_interrupt_wkups", "ri_child_pageins", "ri_child_elapsed_abstime", "ri_diskio_bytesread",
        "ri_diskio_byteswritten", "ri_cpu_time_qos_default", "ri_cpu_time_qos_maintenance",
        "ri_cpu_time_qos_background", "ri_cpu_time_qos_utility", "ri_cpu_time_qos_legacy",
        "ri_cpu_time_qos_user_initiated", "ri_cpu_time_qos_user_interactive", "ri_billed_system_time",
        "ri_serviced_system_time", "ri_logical_writes", "ri_lifetime_max_phys_footprint", "ri_instructions",
        "ri_cycles", "ri_billed_energy", "ri_serviced_energy", "ri_interval_max_phys_footprint",
        "ri_runnable_time")]


class ProcTaskInfo(ctypes.Structure):
    _fields_ = [(name, ctypes.c_uint64) for name in (
        "pti_virtual_size", "pti_resident_size", "pti_total_user", "pti_total_system", "pti_threads_user",
        "pti_threads_system")] + [(name, ctypes.c_int32) for name in (
        "pti_policy", "pti_faults", "pti_pageins", "pti_cow_faults", "pti_messages_sent",
        "pti_messages_received", "pti_syscalls_mach", "pti_syscalls_unix", "pti_csw", "pti_threadnum",
        "pti_numrunning", "pti_priority")]


class ProcBsdInfo(ctypes.Structure):
    _fields_ = [(name, ctypes.c_uint32) for name in (
        "pbi_flags", "pbi_status", "pbi_xstatus", "pbi_pid", "pbi_ppid", "pbi_uid", "pbi_gid", "pbi_ruid",
        "pbi_rgid", "pbi_svuid", "pbi_svgid", "rfu_1")] + [
        ("pbi_comm", ctypes.c_char * 16), ("pbi_name", ctypes.c_char * 32)] + [(name, ctypes.c_uint32) for name in (
        "pbi_nfiles", "pbi_pgid", "pbi_pjobc", "e_tdev", "e_tpgid")] + [
        ("pbi_nice", ctypes.c_int32), ("pbi_start_tvsec", ctypes.c_uint64), ("pbi_start_tvusec", ctypes.c_uint64)]


class _Timebase(ctypes.Structure):
    _fields_ = [("numer", ctypes.c_uint32), ("denom", ctypes.c_uint32)]


_libproc = ctypes.CDLL(ctypes.util.find_library("proc") or "/usr/lib/libproc.dylib", use_errno=True)
_libc = ctypes.CDLL(ctypes.util.find_library("c") or "/usr/lib/libSystem.dylib", use_errno=True)
_libproc.proc_pid_rusage.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_void_p]
_libproc.proc_pidinfo.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_uint64, ctypes.c_void_p, ctypes.c_int]
_libproc.proc_listallpids.argtypes = [ctypes.c_void_p, ctypes.c_int]
_timebase = _Timebase()
_libc.mach_timebase_info(ctypes.byref(_timebase))


def _missing(pid: int) -> ProcessLookupError:
    return ProcessLookupError(f"process {pid} is gone or not accessible")


def usage(pid: int) -> RUsageInfoV4:
    """Resource counters of a process. An exited child keeps them until its parent reaps it."""
    info = RUsageInfoV4()
    if _libproc.proc_pid_rusage(pid, RUSAGE_INFO_V4, ctypes.byref(info)) != 0:
        raise _missing(pid)
    return info


def cpu_seconds(info: RUsageInfoV4) -> float:
    # The kernel reports CPU time in Mach time units.
    return (info.ri_user_time + info.ri_system_time) * _timebase.numer / _timebase.denom / 1e9


def _pidinfo(pid: int, flavor: int, info: Any) -> Any:
    if _libproc.proc_pidinfo(pid, flavor, 0, ctypes.byref(info), ctypes.sizeof(info)) != ctypes.sizeof(info):
        raise _missing(pid)
    return info


def threads(pid: int) -> int:
    return _pidinfo(pid, PROC_PIDTASKINFO, ProcTaskInfo()).pti_threadnum


def descriptors(pid: int) -> int:
    # The first call sizes the buffer for the whole descriptor table. The second returns the open entries.
    size = _libproc.proc_pidinfo(pid, PROC_PIDLISTFDS, 0, None, 0)
    if size <= 0:
        raise _missing(pid)
    size += 64 * FD_INFO_SIZE  # descriptors opened between the two calls
    buffer = ctypes.create_string_buffer(size)
    used = _libproc.proc_pidinfo(pid, PROC_PIDLISTFDS, 0, buffer, size)
    if used <= 0:
        raise _missing(pid)
    return used // FD_INFO_SIZE


def memory_sample(pid: int) -> dict[str, int]:
    """The memory, thread, and descriptor fields that the soak gates compare."""
    info = usage(pid)
    return {"rss_bytes": info.ri_resident_size, "rss_anon_bytes": info.ri_phys_footprint,
            "threads": threads(pid), "fds": descriptors(pid)}


def parse_procargs(raw: bytes) -> tuple[list[str], list[bytes]]:
    """Split a `KERN_PROCARGS2` buffer into argv and environment entries.

    Layout: argc, the executable path, NUL padding, argc arguments, then the environment.
    """
    argc = struct.unpack_from("i", raw)[0]
    rest = raw[4:]
    parts = rest[rest.find(b"\0"):].lstrip(b"\0").split(b"\0")
    return [part.decode(errors="replace") for part in parts[:argc]], [part for part in parts[argc:] if part]


def _procargs(pid: int) -> bytes:
    mib = (ctypes.c_int * 3)(CTL_KERN, KERN_PROCARGS2, pid)
    size = ctypes.c_size_t(0)
    if _libc.sysctl(mib, 3, None, ctypes.byref(size), None, 0) != 0 or size.value < 4:
        raise _missing(pid)
    buffer = ctypes.create_string_buffer(size.value)
    if _libc.sysctl(mib, 3, buffer, ctypes.byref(size), None, 0) != 0:
        raise _missing(pid)
    return buffer.raw[:size.value]


def processes_with_environment(entry: bytes) -> list[dict[str, Any]]:
    """Live processes of this user whose environment contains `entry`, with their parent and argv.

    The kernel hides the arguments of other users' processes and of exited
    processes, so neither can match.
    """
    count = _libproc.proc_listallpids(None, 0)
    pids = (ctypes.c_int * (count + 256))()
    count = _libproc.proc_listallpids(pids, ctypes.sizeof(pids))
    found = []
    for pid in pids[:max(count, 0)]:
        try:
            argv, environment = parse_procargs(_procargs(pid))
            if entry in environment:
                found.append({"pid": pid, "ppid": _pidinfo(pid, PROC_PIDTBSDINFO, ProcBsdInfo()).pbi_ppid,
                              "argv": argv})
        except ProcessLookupError:
            continue  # not ours, or it exited between listing and reading
    return found
