"""Bound a command and its descendants by wall time and memory (requires psutil).

The command must also supply a finite script/tick/step limit. Output is inherited;
the final WATCHDOG line reports peak sampled tree RSS and peak process working set.
"""
import argparse
import subprocess
import sys
import time

import psutil


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seconds", type=float, default=60)
    parser.add_argument("--mib", type=float, default=1536)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command or args.seconds <= 0 or not 0 < args.mib <= 8192:
        parser.error("need a command, positive wall limit and memory limit <= 8192 MiB")
    start = time.monotonic()
    child = subprocess.Popen(command)
    root = psutil.Process(child.pid)
    seen = {root.pid: root}
    peak = process_peak = private_peak = 0
    reason = "complete"
    try:
        while True:
            try:
                seen.update((p.pid, p) for p in root.children(recursive=True))
            except psutil.NoSuchProcess:
                pass
            rss = 0
            for proc in tuple(seen.values()):
                try:
                    if not proc.is_running():
                        seen.pop(proc.pid, None)
                        continue
                    mem = proc.memory_info()
                    rss += mem.rss
                    process_peak = max(process_peak, getattr(mem, "peak_wset", mem.rss))
                    private_peak = max(private_peak, getattr(mem, "private", mem.rss))
                except psutil.NoSuchProcess:
                    pass
            peak = max(peak, rss)
            if max(rss, process_peak, private_peak) > args.mib * 1024**2:
                reason = "memory limit"
                break
            if time.monotonic() - start > args.seconds:
                reason = "wall limit"
                break
            if child.poll() is not None:
                break
            time.sleep(0.01)
    finally:
        for proc in reversed(tuple(seen.values())):
            try:
                if proc.is_running():
                    proc.kill()
            except psutil.NoSuchProcess:
                pass
        child.wait()
        print(f"WATCHDOG {reason}: exit={child.returncode}, seconds={time.monotonic()-start:.2f}, "
              f"peak_tree_rss_mib={peak/1024**2:.2f}, peak_process_mib={process_peak/1024**2:.2f}, "
              f"memguard_private_mib={private_peak/1024**2:.2f}", flush=True)
    return child.returncode if reason == "complete" else 124


if __name__ == "__main__":
    sys.exit(main())
