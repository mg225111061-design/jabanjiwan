# Mr. Jeffrey (`mr`)

**Fast, decisive, honest verification of AI-written code.**
The power is in the *verification*, not the model. Models — Claude, GPT, open-source — make
mistakes, and no amount of prompting makes them perfect. Mr. doesn't try. Mr. *catches* the
mistake, hands the model the **exact input that breaks it**, and for math-shaped code **proves the
answer for all inputs**. Imperfect model + perfect verification + a self-correction loop =
output you can trust.

> One line: **don't make the model perfect — make the verification perfect.**

---

## The three honest tiers (never blurred)

| Verdict | Meaning | How |
|---|---|---|
| 🟢 **PROVEN** | exact, **all inputs** | JEFF exact coefficient-zero engine → sympy CAS |
| 🟡 **VERIFIED** | bounded — **strong evidence, NOT a proof** | edge cases + randomized fuzz + timeout guard |
| 🔴 **REFUTED** | here is the **breaking input** | a real counterexample, shrunk to its smallest form |

**We never call "VERIFIED" a proof.** A clean fuzz run means *no counterexample was found in the
inputs we tried* — an unseen input may still hold a bug. Only the exact tier (PROVEN) covers all
inputs, and only for the math-shaped part of the code. This distinction is the product.

- **Math / algebraic code** → can be **PROVEN** (closed-form identities, all inputs).
- **General code** → **strong hunting** (VERIFIED bounded, or REFUTED with a witness).
- The two are **honestly distinguished** — no overhyped "proven" stamp on a fuzz run.

---

## Install

Pure standard library, plus **sympy** for the exact CAS tier:

```bash
pip install sympy            # required for `prove` / exact tier
# optional: build the real JEFF exact backend (Rust) for proof-carrying identity checks:
#   cargo build -p jeff-math --example jeff_identity     (from the repo root)
# optional: export ANTHROPIC_API_KEY or OPENAI_API_KEY to use `mr solve` with a real model
```

If the JEFF binary isn't built, the exact tier transparently falls back to sympy (still exact),
and says so. If no API key is set, `mr solve` says so and points you at the offline commands.

---

## CLI

```bash
# 1) PROVE an identity for ALL inputs (exact: JEFF → sympy)
python3 mr.py prove "(x+1)**2" "x**2 + 2*x + 1" --vars x
#   → PROVEN  [jeff]

python3 mr.py prove "n*n/2" "n*(n+1)/2" --vars n
#   → REFUTED [jeff]  exact disagreement: degree=1 residual=-1/2

# 2) HUNT for bugs in a function (bounded + fuzz + timeout guard)
python3 mr.py verify mycode.py --func average --spec spec.py
#   spec.py defines:  reference(...)  and/or  properties=[...]  and/or  arg_kinds=[...]

# 3) SOLVE: a model writes the function, Mr. verifies + feeds back the breaking input until it holds
python3 mr.py solve "sort a list ascending" --func my_sort --kinds list_int --backend auto
#   (needs ANTHROPIC_API_KEY / OPENAI_API_KEY; works offline only via the library + ScriptedLLM)

# 4) DEMO: the showcase — 10 real AI bugs, 10 decisive verdicts
python3 mr.py demo
```

Verdicts are color-coded (PROVEN green / VERIFIED yellow / REFUTED red; honors `NO_COLOR`).
Exit codes: `0` = PROVEN/VERIFIED, `1` = REFUTED, `3` = DEFER / needs-a-model / other.

### `--spec` file format (for `verify`)

```python
# spec.py
def reference(xs):                 # a known-correct version (optional)
    return max(xs) if xs else 0
def is_nonneg(inputs, output):     # named properties (optional)
    return output >= 0
properties = [is_nonneg]
arg_kinds = ["list_int"]           # overrides --kinds (optional)
```

Supported `arg_kinds`: `int`, `nonneg_int`, `float`, `str`, `list_int`, `list_float`,
`dict`, `tuple`, `list2d`, `bool`.

