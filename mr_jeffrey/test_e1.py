"""v17 Part E · E1 tests — differential specs (catch property-invisible bugs). Run: python3 test_e1.py

E1.1 git differential: last passing commit vs now → diverging inputs (real temp git repo).
E1.2 N-version differential: majority vote → the disagreeing implementation is the suspect.
E1.3 property-INVISIBLE bug (scale_bug: x-1 vs x*2) — B finds nothing; differential catches it.
"""
import os
import subprocess
import sys
import tempfile

import differential as D

PASS, FAIL, SKIP = [], [], []


def check(n, c, d=""):
    (PASS if c else FAIL).append(n)
    print(f"  [{'PASS' if c else 'FAIL'}] {n}" + (f" — {d}" if d and not c else ""))


OLD = "def scale(x):\n    return x*2\n"
NEW = "def scale(x):\n    return x-1\n"     # property-invisible regression


def _git(d, *args):
    subprocess.run(["git", "-C", d, "-c", "commit.gpgsign=false", *args],
                   check=True, capture_output=True)


def _make_repo():
    d = tempfile.mkdtemp()
    _git(d, "init", "-q")
    _git(d, "config", "user.email", "t@t"); _git(d, "config", "user.name", "t")
    _git(d, "config", "commit.gpgsign", "false")
    open(os.path.join(d, "s.py"), "w").write(OLD)
    _git(d, "add", "s.py"); _git(d, "commit", "--no-gpg-sign", "-qm", "v1-correct")
    open(os.path.join(d, "s.py"), "w").write(NEW)
    _git(d, "add", "s.py"); _git(d, "commit", "--no-gpg-sign", "-qm", "v2-buggy")
    return d


def differential_git():
    try:
        d = _make_repo()
    except Exception as e:
        check("differential_git", False, f"git fixture failed: {e}"); return
    r = D.git_differential(d, "s.py", "scale", [1, 2, 5, 10], "HEAD~1", "HEAD")
    ok = r.diverged and r.tested == 4 and len(r.divergences) == 4
    check("differential_git", ok, f"diverged={r.diverged} {r.detail}")
    print(f"      → real git repo (v1 correct → v2 buggy): {r.detail}; the last passing commit IS the spec.")


def regression_bug_caught():
    r = D.differential(OLD, NEW, "scale", [1, 2, 5, 10])
    sample = [(d.inp, d.ref_out, d.new_out) for d in r.divergences[:2]]
    ok = r.diverged and (5, 10, 4) in [(d.inp, d.ref_out, d.new_out) for d in r.divergences]
    check("regression_bug_caught", ok, f"divergences={sample}")
    print(f"      → differential: scale(5) was 10, now 4 — divergence pinpoints the regression input.")


def n_version_outlier():
    impls = ["def scale(x):\n return x*2\n", "def scale(x):\n return x+x\n", "def scale(x):\n return x-1\n"]
    nv = D.n_version(impls, "scale", [1, 2, 5, 10])
    ok = nv.outliers == [2] and nv.majority_index in (0, 1)
    check("n_version_outlier", ok, f"majority={nv.majority_index} outliers={nv.outliers}")
    print(f"      → N-version: 2/3 implementations agree (x*2); implementation #2 (x-1) is the outlier.")


def property_invisible_via_diff():
    # B's metamorphic properties on the CORRECT (scalar) domain: x-1 breaks none of them → invisible
    import hir
    import properties as PR
    import property_test as PT
    f = hir.to_hir(NEW, "scale.py").module.fn("scale")
    fn = PR.compile_callable(f)
    props = PR.extract_properties(f, sample=5)              # scalar shape → determinism only
    rep = PT.test_properties(fn, props, [1, 2, 5, 10, -3, 0])   # scalar inputs (the right domain)
    b_blind = rep.violated_properties() == []               # NO property catches the x-1 bug
    # differential vs the old version DOES catch it
    diff = D.differential(OLD, NEW, "scale", [1, 2, 5, 10])
    ok = b_blind and diff.diverged
    check("property_invisible_via_diff", ok, f"B_violated={rep.violated_properties()} diff_caught={diff.diverged}")
    print(f"      → B's properties on scale_bug (scalar domain): {rep.held_properties()} all hold, "
          f"violated={rep.violated_properties()} — property-INVISIBLE; differential vs the prior version "
          f"catches it. The v16 blind spot, covered.")
    print("        HONEST: differential needs a reference (prior version / other impls). With none it "
          "cannot run; property+differential together still miss bugs no oracle distinguishes.")


if __name__ == "__main__":
    print("v17 Part E · E1 — differential specs (property-invisible bugs)")
    differential_git(); regression_bug_caught(); n_version_outlier(); property_invisible_via_diff()
    print(f"\nE1: {len(PASS)} passed, {len(FAIL)} failed, {len(SKIP)} skipped")
    sys.exit(1 if FAIL else 0)
