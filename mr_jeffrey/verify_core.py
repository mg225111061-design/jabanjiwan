"""
AI Code Verifier — v2 (stronger)
=================================
Fixes the two misses from v1:
  - CASE 1 miss: weak input gen (no mixed-case/space palindromes). v2 adds
    structure-aware string/int/list generators + RANDOMIZED fuzzing.
  - CASE 4 miss: side-effect (input mutation) was invisible. v2 snapshots every
    input before the call and checks it wasn't mutated.
Still honest: "REFUTED" only with a real counterexample; otherwise
"VERIFIED (bounded)" — never a fake proof. Exact numeric proof comes from JEFF.
"""
import copy, random, string
from dataclasses import dataclass, field
from typing import Callable, Any

@dataclass
class Counterexample:
    inputs: tuple
    expected: Any
    got: Any
    kind: str
    detail: str = ""

@dataclass
class VerdictReport:
    verdict: str
    tests_run: int = 0
    counterexamples: list = field(default_factory=list)
    note: str = ""
    def __str__(self):
        if self.verdict == "VERIFIED":
            return f"VERIFIED (bounded) -- {self.tests_run} tests, no counterexample. {self.note}"
        if self.verdict == "REFUTED":
            lines = [f"REFUTED -- {len(self.counterexamples)} counterexample(s) / {self.tests_run} tests:"]
            seen = set()
            for c in self.counterexamples:
                sig = (c.kind, repr(c.inputs))
                if sig in seen: continue
                seen.add(sig)
                if c.kind == "crash":
                    lines.append(f"   - {c.inputs!r} -> CRASHED: {c.detail}")
                elif c.kind == "wrong_output":
                    lines.append(f"   - {c.inputs!r} -> got {c.got!r}, expected {c.expected!r}")
                elif c.kind == "side_effect":
                    lines.append(f"   - {c.inputs!r} -> MUTATED its input: {c.detail}")
                else:
                    lines.append(f"   - {c.inputs!r} -> property violated: {c.detail}")
                if len(lines) >= 7: break
            return "\n".join(lines)
        return f"ERROR -- {self.note}"

def _int_edge():
    return [0,1,-1,2,-2,3,10,-10,100,-100,7,13,2**31,-(2**31),2**31-1,999983,-999983]
def _int_random(n):
    out=[]
    for _ in range(n):
        r=random.random()
        if r<0.3: out.append(random.randint(-20,20))
        elif r<0.6: out.append(random.randint(-10**6,10**6))
        else: out.append(random.randint(-10**12,10**12))
    return out
def _str_structured():
    return ["","a","ab","aba","abc","racecar","hello","  ","   ","AaBb","12321",
            "Hello, World!","A man a plan a canal Panama","Race car",
            "Was it a car or a cat I saw?","No lemon, no melon",
            "Able was I ere I saw Elba","Madam","Not a palindrome",
            "almost emostla","AbA aBa"]
def _str_random(n):
    out=[]; alphabets=[string.ascii_lowercase,string.ascii_letters,
        string.ascii_letters+" ",string.ascii_letters+" ,.!?"]
    for _ in range(n):
        alpha=random.choice(alphabets); L=random.randint(0,12)
        s=''.join(random.choice(alpha) for _ in range(L))
        if random.random()<0.3 and len(s)>1: s=s+s[::-1]
        out.append(s)
    return out
def _list_structured():
    return [[],[0],[1],[-1],[1,2,3],[3,1,2],[5,5,5],[-3,-1,-2],[10,-10,0],
            list(range(20)),list(range(20,0,-1)),[10**6,-10**6],[1,1,2,3,5,8],
            [2,2,2,2],[-1,-2,-3,-4,-5]]
def _list_random(n):
    out=[]
    for _ in range(n):
        L=random.randint(0,15); out.append([random.randint(-1000,1000) for _ in range(L)])
    return out
def _samples(kind,fuzz):
    if kind=="int": return _int_edge()+_int_random(fuzz)
    if kind=="nonneg_int": return [0,1,2,3,5,7,10,15,20,30,50,100]+[random.randint(0,200) for _ in range(fuzz)]
    if kind=="str": return _str_structured()+_str_random(fuzz)
    if kind=="list_int": return _list_structured()+_list_random(fuzz)
    raise ValueError(f"unknown arg kind: {kind}")

class CodeVerifier:
    def __init__(self, max_tests=800, fuzz_per_arg=60, seed=12345):
        self.max_tests=max_tests; self.fuzz=fuzz_per_arg; random.seed(seed)
    def verify(self, candidate, arg_kinds, reference=None, properties=None,
               examples=None, pure=True):
        if reference is None and not properties and not examples:
            return VerdictReport("ERROR", note="give a reference, properties, or examples")
        sample_lists=[_samples(k,self.fuzz) for k in arg_kinds]
        test_inputs=self._combine(sample_lists)
        counters,run=[],0
        for inputs in test_inputs:
            run+=1
            snapshot=copy.deepcopy(inputs); call_args=copy.deepcopy(inputs)
            try:
                got=candidate(*call_args)
            except Exception as e:
                counters.append(Counterexample(inputs,None,None,"crash",f"{type(e).__name__}: {e}"))
                if len(counters)>=10: break
                continue
            if pure and call_args!=snapshot:
                changed=[(snapshot[i],call_args[i]) for i in range(len(snapshot)) if call_args[i]!=snapshot[i]]
                counters.append(Counterexample(inputs,None,got,"side_effect",
                    f"arg changed {changed[0][0]!r} -> {changed[0][1]!r}"))
                if len(counters)>=10: break
                continue
            if reference is not None:
                try: expected=reference(*copy.deepcopy(inputs))
                except Exception: expected="<reference crashed>"
                if got!=expected:
                    counters.append(Counterexample(inputs,expected,got,"wrong_output"))
                    if len(counters)>=10: break
                    continue
            if examples and inputs in examples and got!=examples[inputs]:
                counters.append(Counterexample(inputs,examples[inputs],got,"wrong_output"))
                if len(counters)>=10: break
                continue
            if properties:
                broke=False
                for prop in properties:
                    try: ok=prop(inputs,got); name=getattr(prop,"__name__","property")
                    except Exception as e: ok,name=False,f"raised {e}"
                    if not ok:
                        counters.append(Counterexample(inputs,None,got,"property_violated",name))
                        broke=True; break
                if broke:
                    if len(counters)>=10: break
                    continue
        if counters:
            return VerdictReport("REFUTED",tests_run=run,counterexamples=counters)
        return VerdictReport("VERIFIED",tests_run=run,
            note="(bounded+fuzzed -- strong evidence; exact proof for numeric specs via JEFF)")
    def _combine(self, sample_lists):
        if len(sample_lists)==1:
            return [(x,) for x in sample_lists[0]][:self.max_tests]
        import itertools
        out=[]; m=max(len(s) for s in sample_lists)
        for i in range(m):
            out.append(tuple(s[i%len(s)] for s in sample_lists))
        small=[s[:5] for s in sample_lists]
        for combo in itertools.product(*small):
            out.append(combo)
            if len(out)>=self.max_tests: break
        seen,uniq=set(),[]
        for t in out:
            k=repr(t)
            if k not in seen: seen.add(k); uniq.append(t)
        return uniq[:self.max_tests]
