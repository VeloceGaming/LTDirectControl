"""Summarize opt-in PERF records without confusing native-inclusive scopes
with mod CPU cost. Reads logs only; does not change affinity, power or game
settings. Example: python tools/summarize_performance.py probe.log --output report.json
"""
import argparse
import json
import re
from pathlib import Path


def summarize(text):
    version = "unknown (log may start mid-session)"
    windows, work = [], []
    for line in text.splitlines():
        init = re.search(r"\bINIT .*?probe=([^\s]+)", line)
        if init:
            version = init[1]
        if "PERF window_s=" in line:
            values = dict(re.findall(r"(window_s|frames|updates_per_s|mod_total_ms|slow_25ms)=([\d.]+)", line))
            values = {k: (float(v) if '.' in v else int(v)) for k, v in values.items()}
            frame = re.search(r"frame_ms p50=(\S+) p95=(\S+) p99=(\S+) max=([\d.]+)", line)
            if frame and "frames" in values:
                windows.append({"version": version, **values, "frame_ms": {
                    "p50": frame[1], "p95": frame[2], "p99": frame[3], "max": float(frame[4])}})
        match = re.search(r"PERF WORK (\S+) count=(\d+) avg_us=(\d+) max_us=(\d+) p95_us=(<= \d+|>\d+|-) p99_us=(<= \d+|>\d+|-) total_us=(\d+)", line)
        if match:
            work.append({"version": version, "scope": match[1], "count": int(match[2]),
                "avg_us": int(match[3]), "max_us": int(match[4]), "p95_us": match[5],
                "p99_us": match[6], "total_us": int(match[7])})
    versions = sorted({w["version"] for w in windows + work})
    return {"versions": versions, "frame_windows": windows, "work_windows": work,
        "coverage": "Detailed timings present" if work else "No detailed timings; enable Advanced > Debug > Capture performance measurements and play for 10+ seconds.",
        "interpretation": [
            "Frame intervals describe client updates, not GPU presentation or measured visible input onset.",
            "mod_total_ms covers client callbacks only. SDK timings cover all worker threads, including background callbacks.",
            "Scopes containing incl_native include original game work; never treat them as pure mod cost.",
            "Scopes overlap; their totals must not be summed. Work percentile values are histogram upper bounds.",
            "capture_to_playback ends at published-frame playback, not movement or projectile onset; bounded queue may omit samples during stalls.",
            "Compare equivalent match phases, item/champion mod sets and capture settings. Profiling itself has overhead.",
            "Window percentiles remain separate; averaging them does not give a whole-session percentile.",
        ]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("logs", nargs="+", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = {"logs": [{"name": p.name, **summarize(p.read_text(encoding="utf-8", errors="replace"))} for p in args.logs]}
    output = json.dumps(result, indent=2) + "\n"
    if args.output:
        args.output.write_text(output, encoding="utf-8")
    else:
        print(output, end="")


if __name__ == "__main__":
    main()
