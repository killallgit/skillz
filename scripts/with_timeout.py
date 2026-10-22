"""Run a command and kill its whole process group if it outlives the limit.

usage: with_timeout.py SECONDS COMMAND [ARGS...]
Exits 124 on timeout (like coreutils timeout), else the command's own exit code.
"""

import os
import signal
import subprocess
import sys

TIMED_OUT = 124


def main() -> int:
    seconds, command = int(sys.argv[1]), sys.argv[2:]
    child = subprocess.Popen(command, start_new_session=True)
    try:
        return child.wait(timeout=seconds)
    except subprocess.TimeoutExpired:
        os.killpg(child.pid, signal.SIGKILL)
        child.wait()
        print(f"timed out after {seconds}s: {' '.join(command)}", file=sys.stderr)
        return TIMED_OUT


if __name__ == "__main__":
    sys.exit(main())
