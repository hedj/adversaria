"""Write an analysis's results for the memex to import (docs/design.md
§ Importing results).

One call at the end of an analysis:

    from memex_results import write_results
    write_results("results.json",
                  results=[("mean_delay", 1.52, 0.07, "s")],
                  inputs=["data.csv"])

The file holds the values with their standard uncertainties and units, and a
provenance record after the W3C PROV model: the script, its version-control
commit, the command, the Python version, and the SHA-256 of each input file,
so the memex can mark a result whose inputs changed since.
"""

import datetime
import hashlib
import json
import os
import platform
import subprocess
import sys

FORMAT = "memex-results"
VERSION = 1


def _sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 16), b""):
            h.update(chunk)
    return h.hexdigest()


def _commit(directory):
    try:
        out = subprocess.run(["git", "rev-parse", "HEAD"], cwd=directory or ".",
                             capture_output=True, text=True, check=True)
        return out.stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return None


def write_results(path, results, inputs, script=None):
    """Write results [(name, value, standard uncertainty, unit), ...] and the
    provenance of inputs [file path, ...], relative to the results file."""
    script = script or os.path.basename(sys.argv[0] or "analysis")
    base = os.path.dirname(os.path.abspath(path))
    used = []
    for p in inputs:
        full = p if os.path.isabs(p) else os.path.join(base, p)
        used.append({"path": os.path.relpath(full, base), "sha256": _sha256(full)})
    record = {
        "format": FORMAT,
        "version": VERSION,
        "prov": {
            "activity": {
                "script": script,
                "commit": _commit(base),
                "command": sys.argv,
                "endedAtTime": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
            },
            "agent": {"python": platform.python_version()},
            "used": used,
        },
        "results": [
            {"name": n, "value": float(v), "uncertainty": float(u), "unit": unit}
            for (n, v, u, unit) in results
        ],
    }
    with open(path, "w", encoding="utf-8") as f:
        json.dump(record, f, indent=1)