---

## Web demo (no Flask — pure stdlib)

```bash
python3 mr_web.py --port 8000
# open http://127.0.0.1:8000  — paste a function, or prove an identity.
```

JSON API: `POST /api/prove {cand, ref, vars}` · `POST /api/verify {code, func, kinds, reference}`
· `GET /health`.
⚠ **Local developer demo only**: the bug-hunt executes the code you submit (inside Mr.'s
file/network sandbox, but in-process). Don't expose it to untrusted input.

---

## Library API

```python
from verify_strong import StrongVerifier        # bounded hunting (stage 2)
from jeff_adapter   import prove_identity         # exact proof (stage 3): JEFF → sympy → defer
from mr_jeffrey     import MrJeffrey, Task        # self-correction loop (stage 1)
from llm_adapters   import get_adapter, ScriptedLLM

# prove (all inputs)
prove_identity("(x+1)**2", "x**2+2*x+1", ["x"])   # -> ExactResult(PROVEN, backend='jeff')

# hunt (bounded)
StrongVerifier().verify(my_fn, ["list_int"], reference=sorted, func_name="my_sort")

# self-correct against ANY llm(prompt)->str  (real adapter, or ScriptedLLM for offline/tests)
llm = get_adapter("auto")                          # real model if a key is set, else simulation
MrJeffrey(llm).solve(Task(name="sq", prompt="square n", func_name="sq", arg_kinds=["int"],
                          reference=lambda n: n*n))
```

---

## How it works (architecture)

```
            ┌──────────────── Mr. Jeffrey ────────────────┐
  task ───▶ │  LLM adapter   →   writes code               │   stage 1  (llm_adapters.py)
            │       ▲                 │                     │
            │       │   exact breaking input (feedback)     │
            │       │                 ▼                     │
            │  ┌─ verification ──────────────────────────┐ │
   code ───▶│  │ StrongVerifier  bounded hunt + shrink    │ │   stage 2  (verify_strong.py)
            │  │   crash / wrong-output / side-effect /   │ │
            │  │   property / TIMEOUT(no-hang)            │ │
            │  │ jeff_adapter    EXACT proof, all inputs  │ │   stage 3  (jeff_adapter.py
            │  │   JEFF coeff-zero → sympy → defer        │ │              + jeff-math/examples)
            │  └──────────────────────────────────────────┘ │
            └────────────────────────────────────────────────┘
                       CLI (mr.py) · web (mr_web.py) · demo      stage 4
```

The exact tier calls the **real JEFF** engine: `crates/jeff-math/examples/jeff_identity.rs` decides
`candidate ≡ reference` via `UniPoly::from_coeffs` + `is_zero` over `BigRational` — the same
coefficient-zero mechanism the rest of the JEFF/GACC compiler uses for proof-carrying collapse.

---

## Honest limits (read this)

- **VERIFIED is not a proof.** It's bounded evidence. An input outside the tested set can still
  break the function. Use the exact tier (`prove`) for guarantees, and it only applies to
  math-shaped claims.
- **The exact tier's reach.** JEFF here proves **univariate polynomial** identities directly;
  multivariate / transcendental identities fall through to sympy (still exact). JEFF's deeper
  reach (telescoper / fold certificates for structured sums) exists in the Rust engine but is not
  yet exposed through this CLI — stated, not implied.
- **`solve` needs a model.** Without an API key, the loop has no code-writer; the verification
  half still runs offline (`verify`, `prove`, `demo`, or the library with `ScriptedLLM`).
- **The web demo runs your code.** Local/dev only.
- **No fake passes, ever.** If Mr. can't decide, it says DEFER — it does not pretend.

---

## Tests

```bash
python3 test_stage1.py   # LLM adapters + loop          (5)
python3 test_stage2.py   # strengthened verification    (7)
python3 test_stage3.py   # JEFF exact engine            (6)
python3 test_stage4.py   # CLI / web / showcase         (9)
```
