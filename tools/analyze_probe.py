"""Summarize diagnostic timing without inferring live-control capability."""
import argparse
import collections
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LINE = re.compile(r'^(\d+) SIM tick=(\d+) seed=(\d+) kind=(\d+) match=(\d+) replay=(\d+) set=(\d+)')


def analyze(path):
    groups = collections.defaultdict(list)
    counts = collections.Counter()
    scenes = []
    pacing = []
    native = []
    display = []
    latest_tick = None
    windows = []
    active_window = None
    budget_from_readiness = False
    for line in path.read_text(encoding='utf-8').splitlines():
        match = LINE.match(line)
        if match:
            stamp, tick, seed, kind, match_id, replay, set_id = map(int, match.groups())
            counts[kind] += 1
            if kind == 2:
                groups[(seed, match_id, set_id)].append((stamp, tick))
                latest_tick = tick
        if ' CLIENT ' in line:
            scenes.append(line)
        if ' PACE ' in line:
            pacing.append(line)
        if ' NATIVE ' in line:
            native.append(line)
        if ' TIMING ' in line:
            budget_from_readiness = 'budget_from_readiness=true' in line
        start = re.match(r'^(\d+) PACE starting key=\((\d+), (\d+), (\d+)\).* tick=(\d+)', line)
        stop = re.match(r'^\d+ PACE stopped reason=(\w+) tick=(\d+) elapsed_us=(\d+)', line)
        ready = re.match(r'^(\d+) PACE battlefield_ready tick=(\d+) elapsed_us=(\d+)', line)
        if start:
            stamp, seed, match_id, set_id, tick = map(int, start.groups())
            active_window = {'start_timestamp_ms': stamp, 'seed': seed, 'match_id': match_id, 'set_index': set_id, 'start_tick': tick}
            windows.append(active_window)
        if ready and active_window is not None:
            stamp, tick, elapsed = map(int, ready.groups())
            active_window.update({'ready_timestamp_ms': stamp, 'ready_tick': tick, 'startup_seconds': elapsed / 1_000_000})
        if stop and active_window is not None:
            reason, tick, elapsed = stop.groups()
            running = re.search(r' running_us=(\d+)', line)
            ratio_elapsed = int(elapsed) / 1_000_000
            ratio_tick = active_window['start_tick']
            if budget_from_readiness:
                ratio_elapsed = int(running.group(1)) / 1_000_000 if running else 0
                ratio_tick = active_window.get('ready_tick', int(tick))
                active_window['running_seconds'] = ratio_elapsed
            simulation_seconds = (int(tick) - ratio_tick) / 60
            active_window.update({'stop_reason': reason, 'stop_tick': int(tick), 'elapsed_seconds': int(elapsed)/1_000_000, 'simulation_seconds': simulation_seconds,
                                  'ratio_origin': 'battlefield_readiness' if budget_from_readiness else 'worker_start',
                                  'simulation_to_wallclock_ratio': simulation_seconds/ratio_elapsed if ratio_elapsed else None})
            active_window = None
        clock = re.match(r'^(\d+) DISPLAY timer=Some\("(\d+):(\d+)"\)', line)
        if clock:
            stamp, minute, second = map(int, clock.groups())
            display.append({'timestamp_ms': stamp, 'display_seconds': minute*60+second, 'latest_sampled_worker_tick': latest_tick})
    matches = []
    for (seed, match_id, set_id), samples in groups.items():
        first, last = samples[0], samples[-1]
        elapsed_seconds = (last[0] - first[0]) / 1000
        simulation_seconds = (last[1] - first[1]) / 60
        matches.append({
            'seed': seed,
            'match_id': match_id,
            'set_index': set_id,
            'sample_count': len(samples),
            'first_tick': first[1],
            'last_tick': last[1],
            'elapsed_seconds': elapsed_seconds,
            'simulation_seconds': round(simulation_seconds, 3),
            'simulation_to_wallclock_ratio': round(simulation_seconds / elapsed_seconds, 3) if elapsed_seconds else None,
        })
    return {
        'recording': str(path.resolve()),
        'sample_counts_by_origin': dict(sorted(counts.items())),
        'client_scenes': scenes,
        'pacing_events': pacing,
        'native_coordinator_events': native,
        'pacing_windows': windows,
        'display_samples': display,
        'foreground_matches': matches,
        'caveat': 'Kind 2 identifies the client match simulation, not its displayed playback tick or saved result authority. Whole-second display labels do not measure precise input latency.',
    }


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('log', nargs='?', type=Path, default=ROOT / 'probe.log')
    parser.add_argument('--output', type=Path, default=ROOT / 'research' / 'probe-summary.json')
    args = parser.parse_args()
    result = analyze(args.log)
    args.output.write_text(json.dumps(result, indent=2), encoding='utf-8')
    print(json.dumps(result, indent=2))
