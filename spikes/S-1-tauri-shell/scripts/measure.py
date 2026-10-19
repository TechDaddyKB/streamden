#!/usr/bin/env python3
"""Idle CPU and memory of a process tree, from /proc only.

Usage: python3 -I measure.py ROOT_PID [SECONDS=60] [INTERVAL=5]

CPU% is the sum over the whole tree (root + all descendants, e.g.
WebKitWebProcess / WebKitNetworkProcess), as % of ONE core, computed from
utime+stime deltas. Memory is sampled every INTERVAL s: RSS (double-counts
shared pages) and PSS (proportional, closer to real cost), both summed.
Prints one JSON object.
"""
import json
import os
import sys
import time

CLK = os.sysconf("SC_CLK_TCK")


def children_map():
    kids = {}
    for d in os.listdir("/proc"):
        if not d.isdigit():
            continue
        try:
            with open(f"/proc/{d}/stat") as f:
                s = f.read()
        except OSError:
            continue
        rest = s[s.rfind(")") + 2 :].split()
        kids.setdefault(int(rest[1]), []).append(int(d))
    return kids


def tree(root):
    kids = children_map()
    out, todo = [], [root]
    while todo:
        p = todo.pop()
        out.append(p)
        todo.extend(kids.get(p, []))
    return out


def cpu_ticks(pid):
    try:
        with open(f"/proc/{pid}/stat") as f:
            s = f.read()
    except OSError:
        return None
    comm = s[s.find("(") + 1 : s.rfind(")")]
    rest = s[s.rfind(")") + 2 :].split()
    return comm, int(rest[11]) + int(rest[12])


def mem_kb(pid):
    rss = pss = 0
    try:
        with open(f"/proc/{pid}/smaps_rollup") as f:
            for line in f:
                if line.startswith("Rss:"):
                    rss = int(line.split()[1])
                elif line.startswith("Pss:"):
                    pss = int(line.split()[1])
    except OSError:
        pass
    return rss, pss


def main():
    root = int(sys.argv[1])
    secs = float(sys.argv[2]) if len(sys.argv) > 2 else 60.0
    step = float(sys.argv[3]) if len(sys.argv) > 3 else 5.0

    t0 = time.monotonic()
    start = {p: cpu_ticks(p) for p in tree(root)}
    samples = []
    pss_by_comm = {}
    while time.monotonic() - t0 < secs:
        pids = tree(root)
        rss = pss = 0
        last = {}
        for p in pids:
            r, s = mem_kb(p)
            rss += r
            pss += s
            c = cpu_ticks(p)
            if c:
                last[c[0]] = last.get(c[0], 0) + s
        pss_by_comm = last
        samples.append((rss, pss, len(pids)))
        time.sleep(min(step, max(0.0, secs - (time.monotonic() - t0))))
    elapsed = time.monotonic() - t0
    end_pids = tree(root)
    per_comm = {}
    total = 0
    for p in end_pids:
        e = cpu_ticks(p)
        if e is None:
            continue
        s = start.get(p)
        d = e[1] - (s[1] if s else 0)
        total += d
        per_comm[e[0]] = per_comm.get(e[0], 0) + d
    rss_mb = [s[0] / 1024 for s in samples]
    pss_mb = [s[1] / 1024 for s in samples]
    comms = {}
    for p in end_pids:
        c = cpu_ticks(p)
        if c:
            comms[c[0]] = comms.get(c[0], 0) + 1
    print(
        json.dumps(
            {
                "root": root,
                "seconds": round(elapsed, 1),
                "cpu_pct_one_core": round(100.0 * total / CLK / elapsed, 2),
                "cpu_pct_by_process": {
                    k: round(100.0 * v / CLK / elapsed, 2) for k, v in per_comm.items()
                },
                "rss_mb_avg": round(sum(rss_mb) / len(rss_mb), 1),
                "rss_mb_max": round(max(rss_mb), 1),
                "pss_mb_avg": round(sum(pss_mb) / len(pss_mb), 1),
                "pss_mb_max": round(max(pss_mb), 1),
                "pss_mb_by_process_last": {k: round(v / 1024, 1) for k, v in pss_by_comm.items()},
                "processes": comms,
            }
        )
    )


if __name__ == "__main__":
    main()
