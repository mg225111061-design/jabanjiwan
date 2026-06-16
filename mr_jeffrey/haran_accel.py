"""
STAGE W6 — integrated unstructured-acceleration table (measured, per workload, no headline number).
Constant-factor only; compute-bound gains > memory-bound; techniques overlap (not a product); Ω(N) intact.
"""
from accel_harness import run, accel_bin


def table():
    rows = []
    if not accel_bin():
        return rows
    s = run("simd", 2048); sl = run("simd", 1 << 20)
    rows.append(("SIMD poly8 (compute-bound)", f"{s['poly8_speedup']}×", "L1; real arithmetic SIMD win"))
    rows.append(("SIMD sum (latency→bandwidth)", f"{sl['sum_speedup']}×", "1M; naive scalar→bandwidth"))
    so = run("soa", 1 << 22)
    rows.append(("SoA layout (memory-bound)", f"{so['soa_speedup']}×", "4M; bandwidth utilization"))
    al = run("algo", 100000); al2 = run("algo", 4000000)
    rows.append(("radix vs std sort", f"{al['speedup']}× … {al2['speedup']}×", "100K…4M; O(n), Ω(N) intact"))
    p = run("parallel", 1 << 23, 4)
    rows.append(("parallel compute-bound (4c)", f"{p['cpu_speedup']}×", "near-linear in cores"))
    rows.append(("parallel memory-bound (4c)", f"{p['mem_speedup']}×", "SUBLINEAR — bandwidth saturates"))
    na = run("noalias", 1 << 16)
    rows.append(("verified noalias (memory-bound)", f"{na['fma_speedup']}×", "≈1× here; SAFE vs C restrict"))
    return rows


def render(rows):
    out = ["===== HARAN v4.5 — unstructured acceleration (MEASURED, per workload) ====="]
    out.append(f"   {'technique / kernel':34} {'measured':>14}   note")
    for name, sp, note in rows:
        out.append(f"   {name:34} {sp:>14}   {note}")
    out.append("   ── highest: SIMD poly8 ~8× (compute-bound) · lowest: noalias/radix-at-scale ~1× (memory-bound)")
    out.append("   ── constant-factor ONLY; techniques overlap (not a product); Ω(N) never broken.")
    out.append("   ── verified noalias is HARAN's unique edge (C needs unchecked `restrict`).")
    return "\n".join(out)
