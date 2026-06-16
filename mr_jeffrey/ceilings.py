"""
STAGE S1 (v10) — the ceilings, documented with rationale (pre-product completion).
=================================================================================
"Pre-product completion" = we filled what is fillable and CONFIRMED, with rationale, what is not —
distinguishing FUNDAMENTAL mathematical ceilings (nobody can cross them) from ENGINEERING DEFERRALS
(crossable with more tools/effort). These are recorded so there is no remaining *unknown* confession.
"""
from __future__ import annotations

from dataclasses import dataclass


@dataclass
class Ceiling:
    name: str
    kind: str           # "FUNDAMENTAL" (math/theory limit) | "TOOL" (needs another tool) | "ENGINEERING"
    rationale: str
    where: str          # where HARAN hits it / how it's handled


FUNDAMENTAL = [
    Ceiling("holonomic order-2+ summation (e.g. Franel ΣC(n,k)³)", "FUNDAMENTAL",
            "Creative-telescoping / non-commutative Gröbner bases are double-exponential (Mayr–Meyer "
            "EXPSPACE-completeness of ideal membership). Not a HARAN weakness — the worst case is "
            "intrinsic to the algebra.",
            "v5 T3: measured timeout > order-1; classified DEFER."),
    Ceiling("RIP (Restricted Isometry Property) static certification", "FUNDAMENTAL",
            "Deciding RIP of an arbitrary sensing matrix is NP-hard (Tillmann–Pfetsch). No verifier can "
            "do it in general.",
            "v6 U2: replaced by a per-execution RUNTIME residual certificate."),
    Ceiling("non-holonomic equivalence / zero-testing", "FUNDAMENTAL",
            "Richardson's theorem: equality of expressions in a sufficiently rich class is UNDECIDABLE; "
            "prime-counting / chaotic recurrences are non-holonomic.",
            "v2/v5: classified NO_STRUCTURE (Ω(N)), never claimed closed."),
    Ceiling("Ω(N) information floor on data-dependent work", "FUNDAMENTAL",
            "Reading the answer requires touching the data — no closed form avoids it (conservation law).",
            "v4/v4.5: unstructured = constant-factor only, Ω(N) never broken."),
    Ceiling("#P / NP-hard counting & optimization", "FUNDAMENTAL",
            "Permanent (#P, Valiant), Ising ground state (NP-hard) — not dissolved by reformulation.",
            "design PART 18: refused / NO_STRUCTURE."),
]

TOOL_OR_ENGINEERING = [
    Ceiling("unbounded ∀-proof of sort", "TOOL",
            "Z3 has no automatic induction over recursion; the property is provable but needs an "
            "inductive prover (Coq/Isabelle) or hand-written induction lemmas.",
            "v8 Q1: Z3 ∀-VALUES proof at length ≤ 4 done; unbounded DEFERRED."),
    Ceiling("probabilistic (ε,δ) TAIL bound (Chernoff)", "TOOL",
            "Caesar proves the EXPECTATION (first moment); the high-probability tail needs heavier "
            "concentration reasoning in HeyVL.",
            "v6.5 C4: expectation PROVEN (Caesar); tail DEFERRED."),
    Ceiling("Kovacic Cases 2 & 3 with poles", "ENGINEERING",
            "Decidable (Kovacic is complete for order-2) but the full case analysis is unimplemented.",
            "v5 T4: Case 1 + odd-degree-absence done; rest → UNKNOWN (honest, not ABSENT)."),
    Ceiling("full LLVM native backend (general code, all types, bignum)", "ENGINEERING",
            "A complete optimizing backend is months of engineering; native long-long path only.",
            "v9 R1/R2: collapsing folds → native O(1); general/bignum DEFERRED."),
]


def all_ceilings():
    return FUNDAMENTAL + TOOL_OR_ENGINEERING


def render():
    out = ["HARAN ceilings (pre-product completion):", "", "FUNDAMENTAL (mathematical — nobody crosses these):"]
    for c in FUNDAMENTAL:
        out.append(f"   ✗ {c.name}\n       why: {c.rationale}\n       HARAN: {c.where}")
    out.append("\nTOOL / ENGINEERING (crossable with more tools/effort — not math limits):")
    for c in TOOL_OR_ENGINEERING:
        out.append(f"   ⧖ {c.name} [{c.kind}]\n       why: {c.rationale}\n       HARAN: {c.where}")
    return "\n".join(out)
