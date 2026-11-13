"""Run a bounded harness and its children; never retain drawing frames here."""
import os
import subprocess
import sys
import time
import psutil

def run(argv, seconds=60, memory=1536 * 1024 * 1024):
    env = dict(os.environ, PYTHONDONTWRITEBYTECODE='1')
    child = subprocess.Popen(argv, env=env)
    root = psutil.Process(child.pid)
    deadline = time.monotonic() + seconds
    while child.poll() is None:
        try:
            family = [root] + root.children(recursive=True)
            rss = sum(p.memory_info().rss for p in family if p.is_running())
        except psutil.Error:
            rss = 0
        if time.monotonic() > deadline or rss > memory:
            print(f'WATCHDOG FAIL: time>{seconds}s or RSS>{memory} bytes', flush=True)
            for p in reversed(family):
                try: p.kill()
                except psutil.Error: pass
            child.wait()
            return 3
        time.sleep(.1)
    return child.returncode

if __name__ == '__main__':
    seconds = int(sys.argv[1])
    raise SystemExit(run(sys.argv[2:], seconds))
