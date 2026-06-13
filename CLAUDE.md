# CLAUDE.md — JEFF Build Constitution (Full / Expanded)

> 이 파일은 JEFF/GACC 저장소의 최상위 지침이다. Claude Code는 모든 작업 전에 이 문서를 읽고, 충돌 시 **PART 1의 PRIORITY HIERARCHY가 다른 모든 것을 이긴다.** 이 문서는 채워야 할 줄 수가 아니라 *지켜야 할 규율*이다. 한 줄도 헛되지 않게 작성되었으니, 한 줄도 무시하지 마라.
>
> 본 확장본은 압축본의 모든 규율에 더해, *구현 수준의 detail* — 크레이트별 Rust API skeleton, typing rule, collapser 알고리즘 pseudocode, certificate 예시, barrier detection sketch, fixture 카탈로그, 진단 메시지 카탈로그, Stage 0 코드 stub — 을 담는다.

---
---

# 목차

- PART 0 — 사용법 (READ ORDER & PRECEDENCE)
- PART 1 — MISSION & PRIORITY HIERARCHY
- PART 2 — HONESTY DISCIPLINE, OPERATIONALIZED
- PART 3 — HARD RULES (R1..R40)
- PART 4 — REFERENCE SPECS
- PART 5 — TECH STACK & REPO LAYOUT
- PART 6 — CRATE-BY-CRATE API SKELETONS
- PART 7 — TYPE SYSTEM IMPLEMENTATION SPEC (typing rules)
- PART 8 — IR STACK (surface → core → dialects → JLIR → LLVM)
- PART 9 — BUILD ORDER / MILESTONES (Stage 0..8)
- PART 10 — PER-LAYER IMPLEMENTATION GUIDES
- PART 11 — CERTIFICATE SCHEMA & VERIFICATION (with worked examples)
- PART 12 — HONEST_DEFER TAXONOMY (detection sketches + diagnostics)
- PART 13 — DIAGNOSTIC MESSAGE CATALOG
- PART 14 — TESTING STRATEGY & FIXTURE CATALOG
- PART 15 — CI GATES (scripts)
- PART 16 — CLAUDE CODE WORKING STYLE
- PART 17 — GLOBAL DEFINITION OF DONE
- PART 18 — ANTI-PATTERNS / NEVER DO
- PART 19 — KICKOFF TASK (Stage 0, with code stubs)
- PART 20 — STANDING PROMPTS (reusable checklists)
- PART 21 — WORKED END-TO-END WALKTHROUGHS
- PART 22 — GLOSSARY / QUICK REFERENCE

---
---

# PART 0 — 이 문서 사용법 (READ ORDER & PRECEDENCE)

## 0.1 읽는 순서
1. **CLAUDE.md** (이 파일) — 규율·우선순위·룰·작업 방식·구현 가이드. 항상 먼저.
2. **jeff-language-specification.md** — 언어 그 자체(문법·타입·semantics·stdlib). 표면 언어 detail의 원천.
3. **gacc-architecture.md** — 컴파일러 아키텍처(Layer 0–4, dispatch, barrier, 정리적 경계).
4. **gacc-backend-layer.md** — 백엔드 Layer B(상수배 최적화, JLIR, secret-taint guard).

## 0.2 우선순위 규칙
- 이 문서의 **PART 1(PRIORITY HIERARCHY)이 절대 우선**한다. spec 문서와 충돌하면 이 문서가 이긴다. spec 문서끼리 충돌하면 language-spec → architecture → backend 순.
- spec 문서는 *detail의 원천*이다. 알고리즘·자료구조·certificate 형식을 **재발명하지 말고** spec의 섹션 번호를 인용하라 (예: "per language-spec §3.4", "per architecture §Layer-2").
- 이 CLAUDE.md가 제공하는 코드 skeleton·typing rule·pseudocode는 *구현 지침*이다 — 정확히 따르되, spec과 충돌하면 spec(의미)이 이기고 이 문서를 고쳐 PR하라(DR6).
- 모르면 추측하지 말고 **멈추고 물어라**. 정직성 규율(PART 2)은 너 자신의 작업 보고에도 적용된다.

## 0.3 이 문서가 *아닌* 것
- 코드의 복사본이 아니다. 코드는 저장소에 있고, 이 문서는 *어떻게·무엇을·왜* 다. skeleton은 시작점이지 최종 코드가 아니다.
- 마케팅이 아니다. 과장·미사여구 금지. 사실과 지시만.
- 줄 수를 채우기 위한 문서가 아니다. 모든 줄이 실질이다. 너도 그렇게 작업하라(R30).

---
---

# PART 1 — MISSION & PRIORITY HIERARCHY (비협상 / NON-NEGOTIABLE)

## 1.1 Mission (한 문단)
JEFF는 검증된 수치 커널과 post-quantum cryptography를 위한 언어다. 그 컴파일러 GACC는 *구조적으로 생성된 work*(closed form으로 환원 가능한 것)를 기계 검증된 certificate와 함께 점근적으로 붕괴시키고, *데이터 의존 work*는 하드웨어 한계 속도로 돌리며, *수학적으로 불가능한 것*은 named barrier로 정직하게 거부한다. 핵심 불변식: **표현은 공짜지만 구조는 아니다.**

## 1.2 THE PRIORITY HIERARCHY
충돌하면 **번호가 낮은 것이 항상 이긴다.** 이것이 이 프로젝트에서 가장 중요한 단 하나의 규칙이다.

```
P0  NEVER MISCOMPILE        — 정확성이 모든 것 위에 있다.
P1  HONESTY DISCIPLINE      — 가짜 collapse·가짜 certificate·overclaim 금지.
P2  PROOF-CARRYING          — 모든 collapse는 기계 검증된 certificate를 낸다.
P3  CONSERVATION LAW        — 구조적 work만 붕괴; 불가능한 것은 거부.
P4  PERFORMANCE             — 점근 붕괴(structure) + 상수배(deferred).
P5  ERGONOMICS / DX         — Python-급 표면, 좋은 진단, 좋은 도구.
```

각 항목의 *코딩할 때의 의미*:

**P0 — NEVER MISCOMPILE.** 어떤 변환도 의미를 바꾸면 안 된다. 모든 collapse·모든 백엔드 변환은 *모든 경계에서 fallback*을 가진다. 검증이 실패하면 원본 루프를 그대로 코드젠한다. *최악의 허용 결과는 "놓친 최적화"이고, 결코 "틀린 답"이 아니다.* 이 한 줄을 위반하는 PR은 무조건 reject. 코드에서: 모든 `collapse()`는 `Result<Collapsed, Defer>`를 반환하고, `Collapsed`는 검증된 certificate를 동반하지 않으면 *구성될 수 없다*(type-level enforcement, PART 6).

**P1 — HONESTY DISCIPLINE.** collapse가 안 되면 *왜* 안 되는지 named tag(PART 12)로 말한다. 검증 안 된 certificate를 emit하지 않는다. 측정하지 않은 가속을 주장하지 않는다. 조용히 근사하지 않는다. *너 자신의 작업 보고도* 정직하라 — 막히면 막혔다고 말하고, 가짜로 테스트를 통과시키지 마라.

**P2 — PROOF-CARRYING.** 모든 collapse는 Certificate(PART 11)를 낸다. checker가 그것을 검증한다(Z3 기본, holonomic은 Lean). **collapser보다 checker를 먼저 구현한다**(R2). 검증할 수 없는 collapse는 collapse가 아니라 추측이다 → defer.

**P3 — CONSERVATION LAW.** 세 벽을 외워라: (a) 진성 data-dependent work는 Ω(N) floor — 압축 불가. (b) #P/NP-hard는 보존된다 — 좌표를 바꿔도 안 사라진다. (c) undecidable(Rice/halting)은 computable re-encoding에 불변. *구조*(low-rank, sparse, affine, integrable, periodic, holonomic)가 있을 때만 붕괴하라. 구조가 없으면 거부하라.

**P4 — PERFORMANCE.** collapse = 점근(structured fragment). 백엔드 = 상수배(deferred fragment). 둘은 곱해진다. 성능은 *P0–P3를 위반하지 않는 한에서만* 추구한다. 빠르지만 틀리거나 정직하지 않은 것은 실패다.

**P5 — DX.** 좋은 진단(특히 barrier 진단), 좋은 에러 메시지, Python-급 표면 문법. 단 P0–P4 다음.

## 1.3 한 줄 요약 (외워라)
> **구조는 certificate와 함께 붕괴시키고, 나머지는 하드웨어 한계로 돌리고, 불가능한 것은 이름을 붙여 거부한다 — 그리고 결코 틀린 답을 내지 않는다.**

## 1.4 우선순위 충돌 결정표 (실전)
| 상황 | 결정 |
|---|---|
| collapse가 빠른데 certificate가 검증 안 됨 | **defer** (P2 > P4) |
| collapse 가능하나 구조가 진짜 없음(추측) | **defer + tag** (P3 > P4) |
| 최적화가 secret 경로 timing을 바꿈 | **거부** (P1/보안 > P4) |
| 변환이 의미 보존하나 비결정적 산출 | **거부/수정** (P0 결정성, R11) |
| 테스트 통과를 위해 정답 하드코딩 유혹 | **금지 → HONEST_DEFER 구현** (P1) |
| 더 빠른 GPL 라이브러리 존재 | **링크 금지, clean-room 또는 oracle** (R5) |

---
---

# PART 2 — THE HONESTY DISCIPLINE, OPERATIONALIZED

## 2.1 보존법칙 (왜 정직해야 하는가)
JEFF의 가치 제안 전체가 "구조적 work는 붕괴, 데이터 의존 work는 floor"라는 경계 위에 있다. 이 경계를 흐리는 순간 — 가짜 collapse 하나, 검증 안 된 certificate 하나 — JEFF는 그냥 또 하나의 거짓말하는 컴파일러가 된다. 정직성은 *기능*이지 제약이 아니다. 이 프로젝트의 차별점 전부가 여기서 나온다.

## 2.2 빌더를 위한 정직성 규칙 (DR1..DR8)
- **DR1.** 검증되지 않은 certificate를 emit하지 마라. certificate는 *실제로 checker를 통과*해야 한다. 통과 안 하면 collapse 안 한다.
- **DR2.** 측정하지 않은 speedup을 코드·주석·문서·벤치마크에 적지 마라. 측정했으면 *kernel과 N*과 함께 적고 "non-uniform"임을 명시하라.
- **DR3.** 조용히 근사하지 마라. 정확해야 할 곳(collapse 경로)에서 근사하면 그건 버그다. 근사가 불가피하면(예: float 커널) 타입·계약·문서에 명시하라.
- **DR4.** 모든 barrier에서 named tag(PART 12)를 emit하라. "그냥 최적화 안 함"은 금지. *왜* 안 되는지 말하라.
- **DR5.** 너 자신에게 정직하라. 막히면 "막혔다, 이유는 X"라고 보고하라. 테스트를 통과시키려고 구현을 가짜로 만들지 마라(예: 정답을 하드코딩, oracle을 우회, 검사를 비활성화). 그런 충동이 들면 **멈추고 HONEST_DEFER 경로를 구현하라.**
- **DR6.** spec과 다르게 구현하고 싶으면, 먼저 *왜*를 적고 승인을 구하라. 침묵의 일탈 금지. spec/CLAUDE.md를 고쳐야 하면 그 변경도 PR에 포함.
- **DR7.** "거의 맞음"은 collapse 경로에서 "틀림"이다. 1개 입력에서라도 원본과 다르면 그 collapse는 실패다.
- **DR8.** 불확실성을 숨기지 마라. solver가 `unknown`을 주면 그건 "검증됨"이 아니다 → defer. 추정 treewidth가 경계 근처면 보수적으로(budget 안쪽만 collapse).

## 2.3 "가짜로 만들고 싶을 때" 프로토콜
테스트가 안 통과해서 무언가를 우회·하드코딩·비활성화하고 싶어지면:
1. 멈춰라.
2. 이게 P0(miscompile) 또는 P1(honesty) 위반인지 자문하라.
3. 위반이면, 그 기능을 *지금은 할 수 없다*고 인정하고, 대신 (a) HONEST_DEFER 경로를 구현하거나, (b) 정확한 blocker를 보고하라.
4. 절대 — 통과하는 것처럼 *보이게* 만들지 마라. (대화 기록의 문서 93 "정답을 potential에 심고 찾는" 순환 코드가 정확히 이 안티패턴이다. 답을 안 다음에 답을 찾는 건 검색이 아니다.)

## 2.4 정직한 작업 보고 형식 (Claude Code 출력)
각 작업 단위 끝에:
```
DONE: <무엇을 구현했나>
VERIFIED: <어떤 테스트/검증이 green인가> (CI gate 이름)
DEFERRED/BLOCKED: <안 된 것 + 정확한 이유 + tag 또는 blocker>
NEXT: <다음 단계>
```
"전부 됐다"는 PART 17(DoD)를 *전부* 충족할 때만. 아니면 무엇이 남았는지 명시.

---
---

# PART 3 — HARD RULES (R1..R40, 전부 검증 가능)

각 룰은 위반 시 PR reject 사유다. CI가 강제하는 것은 [CI].

**정확성 / fallback**
- **R1** [CI] 모든 collapser와 모든 백엔드 변환은 *fallback 경로*와 *fallback 테스트*를 가진다. 검증 실패 → 원본. (P0)
- **R2** collapser를 구현하기 전에 그 certificate의 checker를 먼저 구현하고 테스트한다. (P2)
- **R3** [CI] collapse를 내는 모든 기능은 (a) certificate-verification 테스트와 (b) fallback 테스트를 가진다. 둘 없으면 merge 불가.
- **R7** [CI] 테스트 없는 기능 merge 금지. 최소 unit + (해당 시) property-based.
- **R11** [CI] 결정성: 동일 입력+플래그 → 동일 컴파일 산출물(certificate 포함). non-determinism은 버그.
- **R16** certificate는 self-contained: source_ref, collapsed_ref, proof_obligation, evidence, boundary_conditions, fallback 포함(PART 11 schema).
- **R17** 한 collapser가 다른 collapser 출력에 의존할 때, certificate 체인을 명시·검증 가능하게.
- **R18** [debug-assert] IR 불변식 검사: SSA well-formedness, type 보존, metadata 일관성(특히 secret_taint, dependence).
- **R23** 외부 solver(Z3/Lean) 호출은 timeout과 fallback을 가진다. 미응답 → defer, 절대 hang 금지.
- **R25** [CI] 모든 collapse 출력에 golden test(입력→collapsed form→certificate) 고정. 회귀 방지.
- **R31** verify 결과 `unknown`/`timeout`은 `valid`가 아니다 → 원본 fallback (DR8).
- **R32** collapser는 부분 결과를 코드젠하지 않는다 — 전체 collapse가 검증되거나, 전체 fallback. (no half-collapse)

**정직성**
- **R4** 모든 barrier에서 PART 12의 named tag를 emit. silent skip 금지. (P1)
- **R8** [CI] 조작된 벤치마크 금지. 수치 하드코딩 불가, 측정만. speedup은 kernel+N과 함께, non-uniform 표시.
- **R9** PART 18 non-goal이 코드·설계에 나타나면 거부.
- **R24** 부분 구현은 `HONEST_DEFER` 또는 `unimplemented!("reason")`로 표시. 조용한 no-op 금지.
- **R30** 문서·주석·커밋에 과장 금지. "수천 배"·"지수적"·"O(1)"은 증명·측정·구조 전제부와 함께만.
- **R33** collapse 경로 산술은 정확(`int/nat/rat/mod`). closed-form certificate에 float 금지. (= 구 R10)

**보존법칙**
- **R19** stdlib의 모든 numeric 커널은 *활용 구조*를 타입/계약으로 명시하고, 구조 부재 시 `constant-factor-only` 또는 적절한 tag로 강등.
- **R34** Total mode 함수는 증명 가능하게 종료(structural/sized recursion, guarded corecursion). 종료 증명 못 하면 General로 강등하거나 reject. (= 구 R14)
- **R35** General mode collapse는 best-effort. 종료를 결정하지 마라. absint로 sound 상한만. (= 구 R15)

**보안 / 라이선스**
- **R5** [CI] 라이선스 게이트: 링크 의존성은 **MIT/Apache-2.0/BSD만**. **barvinok·LattE·PPL·GMP·M4RI 링크 금지.** Barvinok decomposition·Four-Russians는 **clean-room**. isl는 `--with-int=imath`. GPL 도구는 `jeff-test-oracles`에서 *out-of-process*만(dev-only). license-scan 위반 시 CI 실패.
- **R6** [CI] secret-taint: `secret[T]`에 data-dependent branch/index/timing 도입 금지. constant-time이 최적화를 이긴다. const-time-audit가 CI 게이트.
- **R28** PQC 커널은 전 구간 `secret[T]` + `@constant_time`. timing-class 동치 테스트 포함.
- **R29** 어떤 변환도 `@constant_time` 함수의 timing profile을 바꾸면, semantic 동치 certificate가 있어도 거부.
- **R36** secret 메모리는 zeroize-on-drop. secret 값은 로그·에러·디버그 출력에 누설 금지.

**엔지니어링 위생**
- **R13** [CI] 작은 검증된 증분. 모든 PR green: build + test + property + clippy + license-scan + const-time-audit + cert-replay + determinism + coverage.
- **R12** 모든 public API와 모든 barrier tag 문서화. barrier tag는 *근거 정리*와 함께.
- **R20** 에러 복구: parser·checker는 가능한 많은 진단을 모아 보고(첫 에러에서 멈추지 말 것).
- **R21** unsafe(Rust)는 격리·문서화·정당화. 모든 unsafe 블록에 `// SAFETY:` 주석.
- **R22** [CI] 모든 PR은 관련 spec 섹션 인용. 미인용 시 review 보류.
- **R26** [CI] 커버리지 게이트: collapser/checker/typechecker 코어 라인+분기 하한 유지.
- **R27** 진단은 사용자가 읽을 수 있게(왜 collapse/defer 했는지). 내부 메커니즘 누설 금지하되 barrier 이유는 명확히.
- **R37** 모든 IR 노드·certificate·diagnostic은 source span을 보존(좋은 에러·디버깅).
- **R38** API는 panic으로 사용자 입력 에러를 처리하지 않는다 — `Result`/diagnostic. panic은 내부 불변식 위반(버그)에만.
- **R39** 의존성 추가 전 라이선스 확인(R5)·minimal 원칙. 핵심 자료구조는 직접 소유.
- **R40** 모든 PR은 PART 17 DoD 체크리스트를 명시적으로 통과 표시.

---
---

# PART 4 — REFERENCE SPECS (어떻게 쓸 것인가)

- **language-spec** — §1 pillars, §3 type system, §4 dual-mode, §5 semantics + collapse-transparency 정리, §7 collapse-as-feature(@collapse/@constant_time), §9 barriers, §10 stdlib, §11 IR, §15 guarantees, §16 non-goals, §17 examples. 언어 표면·타입·semantics의 권위.
- **architecture** — Layer 0–4, dispatch 결정 절차, 각 layer의 정리적 경계(Cai–Lu trichotomy, Markov–Shi treewidth, Barvinok fixed-dim, Mayr–Meyer EXPSPACE, BBBV, Valiant #P, Gosper/Zeilberger). collapser 구현은 여기 인용.
- **backend** — Layer B 10 기법 배치, JLIR metadata envelope, 정확성 체제(transform-level / per-instance validation / delegated), secret-taint guard, isl 이중 사용(counting+scheduling), 빌드 순서. codegen·최적화는 여기 인용.

규칙: 이 문서들이 detail을 가진다. 재발명 금지. 인용하라(R22).

---
---

# PART 5 — TECH STACK & REPO LAYOUT

## 5.1 구현 언어 & 핵심 도구
- **컴파일러: Rust** (근거: egg/egglog이 Rust+MIT → Layer 0 재사용; 강타입·메모리 안전·systems-grade; cargo workspace 크레이트 분리; LLVM 바인딩 성숙). 기본값 변경은 DR6로 정당화.
- **SMT: Z3** (MIT) via `z3` crate / FFI. timeout+fallback(R23).
- **증명(holonomic cert): Lean 4** (Apache) — *external process*. 정적 링크 안 함.
- **다면체: isl** (MIT) `--with-int=imath`. Barvinok decomposition clean-room(R5).
- **codegen: LLVM** via `inkwell`/`llvm-sys`.
- **GF(2): Four-Russians clean-room** (M4RI 링크 금지).
- **property test: proptest**. **fuzz: cargo-fuzz**. **bench: criterion**(수치 하드코딩 금지, R8).

## 5.2 Cargo workspace 레이아웃
```
jeff/
├─ CLAUDE.md
├─ docs/
│  ├─ jeff-language-specification.md
│  ├─ gacc-architecture.md
│  └─ gacc-backend-layer.md
├─ crates/
│  ├─ jeff-syntax/        # lexer, parser, surface AST, source spans
│  ├─ jeff-types/         # 타입: linear/affine, sized, refinement→Z3, effect rows, secret-taint
│  ├─ jeff-core-ir/       # typed core IR (System-F + linear + refinement + effects)
│  ├─ jeff-absint/        # abstract interpretation: intervals, congruences, polyhedra(isl)
│  ├─ jeff-recognizer/    # Layer 0: egglog e-graph, rulesets, cost model, dispatch
│  ├─ jeff-collapse-arith/      # Layer 1: CR, holonomic(Gosper/Zeilberger), kernel pack
│  ├─ jeff-collapse-gf2/        # Layer 2: GF(2) partition, (M,b), Four-Russians
│  ├─ jeff-collapse-holographic/ # Layer 3: matchgate/FKT/Pfaffian/Holant, planarity
│  ├─ jeff-collapse-tensor/     # Layer 4: tensor-network, treewidth, slicing
│  ├─ jeff-barvinok/      # Layer 3b: clean-room Barvinok on isl/imath
│  ├─ jeff-cert/          # Certificate schema + evidence kinds
│  ├─ jeff-verify/        # checker routing: Z3(default), Lean(holonomic)
│  ├─ jeff-jlir/          # JLIR (SSA + metadata envelope), lowering from dialects
│  ├─ jeff-backend/       # Layer B: comptime, devirt, AoS→SoA, lifetime→free, tiling, SIMD, SMT-fold
│  ├─ jeff-codegen/       # JLIR → LLVM IR
│  ├─ jeff-stdlib/        # core, fold, numeric(kernel pack), crypto.pqc, proof, poly
│  ├─ jeffc/              # driver/CLI
│  └─ jeff-test-oracles/  # DEV-ONLY out-of-process GPL oracles. NEVER linked into release.
├─ tests/                 # 통합·골든·differential·property
├─ benches/               # 정직한 벤치마크(R8)
├─ ci/                    # license-scan, const-time-audit, cert-replay, determinism, coverage
└─ Cargo.toml             # workspace
```

## 5.3 jeffc CLI
```
jeffc build <file>            # 기본 컴파일
  --emit-certificates <dir>   # 증명 객체 산출
  --collapse-report           # collapse coverage + barrier tags (per function)
  --total                     # Total mode 강제 (종료 미증명시 에러)
  --const-time-audit          # secret-taint 경로 감사
  --emit-jlir / --emit-llvm   # IR 덤프
  --opt-level 0..3            # 백엔드 Layer B 강도
```

---
---

# PART 6 — CRATE-BY-CRATE API SKELETONS

> skeleton은 *시작점*이다. 정확히 따르되 spec과 충돌하면 spec이 이긴다(0.2). 모든 미구현은 `unimplemented!("reason")`(R24).

## 6.1 jeff-cert
```rust
pub type CollapserId = &'static str;
pub struct IrRef { pub id: NodeId, pub span: Span }

pub struct Certificate {
    pub collapser_id: CollapserId,
    pub source: IrRef,            // 원본 (semantics 포함 참조)
    pub collapsed: IrRef,         // closed/sublinear form
    pub obligation: Obligation,   // 동치 주장
    pub evidence: Evidence,
    pub boundaries: Vec<Boundary>,
    pub fallback: IrRef,          // 항상 = source
}

pub enum Evidence {
    PolynomialIdentity(Poly),                 // → Z3 (QF_NRA / int / bitvector)
    Telescoper { l: Operator, r: RatFn },     // → Z3 (poly identity) 또는 Lean (operator induction)
    Gf2LinearIdentity { m: Gf2Matrix, b: Gf2Vec }, // → Z3 over GF(2) (basis-vector)
    PfaffianHolant(HolantWitness),            // → algebraic replay
    EigenCharpoly { p: Mat, d: Mat },         // → Z3 ring identity
    NumericResidual { tol: Exact },           // → exact modular/integer recompute
}

// P0/P2 핵심: 검증된 Certificate 없이는 Collapsed를 만들 수 없다 (type-level enforcement).
pub struct Collapsed { pub residual: IrRef, cert: VerifiedCertificate }
pub struct VerifiedCertificate(Certificate); // jeff-verify만 생성 가능 (private 생성자)

pub enum CollapseOutcome { Collapsed(Collapsed), Defer(Defer) }
pub struct Defer { pub tag: BarrierTag, pub diagnostic: Diagnostic, pub original: IrRef }
```

## 6.2 jeff-verify
```rust
pub enum VerifyResult { Valid, Invalid, Unknown }  // Unknown = timeout/incapable

pub trait Checker {
    fn check(&self, ev: &Evidence, ob: &Obligation, b: &[Boundary]) -> VerifyResult;
}

pub struct Z3Checker { timeout: Duration }   // PolynomialIdentity, Gf2LinearIdentity, EigenCharpoly
pub struct LeanChecker { /* external process */ } // Telescoper(operator induction)
pub struct ReplayChecker;                    // NumericResidual, PfaffianHolant (exact recompute)

// 유일하게 VerifiedCertificate를 생성하는 함수. R31: Unknown→None.
pub fn verify(c: Certificate) -> Option<VerifiedCertificate>;
```

## 6.3 jeff-recognizer
```rust
pub enum DispatchTag {
    Holonomic, LinearStateTransition, Convolution, Gf2Affine,
    PlanarCsp, BoundedTreewidth, AffineTripCount, None,
}
pub struct Recognized { pub canonical: IrRef, pub tag: DispatchTag, pub cost: AsymptoticCost }
pub enum AsymptoticCost { Const, Log, Sublinear, Linear, Superlinear(Degree) }

pub fn recognize(ir: &CoreIr, budget: SaturationBudget) -> Recognized;
```

## 6.4 collapser 공통 trait
```rust
pub trait Collapser {
    const ID: CollapserId;
    fn entry(&self, r: &Recognized) -> bool;     // dispatch 진입 조건
    fn try_collapse(&self, ir: &CoreIr) -> CollapseOutcome;  // 반드시 Defer 가능
}
```
규칙: `try_collapse`는 결코 panic하지 않고(R38), 항상 `Collapsed`(검증된 cert 포함) 또는 `Defer`(tag 포함)를 반환(R32: half-collapse 금지).

## 6.5 jeff-jlir
```rust
pub struct JlirRegion { pub body: SsaBody, pub origin: Origin, pub meta: Meta }
pub enum Origin { Collapsed { layer: u8, cert: NodeId }, Deferred { tag: BarrierTag } }

pub struct Meta {
    pub dependence: Option<IslDependence>, // absint 1회 생성, 백엔드 재계산 금지
    pub affine_domain: Option<IslSet>,
    pub secret_taint: BitSet,              // per-SSA-value (R6/R29)
    pub layout_hint: LayoutHint,
    pub call_targets: CallTargets,
    pub lifetimes: Lifetimes,
    pub profile_weight: Option<Profile>,
    pub const_inputs: ConstMask,
}
```

## 6.6 jeff-backend (Layer B pass trait)
```rust
pub trait BackendPass {
    fn name(&self) -> &'static str;
    fn run(&self, r: &mut JlirRegion) -> PassResult; // secret_taint 위반 변환 거부(R6/R29)
}
// 순서: comptime → devirt → AoS→SoA → lifetime→free → tiling → vectorize → branch-fold → (LLVM finish)
```

---
---

# PART 7 — TYPE SYSTEM IMPLEMENTATION SPEC (typing rules)

> language-spec §3 권위. 여기서는 *구현용 규칙*을 명시.

## 7.1 기저 타입
`i8..i64, u8..u64` (machine), `int, nat, rat` (arbitrary precision; collapse 경로 기본, R33), `mod[q]` (q: type-level nat; NTT/PQC), `bv[n]` (GF(2)), `f32, f64` (근사 허용 구간), `bool, char, str`.

## 7.2 sized & indexed
`Vec[T,n]`, `Fin[n]`, `Mat[T,m,n]`. size 인덱스 n은 type-level nat. 두 역할: (1) Total mode termination 추적, (2) affine domain 인식(Barvinok).

## 7.3 ownership modality (선택 typing rules)
```
Γ ⊢ e : own T        (e는 선형 자원)
─────────────────────────────────  [move]
Γ \ e ⊢ move(e) : own T            (이후 e 사용 금지)

Γ ⊢ x : own T
──────────────────  [borrow-shared]
Γ ⊢ &x : &T          (불변, x는 빌림 중 move 불가)

Γ ⊢ x : own T
──────────────────  [borrow-mut]
Γ ⊢ &mut x : &mut T  (배타; 다른 빌림과 공존 불가)
```
linear(정확히 1회 소비, 자원) vs affine(최대 1회). 수명 종료점은 ownership analysis가 정적 계산 → `free` 주입(Layer B #4, GC 없음).

## 7.4 secret-taint (R6 핵심)
```
Γ ⊢ e : secret[T]
─────────────────────────────  [secret-no-branch]
if e { .. } else { .. }   ✗ 컴파일 에러 (data-dependent branch on secret)

Γ ⊢ i : secret[Fin n],  Γ ⊢ v : Vec[T,n]
──────────────────────────────────────  [secret-no-index]
v[i]   ✗ 컴파일 에러 (data-dependent index on secret)
```
secret은 산술·비밀-동형 연산에만. declassify는 명시적·감사 가능 지점에서만(`declassify(e)` — 사용 최소화, 문서화).

## 7.5 refinement (Z3 discharge)
```
type Prob = { x: f64 | 0.0 <= x && x <= 1.0 }
fn idx(v: Vec[T,n], i: { k: nat | k < n }) -> T   // 경계검사 타입에 박힘 → 런타임 검사 제거
```
predicate는 SMT(LIA/NRA/bitvector)로 discharge. 증명되면 런타임 검사 제거(Layer B branch-fold와 동일 게이트), 못 풀면 런타임 검사 fallback(tagged, R1).

## 7.6 effect rows
종류: `IO, Div, Alloc, Rand, Unsafe`. `total ≡ (Div ∉ row) ∧ (IO ∉ row) ∧ termination 타이핑 통과`. Total 함수는 순수·종료 → collapse가 "답 존재" 전제 아래 안전.

## 7.7 termination 타이핑 (Total mode, R34)
- structural recursion: recursive call은 scrutinee의 구문적 부분항에만(size-change/foetus).
- sized recursion: size 인덱스 감소(well-founded).
- guarded corecursion: codata corecursive call은 constructor 아래에서만(productivity).
- 위반 → General 강등 또는 에러. `--total`이면 에러.

---
---

# PART 8 — IR STACK

| IR | 형태 | 불변식 | 크레이트 |
|---|---|---|---|
| Surface AST | indentation 구문 | well-formed parse | jeff-syntax |
| Typed Core | System-F + linear + refinement + effect | type 보존, linearity, effect 정합 | jeff-types/jeff-core-ir |
| High dialects | CR/holonomic · GF(2) affine-block · polyhedral(isl) · tensor-net | dialect별 정합 | 각 collapser |
| **JLIR** | SSA, typed, metadata envelope(6.5) | SSA well-formed, metadata 일관성 | jeff-jlir |
| LLVM IR | SSA | LLVM verifier 통과 | jeff-codegen |

규칙: `dependence`/`secret_taint`는 load-bearing(R18). absint가 dependence 1회 생성, 백엔드 재계산 금지. secret_taint 위반 변환 거부(R6/R29). JLIR은 collapsed residual과 deferred loop이 *둘 다* 내려오는 공통 지점 → 백엔드 universal.

---
---

# PART 9 — BUILD ORDER / MILESTONES (Stage 0..8, 각 DoD)

ROI 순. 각 stage는 이전 stage가 DoD 충족해야 진입.

## Stage 0 — 검증 spine + fallback + CI (전제, 최우선)
- **목표:** 어떤 collapse도 안전하게 실험할 토대 + 최소 end-to-end.
- **산출물:** Certificate schema(6.1); Z3 게이트(6.2, timeout+fallback); never-miscompile fallback harness; cargo workspace(5.2); CI(15); 최소 end-to-end(PART 19).
- **DoD:** `triangular`을 parse→collapse(stub)→verify-or-fallback→LLVM codegen→실행. collapse 경로+fallback 경로 둘 다 테스트 green. license-scan/determinism green.

## Stage 1 — Holonomic 확장 (최고 ROI)
- **산출물:** Gosper(indefinite), Zeilberger(definite) telescoper+certificate; certificate를 polynomial identity로 환원해 Z3 discharge; 경계항 검사.
- **DoD:** 이항/hypergeometric/Catalan류 합 → closed form/저차 recurrence + Z3 검증 + fallback. `non-Gosper-summable` 경로 테스트.

## Stage 2 — Recognizer (Layer 0)
- **산출물:** egglog e-graph; ruleset(CR·Faulhaber·floor_sum·residue_split·holonomic closure·Cole–Hopf); asymptotic cost model; extraction; dispatch tag(6.3).
- **DoD:** PART 14 fixture가 올바른 collapse-class로 라우팅; 미매칭은 올바른 barrier tag.

## Stage 3 — Kernel pack + absint substrate
- **산출물:** eigen/Jordan/matrix-exp, FFT/NTT, Walsh–Hadamard; absint(intervals/congruences/polyhedra); (B)커널: randomized_svd, krylov{lanczos,gmres,arnoldi}, fmm, anderson/shanks/padé, sherman_morrison, koopman_dmd, riccati_schur, rmt_denoise.
- **DoD:** 각 커널 구조 전제부 검사 + 강등 경로(R19) + 테스트(정확/오차/강등). 선형 recurrence가 eigendecomp/Bostan–Mori로 collapse.

## Stage 4 — Barvinok (Layer 3b)
- **산출물:** isl(--with-int=imath) 프런트엔드; clean-room cone decomposition; chamber 검증.
- **DoD:** affine nest count → closed quasi-poly + sample-point 검증. `non-affine-domain` 경로. **license-scan: GPL barvinok 미링크 확인.**

## Stage 5 — GF(2) folder + PQC NTT
- **산출물:** linear/nonlinear partition; (M,b) 누적; clean-room Four-Russians; GF(2) identity 검증(basis-vector); `nonlinearity` barrier; PQC NTT 통합(secret-taint).
- **DoD:** 선형 비트회로 → 단일 (M,b) + GF(2) 검증 + fallback. AES류 S-box 경계 `nonlinearity` defer. PQC poly_mul NTT collapse + const-time-audit green.

## Stage 6 — Layer B 백엔드
- **순서:** B-cf(Z3 재사용) → B-loop(isl 재사용) → layout/devirt/lifetime → comptime → LLVM finish → PGO.
- **DoD:** deferred Θ(N) 루프 SIMD/tiling 상수배 + dependence-legality 검증 + secret-taint 거부 테스트. collapsed residual 백엔드-inert 확인.

## Stage 7 — Tensor-network (Layer 4)
- **산출물:** treewidth 추정(min-degree/min-fill); contraction ordering 휴리스틱; slicing fallback; `treewidth-blowup` barrier.
- **DoD:** bounded-treewidth 회로 collapse + budget 초과 시 defer.

## Stage 8 — Holographic/Pfaffian (Layer 3) + Gröbner
- **산출물:** planarity 검사; matchgrid; FKT/Pfaffian; Holant 검증; `non-planar` barrier. Gröbner budget-gated, `groebner-blowup`.
- **DoD:** planar #CSP collapse + non-planar defer. Gröbner budget 안에서만.

---
---

# PART 10 — PER-LAYER IMPLEMENTATION GUIDES

각 layer: 무엇 / 알고리즘(pseudocode) / 자료구조 / certificate / barrier / dispatch / 테스트.

## 10.1 Layer 0 — Recognizer
- **무엇:** work를 붕괴하지 않고 *recognition* — canonical form으로 몰고 cost 추출, dispatch tag.
- **자료구조:** e-graph(e-class, e-node, union-find, hashcons) via egglog.
- **알고리즘:**
```
fn recognize(ir):
    egraph = build(ir)
    saturate(egraph, RULESET, budget)        # node/time budget
    best   = extract(egraph, cost_model)
    tag    = classify(best)                  # DispatchTag
    return Recognized{canonical=best, tag, cost}
```
- **RULESET:** CR(BWZ/van Engelen), Faulhaber, floor_sum 상호성, residue_split, holonomic closure(sum/product/shift), Cole–Hopf(nonlinear→linear).
- **cost_model:** 점근 지배 — Const ≪ Log ≪ Sublinear ≪ Linear ≪ Superlinear. PGO profile_weight 파라미터화.
- **barrier:** 매칭 없으면 dispatch가 적절 tag.
- **테스트:** PART 14 fixture → 올바른 tag. saturation budget 초과 graceful.

## 10.2 Layer 1 — Arithmetic folder
**CR 코어 (기존 재사용):** Faulhaber, floor_sum, periodic quasi-poly, Sturm rational floor/mod, residue split, Brent cycle, matrix-power-mod, Bostan–Mori. Z3 induction gate 재사용.

**Holonomic:**
```
# Gosper (indefinite Σ t_k):
r(k) = t_{k+1}/t_k                         # rational?
(a,b,c) = gosper_petkovsek_normal_form(r)  # a/b·c(k+1)/c(k)
solve  a(k)·x(k+1) − b(k−1)·x(k) = c(k)    # polynomial x, degree-bounded
if no polynomial solution: DEFER non-Gosper-summable
else: S(k) = (b(k−1)/c(k))·x(k)·t_k        # closed antidifference
      certificate = PolynomialIdentity( S(k+1)−S(k) − t_k == 0 )

# Zeilberger (definite Σ_k F(n,k)):
ansatz L = Σ_{i=0..d} ℓ_i(n)·S_n^i
run parametrized Gosper on (L·F) → telescoper L + certificate G(n,k)
  with (S_k − 1)[G·F] == (L·F)
certificate = Telescoper{ l: L, r: G }     # poly identity (Z3) 또는 operator induction (Lean)
```
**kernel pack** (각: precondition / closed form / certificate / defer):
- eigen·Jordan·matrix-exp — 선형 상태전이 Aⁿ=PDⁿP⁻¹ — cert=EigenCharpoly(charpoly annihilation) — defer: 비선형 점화.
- Bostan–Mori — 선형 recurrence N-th term O(M(d)logN) — cert=NumericResidual(modular).
- FFT/NTT/Toeplitz/circulant — convolution — NTT exact mod q — cert=NumericResidual(exact).
- randomized_svd — *low-rank* — cert=NumericResidual(residual ε bound) — defer/강등 if not low-rank(R19).
- krylov/lanczos/gmres — *sparse* — cert=residual.
- fmm — *감쇠 커널 N-body* O(N²)→O(N) — cert=ε bound.
- cole_hopf — Burgers→heat 정확 선형화(recognizer rule 협동) — cert=변환 항등식.
- koopman_dmd — *linearizable dynamics* — 선형 recurrence collapser로 공급.
- riccati_schur — LQR — closed via Hamiltonian Schur — cert=Riccati residual.
- rmt_denoise — random-matrix 스펙트럼(Wigner/Marchenko–Pastur) — 노이즈 고유값 마스킹.
- **barrier:** `non-Gosper-summable`, `non-affine-domain`(→3b), `groebner-blowup`, `constant-factor-only`(강등).

## 10.3 Layer 3b — Barvinok/Ehrhart
- **무엇:** affine nest trip-count·affine domain 다항 합 → closed (quasi-)poly, 고정차원 다항시간.
- **알고리즘:** isl parametric polytope → Brion/signed unimodular cone decomposition → short rational generating functions → (quasi-)polynomial.
- **certificate:** chamber decomposition(Presburger) + sample 파라미터에서 직접 enumeration 일치(유한) + chamber가 파라미터 공간 분할함을 Z3.
- **licensing:** clean-room on MIT-isl/imath. GPL barvinok/LattE oracle만(R5).
- **barrier:** `non-affine-domain`; 가변차원 counting `sharp-P-hard`.

## 10.4 Layer 2 — GF(2) folder
```
partition(circuit):
    walk DAG; grow affine region over {XOR,NOT,copy,const}; cut at 2-input AND/OR/MUX
accumulate(region):
    M = Π elementary matrices (per affine gate); b from NOT/const
collapse: y = M·x ⊕ b
accel: clean-room Four-Russians (Gray-code table, ~n−1 additions/subspace)
certificate: evaluate on basis {0,e1..en} (n+1 points) → Gf2LinearIdentity → Z3 over GF(2)
barrier: nonlinearity (S-box는 의도적으로 affine을 깨 linearization 방어 → 선형 diffusion만 collapse, S-box defer; 보안적으로 옳음)
```

## 10.5 Layer 3 — Holographic/Pfaffian
- **무엇:** *planar* #CSP — 지수 합 → Pfaffian.
- **알고리즘:** matchgate → matchgrid → FKT(Pfaffian orientation skew-symmetric 행렬의 Pfaffian) → Holant 정리(합=count).
- **precondition:** planarity(linear-time). 실패 → `non-planar`(그러면 #P-hard).
- **경계(정리):** Cai–Lu trichotomy — matchgate는 *planar 다항, 일반 #P-hard*인 #CSP를 정확히 포착. 그 이상 주장 금지.
- **certificate:** PfaffianHolant(Pfaffian 계산 + matchgate signature 항등식 + Holant 등식) → algebraic replay.

## 10.6 Layer 4 — Tensor-network
- **경계(정리):** Markov–Shi — contraction = line graph treewidth; 중간 tensor exp(treewidth); 최적 ordering NP-complete.
- **알고리즘:** treewidth 추정(min-degree/min-fill, budget 내 exact) → ordering 휴리스틱 → budget 초과 `treewidth-blowup`. slicing fallback(인덱스 고정으로 1 infeasible → 다수 feasible 병렬 sub-contraction; 시간↔메모리).
- **certificate:** NumericResidual(정확 환 modular/int replay 또는 random projection).

## 10.7 Layer B — Backend
backend 문서 권위. 요약: collapse=점근, 백엔드=상수배; payoff는 *deferred 경로* 집중(collapsed residual inert). 10 기법:
- B-pre: comptime PE. B-mid: devirt, AoS→SoA(vectorize 전), lifetime→free(linear types).
- B-loop(isl 재사용): cache tiling(dependence-legal), auto-vectorize/SIMD(vector dim dependence-free; secret-tainted addressing 금지).
- B-cf(Z3 재사용): decidable branch만 fold(Rice 비결정).
- LLVM finish: isel, regalloc, scheduling. PGO: cost model 파라미터화(stage 아님).
- 정확성: transform-level(PE/devirt/SoA/lifetime) + per-instance validation(tiling/vectorize: isl legality; branch fold: SMT) + delegated(LLVM).
- secret-taint guard(R6/R29): secret 영역엔 data-oblivious 변환만.

---
---

# PART 11 — CERTIFICATE SCHEMA & VERIFICATION (worked examples)

schema는 6.1. 파이프라인: emit → evidence를 checker로 라우팅(Z3 기본; Lean=holonomic operator induction; Replay=numeric/Pfaffian) → `Valid`→치환; `Invalid|Unknown`→원본+barrier 로그(R31). `jeffc --emit-certificates`로 산출.

## 예시 1 — Faulhaber (PolynomialIdentity → Z3)
```
source:     sum i in 0..=n: i*i
collapsed:  n*(n+1)*(2*n+1)/6
obligation: ∀ n∈ℕ. 6*Σ_{i=0}^{n} i² == n*(n+1)*(2n+1)
evidence:   PolynomialIdentity( 6*S(n) − n*(n+1)*(2n+1) ≡ 0,  with S(n)−S(n−1)=n², S(0)=0 )
check:      Z3 induction (base + step), QF_NRA over ℤ
```

## 예시 2 — Central binomial (Telescoper → Z3/Lean)
```
source:     sum k in 0..=n: C(n,k)^2
collapsed:  C(2*n, n)
evidence:   Telescoper{ l: (n+1)·S_n − (4n+2),  r: G(n,k) }   # Zeilberger
check:      Z3 polynomial identity (S_k−1)[G·F] == (L·F); 필요시 Lean operator induction
```

## 예시 3 — 선형 비트 순열 (Gf2LinearIdentity → Z3/GF(2))
```
source:     bit-mix chain over bv[64]
collapsed:  x ↦ M·x ⊕ b   (M: 64×64 over GF(2), b: 64)
evidence:   Gf2LinearIdentity{ m, b }  (basis {0,e1..e64} 평가로 결정)
check:      Z3 over GF(2): ∀ x. circuit(x) == M·x ⊕ b
```

## 예시 4 — affine nest count (Barvinok)
```
source:     count (i,j) in {0<=i<=j<=n}: 1
collapsed:  (n+1)*(n+2)/2
evidence:   chamber decomp (single chamber n≥0) + sample n∈{0,1,2,3} 일치
check:      Z3 Presburger (chamber 분할) + 유한 sample replay
```

---
---

# PART 12 — HONEST_DEFER TAXONOMY (detection sketch + diagnostic)

각 barrier에서 정확한 tag와 사용자 진단(R4/R27). detection은 sketch — 정확 구현은 해당 layer.

| tag | 근거 | detection sketch | diagnostic 문자열 |
|---|---|---|---|
| `data-dependent-omega-N` | 정보이론 Ω(N) | absint: iteration k 출력이 무계 prior 입력에 의존(데이터 흐름) | "data-dependent across the whole input; Ω(N) information floor — backend gives constant-factor only" |
| `sharp-P-hard` | Valiant'79; Toda | counting이 Cai–Lu tractable class 밖(permanent류) | "#P-hard counting; no polynomial closed form exists" |
| `np-hard` | Lucas'14 | 목적이 QUBO/Ising ground state로 환원 | "NP-hard optimization (Ising ground state); refused" |
| `termination` | halting | General 무계 루프, 미인식 | "termination is undecidable here; use Total mode or provide a bound" |
| `rice-undecidable` | Rice | General 코드의 비자명 semantic property 요구 | "non-trivial semantic property is undecidable; sound over-approximation only" |
| `treewidth-blowup` | Markov–Shi | est. treewidth > budget | "tensor-network treewidth exceeds budget (B={budget})" |
| `nonlinearity` | S-box 설계 | affine 블록 내 2-input AND/OR/MUX | "nonlinear gate reached; GF(2) linear collapse stops here (this is correct for S-boxes)" |
| `non-planar` | Cai–Lu | planarity 검사 실패 | "non-planar #CSP; #P-hard outside the matchgate class" |
| `non-affine-domain` | Barvinok | 비affine 경계/첨자 | "loop domain is not affine; Barvinok inapplicable" |
| `non-Gosper-summable` | Gosper/Zeilberger | bounded telescoper 없음 | "sum is not holonomic-summable (no Gosper/Zeilberger certificate)" |
| `groebner-blowup` | Mayr–Meyer | Gröbner degree/time > budget | "Gröbner basis exceeds budget (EXPSPACE worst case)" |
| `constant-factor-only` | Ω(N), 점근 붕괴 불가 | linear/transformable이나 비가역 Θ(N) | "linear structure but no asymptotic collapse; backend constant-factor only" |
| `memory-hard` | Argon2 | memory-hard KDF/PoW 인식 | "memory-hard by design (e.g., Argon2); not collapsible — that is the point" |
| `physics-counterfactual` | 보존법칙+물리 | CTC/nonlinear-QM-on-classical/wormhole류 | "physics-counterfactual construction; rejected (see PART 18)" |

detection 예 (sketch):
```rust
fn detect_data_dependent(region) -> bool {
    // 각 iteration의 출력이 직전까지의 무계 누적 상태에 의존하면 true
    region.loop_carried_deps().any(|d| d.is_unbounded_accumulation())
}
fn detect_nonlinearity(circuit) -> Option<GateRef> {
    circuit.gates().find(|g| g.is_nonlinear_over_gf2()) // 2-input AND/OR/MUX
}
```

---
---

# PART 13 — DIAGNOSTIC MESSAGE CATALOG (DX, P5)

모든 진단은 (1) 무슨 일, (2) 왜, (3) 사용자가 할 수 있는 것을 담는다. 예:

- **collapse 성공 (--collapse-report):**
  `fn s2: collapsed by arith/holonomic to closed form — O(1); certificate verified (Z3).`
- **defer:**
  `fn checksum: HONEST_DEFER[data-dependent-omega-N] — each step depends on all prior input; backend applied SIMD (constant-factor). To collapse, the recurrence must be linear/affine.`
- **@collapse 실패 (요구했으나 불가):**
  `error: #[collapse] required, but this loop hit HONEST_DEFER[non-affine-domain]. Domain {i*i <= n} is not affine. Remove #[collapse] or restructure to an affine domain.`
- **secret-taint 위반:**
  `error: data-dependent branch on secret value 'k' (type secret[i32]). Constant-time required (#[constant_time]). Rewrite to be data-oblivious (e.g., constant-time select).`
- **Total 종료 실패:**
  `error: total fn 'loop2': recursive call argument does not structurally decrease; termination not provable. Move to General mode or use a sized/structural recursion.`
- **license 위반(CI):**
  `error[license]: crate 'X' is GPL-3.0; linking forbidden (PART 3 R5). Use a clean-room reimplementation or an out-of-process oracle in jeff-test-oracles.`

규칙: secret 값 자체는 진단에 출력하지 않는다(R36) — 이름/타입만.

---
---

# PART 14 — TESTING STRATEGY & FIXTURE CATALOG

## 14.1 테스트 종류
- **Unit:** 모든 함수/변환.
- **Property(proptest):** collapse는 원본과 동치(랜덤 입력 같은 출력); type checker soundness; GF(2)는 basis 전체 동치.
- **Certificate-replay:** 모든 emit cert 독립 재검증(R25).
- **Differential:** out-of-process oracle(Sage/Mathematica/GPL barvinok, dev-only)와 대조. *링크 금지, 프로세스 호출*(R5).
- **Fallback:** 각 collapser에 검증-실패 강제 → 원본 fallback 확인(R1/R3).
- **Const-time:** PQC 경로 timing-class 동치(R28).
- **Fuzz(cargo-fuzz):** parser/typechecker/IR.
- **Golden:** collapse 입력→출력→cert 고정(R25).
- **44/44 확장:** 기존 CR 게이트 체크 집합 회귀 스위트 유지·확대.

## 14.2 Fixture 카탈로그 (positive=collapse, negative=defer+correct-tag)
Layer 1 (holonomic/arith):
```
+ sum i in 0..=n: i           -> n*(n+1)/2            [PolynomialIdentity]
+ sum i in 0..=n: i*i         -> n*(n+1)*(2n+1)/6     [PolynomialIdentity]
+ sum i in 0..n:  2**i        -> 2**n - 1             [LinearStateTransition/eigen]
+ sum k in 0..=n: C(n,k)      -> 2**n                 [Telescoper]
+ sum k in 0..=n: C(n,k)^2    -> C(2n,n)              [Telescoper]
+ fib(n) via matrix power     -> Bostan-Mori/eigen    [EigenCharpoly]
- sum i in 0..=n: f(a[i])     -> data-dependent-omega-N (a unknown)
- sum k: 1/(k^2+1) (rational, not hypergeometric-summable) -> non-Gosper-summable
```
Layer 3b (Barvinok):
```
+ count (i,j) in {0<=i<=j<=n}: 1 -> (n+1)(n+2)/2
+ count i in {0<=i<n, i%2==0}: 1 -> quasi-poly (period 2)
- count (i,j) in {i*i+j*j<=n}: 1 -> non-affine-domain
```
Layer 2 (GF(2)):
```
+ linear XOR/rotate mix over bv[64] -> M·x⊕b           [Gf2LinearIdentity]
- AES round (with S-box)            -> nonlinearity (linear layer collapses, S-box defers)
```
Layer 3/4:
```
+ planar #CSP instance      -> Pfaffian collapse        [PfaffianHolant]
- non-planar #CSP           -> non-planar
+ bounded-treewidth tensor net -> contraction collapse
- dense (high-treewidth) net -> treewidth-blowup
```
PQC:
```
+ poly_mul over mod[3329] (secret a) -> NTT collapse; const-time-audit green
- branch on secret coefficient       -> compile error (secret-no-branch)
```
거부(PART 18):
```
- argmin_ising(encode(cnf))     -> np-hard (refused)
- "geometric O(1)" 요청          -> physics-counterfactual / 거부
```

## 14.3 per-layer 테스트 요구
새 collapser: positive(collapse+cert 검증) + negative(defer+correct tag) + fallback(검증 강제 실패) + golden + property(동치). 빠지면 merge 불가(R3).

---
---

# PART 15 — CI GATES (scripts; 전부 green이어야 merge)

```bash
# ci/run.sh
set -euo pipefail
cargo build --workspace
cargo clippy --workspace -- -D warnings
cargo test  --workspace
cargo test  --workspace --features proptest        # property
./ci/cert_replay.sh                                # 모든 certificate 독립 재검증
./ci/license_scan.sh                               # GPL/LGPL 링크 0
./ci/const_time_audit.sh                           # secret[T] 경로 data-oblivious
./ci/determinism.sh                                # 2회 빌드 산출물 동일(cert 포함)
./ci/bench_honesty.sh                              # 벤치 수치 하드코딩 0
./ci/coverage.sh --min-line 0.85 --min-branch 0.75 # core 커버리지 하한
```
```bash
# ci/license_scan.sh (핵심)
# cargo-deny 또는 cargo-license로 의존성 라이선스 수집.
# GPL-*, LGPL-* 가 release 의존성 그래프에 있으면 fail.
# barvinok/LattE/PPL/GMP/M4RI 이름이 [dependencies]에 있으면 fail (dev-deps in jeff-test-oracles만 허용).
```
```bash
# ci/determinism.sh
jeffc build tests/e2e/triangular.jeff --emit-certificates out1
jeffc build tests/e2e/triangular.jeff --emit-certificates out2
diff -r out1 out2   # 동일해야 함
```

---
---

# PART 16 — CLAUDE CODE WORKING STYLE

- **작은 검증된 증분.** 한 번에 한 컴포넌트. 모든 PR green(PART 15). 큰 미검토 dump 금지(R13).
- **checker-before-collapser**(R2). 검증 못 하면 collapse 아님.
- **막히면 정직하게**(2.4 형식). 가짜로 통과 금지(DR5, §2.3).
- **정확성·명료성 > 영리함.** 영리한 한 줄보다 검증 가능한 열 줄.
- **할 수 없으면 HONEST_DEFER.** budget 내 soundly 못 하면 가짜 대신 defer+tag.
- **spec 인용**(R22). detail 재발명 금지.
- **코드와 함께 테스트·문서 갱신.**
- **패딩 금지**(R30). 줄 수를 위해 채우지 마라.
- **불확실하면 멈추고 물어라.** 추측으로 P0/P1 위반 금지.
- **TODO는 추적 가능하게.** `// TODO(stage-N): ...` + 이슈. 조용한 미구현 금지(R24).

---
---

# PART 17 — GLOBAL DEFINITION OF DONE

기능은 다음을 *모두* 충족할 때만 done:
1. 구현 + 타입 통과(linear/affine/refinement/effect/secret-taint).
2. unit + (해당 시) property 테스트 green.
3. collapse면 certificate-verification 테스트 green(R3).
4. fallback 테스트 green(R1).
5. licensing-clean(R5).
6. secret 경로면 const-time-clean(R6/R28).
7. 문서화(R12) + spec 인용(R22).
8. 정직한 벤치마크(R8) — 해당 시.
9. golden 고정(R25), 결정성(R11).
10. CI 전부 green(PART 15).
빠졌으면 done 아님(R40).

---
---

# PART 18 — ANTI-PATTERNS / NEVER DO

언어 차원의 non-goal(language-spec §16). 코드·설계에 나타나면 reject(R9). 채택과 거부의 경계는 *언제나* "이미 존재하는 구조를 활용하는가" vs "구조 없이 불가능한 것을 주장하는가"다.

1. **임의 계산의 "기하학적 O(1)".** cohomology/manifold/holography/mirror-symmetry로 임의 work를 O(1)로. Ω(N) 위반. (단 *구조 활용* 변환 — Cole–Hopf, Koopman, randomized SVD — 은 채택. 차이가 핵심.)
2. **Rice/halting 우회.** "undecidable 영역 특이점 격리." 불변. 정직한 답 = Total mode + sound absint + defer.
3. **에너지 최소화=정답(Ising/QUBO 마법).** ground state NP-hard. 기하화는 hardness 재배치일 뿐. → `np-hard`.
4. **BBBV 우회 / nonlinear-QM-on-classical.** 구조 활용≠BBBV 깨기(범주오류); 고전 NLSE 에뮬은 매 스텝 full work; Abrams–Lloyd는 물리적 비선형 QM 필요(없음). "정답을 potential에 심고 찾기"는 순환.
5. **CTC/wormhole/AdS-CFT 하드웨어.** right-math-wrong-universe; 현 하드웨어 불가. JEFF는 이 우주의 실제 하드웨어, 선형 QM, 순방향 인과율에서 돈다.
6. **가짜 certificate / 검사 우회 / oracle 우회 / 정답 하드코딩.** P0/P1 직접 위반.
7. **GPL/LGPL 링크.** R5.
8. **조용한 근사 / silent skip / half-collapse.** R4/R32.
9. **조작 벤치마크 / 균일 가속 주장 / "지수적"·"O(1)" 무조건 사용.** R8/R30.
10. **줄 수 패딩.** 코드·문서·프롬프트 어디서든.
11. **secret 값 누설**(로그·진단·디버그). R36.
12. **panic으로 사용자 에러 처리.** R38.

---
---

# PART 19 — KICKOFF TASK (Stage 0, with code stubs)

## 19.1 목표
어떤 collapse도 안전하게 실험할 토대 + 최소 end-to-end. (P0/P2 spine)

## 19.2 작업
1. **워크스페이스 scaffold**(5.2): 모든 크레이트 stub + workspace Cargo.toml + docs/에 3 spec 배치.
2. **Certificate schema**(6.1) 구현(jeff-cert): struct + Evidence enum + serde. `Collapsed`는 `VerifiedCertificate` 없이 생성 불가(type-level).
3. **Z3 게이트**(6.2, jeff-verify): PolynomialIdentity/Gf2LinearIdentity discharge; timeout+fallback(R23/R31). Lean stub.
4. **Fallback harness**(jeff-jlir/jeff-codegen): verify 실패/unknown/timeout → 원본 IR codegen. hang/miscompile 금지.
5. **CI**(ci/, PART 15): build, clippy(-D warnings), test, license-scan, determinism. 전부 게이트.
6. **최소 end-to-end:** 아래 stub 흐름.

## 19.3 end-to-end 흐름 (stub)
```rust
// jeffc: triangular(n) = sum i in 0..=n: i
fn compile(src: &str) -> CompileResult {
    let ast    = jeff_syntax::parse(src)?;                 // R20: 진단 수집
    let core   = jeff_types::check(ast)?;                  // 타입/effect
    let rec    = jeff_recognizer::recognize(&core, BUDGET);// stub: AffineTripCount tag
    let outcome = jeff_collapse_arith::FaulhaberLike.try_collapse(&core); // stub
    let region = match outcome {
        CollapseOutcome::Collapsed(c) => jlir::from_collapsed(c), // cert 검증됨
        CollapseOutcome::Defer(d)     => jlir::from_loop(d.original, d.tag),
    };
    let llvm = jeff_codegen::lower(region)?;               // fallback 포함
    Ok(llvm)
}
```
```rust
// jeff-collapse-arith (stub): closed-form 후보 + certificate
impl Collapser for FaulhaberLike {
    const ID: CollapserId = "arith/faulhaber-like";
    fn entry(&self, r: &Recognized) -> bool { matches!(r.tag, DispatchTag::AffineTripCount) }
    fn try_collapse(&self, ir: &CoreIr) -> CollapseOutcome {
        let candidate = closed_form_for_power_sum(ir); // n*(n+1)/2 등
        let cert = Certificate { /* PolynomialIdentity ... */ };
        match jeff_verify::verify(cert) {              // 실제 Z3 (가짜 금지, DR1)
            Some(vc) => CollapseOutcome::Collapsed(Collapsed::new(candidate, vc)),
            None     => CollapseOutcome::Defer(Defer::new(
                            BarrierTag::ConstantFactorOnly, ir.as_ref())),
        }
    }
}
```

## 19.4 Acceptance criteria
- `cargo build/test/clippy` green; `ci/` 게이트 green(license-scan 포함).
- end-to-end가 collapse 경로와 fallback 경로 *둘 다* 테스트로 커버.
- Certificate가 실제로 Z3로 검증됨(가짜 아님). 검증 강제 실패 시 fallback 동작.
- determinism: 2회 빌드 산출물 동일.
- 모든 unimplemented는 `unimplemented!("reason")`/`HONEST_DEFER` 표시(R24).
- 출력 형식은 2.4 따름(DONE/VERIFIED/DEFERRED/NEXT).

---
---

# PART 20 — STANDING PROMPTS (재사용 체크리스트)

## 20.1 "새 collapser 구현"
1. spec(architecture) 섹션 인용. 무엇을 *정확히* 붕괴? 정리적 경계?
2. **먼저** certificate evidence kind와 checker 구현+테스트(R2).
3. dispatch 진입 조건(recognizer tag)과 *defer 조건*(어느 barrier tag) 정의.
4. `Collapser` trait 구현. fallback 필수(R1). half-collapse 금지(R32).
5. 테스트: positive(collapse+cert), negative(defer+tag), fallback(검증 강제 실패), golden(R25), property(동치).
6. 문서 + spec 인용. DoD(PART 17) 통과.

## 20.2 "numeric 커널 추가"
1. *활용 구조* 명시(low-rank/sparse/decaying/integrable) — 타입/계약.
2. 구조 부재 시 강등 경로(R19).
3. precondition 검사 + closed form/가속 + certificate(evidence kind) + 오차 bound(근사면 타입 명시, R33).
4. 테스트: 정확/오차/강등/fallback. DoD.

## 20.3 "fold family 추가"
1. recognizer rule(egglog) 추가.
2. Layer 1 collapser + certificate.
3. fixture(positive/negative) + golden + property.

## 20.4 "certificate evidence kind 추가"
1. Evidence enum 확장 + checker 라우팅(Z3/Lean/Replay).
2. cert-replay 테스트.
3. 모든 fallback 경로 유지.

## 20.5 "barrier 처리"
1. PART 12에 tag 있나? 없으면 근거 정리와 함께 추가(R12).
2. detection 규칙(static/dynamic) 구현.
3. 사용자 진단 emit(R27, PART 13). 조용한 skip 금지(R4).
4. 테스트: 해당 입력이 정확히 그 tag로 defer.

## 20.6 "PQC primitive 추가"
1. 전 구간 `secret[T]` + `@constant_time`(R28).
2. NTT/modular는 Layer 1 exact collapse 경유.
3. const-time-audit 테스트(timing-class 동치). secret 경로 누설 변환 거부(R29). secret 값 미누설(R36).

## 20.7 "백엔드 pass 추가"
1. 정확성 체제 분류(transform-level / per-instance validation / delegated).
2. per-instance면 validation(isl legality 또는 SMT) 구현.
3. secret_taint 위반 거부(R6/R29). dependence 재계산 금지(metadata 사용).
4. collapsed residual에 inert 확인. deferred 경로 측정(R8).

## 20.8 "done이라고 말하기 전에"
PART 17 전부? CI 전부 green? certificate 실제 검증? fallback 테스트? 패딩 없음? spec 인용? secret 누설 없음? — 아니면 done 아님.

---
---

# PART 21 — WORKED END-TO-END WALKTHROUGHS

## 21.1 collapse 성공 경로 (Faulhaber)
```
입력:   total fn s2(n: nat) -> nat: sum i in 0..=n: i*i
parse → core(타입 nat, effect 없음, Total) 
recognize → AffineTripCount, cost Linear(원본), 후보 cost Const
collapse(arith) → 후보 n*(n+1)*(2n+1)/6 + PolynomialIdentity
verify(Z3) → Valid → VerifiedCertificate
jlir → Origin::Collapsed{layer:1}, residual=closed form (백엔드 inert)
codegen → O(1) 산술
report: "collapsed to O(1); certificate verified (Z3)"
```

## 21.2 defer 경로 (data-dependent)
```
입력:   fn checksum(xs: &[u8]) -> u64 { FNV loop }
recognize → None (loop-carried nonlinear dep)
collapse → 모든 collapser entry=false 또는 Defer
detect → data-dependent-omega-N
jlir → Origin::Deferred{tag}, 원본 루프 보존
backend → SIMD/bit-slice (Θ(N) 유지, 상수배) + dependence-legality
report: "HONEST_DEFER[data-dependent-omega-N]; backend applied SIMD (constant-factor)"
```

## 21.3 거부 경로 (NP-hard)
```
입력:   @collapse fn solve(phi: CNF) -> Assignment: argmin_ising(encode(phi))
recognize → 목적이 Ising ground state로 환원
detect → np-hard
@collapse 요구이므로 → compile error: "HONEST_DEFER[np-hard]: Ising ground state is NP-hard; refused"
```

## 21.4 보안 경로 (secret-taint)
```
입력:   @constant_time fn dec(sk: secret[...], ct: ...) { if sk[0] == 0 {..} }
typecheck → secret-no-branch 위반
compile error (PART 13): "data-dependent branch on secret 'sk'; constant-time required"
```

---
---

# PART 22 — GLOSSARY / QUICK REFERENCE

- **collapse:** 구조적 work를 closed/sublinear form으로 점근 환원(certificate 보증).
- **HONEST_DEFER:** collapse 불가를 named tag로 정직히 표시하고 원본 유지.
- **certificate:** collapse 동치의 기계 검증 가능 증명 객체(PART 11). VerifiedCertificate 없이 Collapsed 생성 불가(type-level).
- **conservation law:** 표현은 공짜, 구조는 아님. 세 벽: Ω(N) data, #P/NP-hard, undecidability.
- **Total mode:** sub-Turing, 종료가 타입. **General mode:** Turing-complete, best-effort.
- **secret-taint:** `secret[T]` — data-dependent timing/branch/index 금지(constant-time guard).
- **JLIR:** collapsed residual과 deferred loop이 둘 다 내려오는 공통 SSA + metadata IR.
- **Layer 0–4:** recognizer / arithmetic+holonomic+kernel / GF(2) / holographic / tensor. **3b:** Barvinok. **B:** backend(상수배).
- **dispatch tag:** Holonomic | LinearStateTransition | Convolution | Gf2Affine | PlanarCsp | BoundedTreewidth | AffineTripCount | None.
- **barrier tag:** PART 12 표.
- **PRIORITY:** P0 never-miscompile > P1 honesty > P2 proof-carrying > P3 conservation > P4 performance > P5 DX.
- **한 줄:** 구조는 certificate와 함께 붕괴, 나머지는 하드웨어 한계로, 불가능은 이름 붙여 거부 — 결코 틀린 답 없이.

---
---

# APPENDIX A — COMPLETE SURFACE GRAMMAR (EBNF) — jeff-syntax

> 이 문법은 jeff-syntax(lexer+parser)의 권위 명세다. parser는 이 문법을 정확히 구현하고, source span을 모든 노드에 보존한다(R37). 파서는 첫 에러에서 멈추지 않고 진단을 모은다(R20). 문법이 language-spec §2와 충돌하면 spec이 이긴다(0.2).

## A.1 표기법
```
"x"      terminal
x        nonterminal
x?       optional (0 or 1)
x*       0 or more
x+       1 or more
x | y    alternative
( ... )  grouping
INDENT / DEDENT / NEWLINE = lexer가 내는 layout 토큰
```

## A.2 Lexer notes
- **layout:** Python식 indentation. lexer가 INDENT/DEDENT/NEWLINE을 합성. 탭 금지(스페이스만), 일관된 들여쓰기 폭.
- **comment:** `# ...` 줄 끝까지.
- **identifier:** `ident ::= (letter | "_") (letter | digit | "_")*` (유니코드 letter 허용 — 식별자에 한해; 키워드 제외).
- **keywords (예약):** `module import total fn data codata type const let var for while return match in and or not own move secret true false IO Div Alloc Rand Unsafe sum prod count fold`.
- **숫자/문자열 리터럴:** A.10.

## A.3 Top level
```
program        ::= module_decl? import_decl* item*
module_decl    ::= "module" dotted_name NEWLINE
import_decl    ::= "import" dotted_name ( "(" ident ("," ident)* ")" )? NEWLINE
dotted_name    ::= ident ("." ident)*
item           ::= fn_decl | data_decl | codata_decl | type_alias | const_decl
```

## A.4 Functions
```
fn_decl        ::= attribute* mode? "fn" ident generics? "(" params? ")" ret? effect? ":" block
mode           ::= "total"                       (* 없으면 General *)
attribute      ::= "@" ident ( "(" attr_args? ")" )?
attr_args      ::= attr_arg ("," attr_arg)*
attr_arg       ::= ident ("=" ident)?            (* @collapse(class = holonomic) 등 *)
generics       ::= "[" generic_param ("," generic_param)* "]"
generic_param  ::= ident (":" kind)?
kind           ::= "type" | "nat"                (* type-level nat for sized types *)
params         ::= param ("," param)*
param          ::= ident ":" type
ret            ::= "->" type
effect         ::= "/" "{" (effect_label ("," effect_label)*)? "}"
effect_label   ::= "IO" | "Div" | "Alloc" | "Rand" | "Unsafe"
```
인식되는 attribute: `@collapse`, `@collapse(allow_defer)`, `@collapse(class = <id>)`, `@constant_time`. 미인식 attribute는 진단(경고).

## A.5 Data / codata / alias / const
```
data_decl      ::= "data" ident generics? ":" NEWLINE INDENT constructor+ DEDENT
constructor    ::= ident ( "(" type ("," type)* ")" )? NEWLINE
codata_decl    ::= "codata" ident generics? ":" NEWLINE INDENT destructor+ DEDENT
destructor     ::= ident ":" type NEWLINE
type_alias     ::= "type" ident generics? "=" type NEWLINE
const_decl     ::= "const" ident ":" type "=" expr NEWLINE
```

## A.6 Types
```
type           ::= ref_type
ref_type       ::= "&" "mut"? type
                 | "own" type
                 | "secret" "[" type "]"
                 | refinement_type
refinement_type::= "{" ident ":" base_type "|" expr "}"      (* Z3로 discharge, §7.5 *)
                 | app_type
app_type       ::= ctor_type ( "[" type_arg ("," type_arg)* "]" )?
type_arg       ::= type | expr                                (* expr = type-level nat: Vec[T, n] *)
ctor_type      ::= base_type | ident
base_type      ::= "i8"|"i16"|"i32"|"i64"|"u8"|"u16"|"u32"|"u64"
                 | "int"|"nat"|"rat"|"bool"|"char"|"str"|"f32"|"f64"
                 | "mod" "[" expr "]"                          (* mod[q] *)
                 | "bv"  "[" expr "]"                          (* bv[n] *)
                 | "Vec" | "Fin" | "Mat"
```
주의: collapse 경로 기본 산술 타입은 `int/nat/rat/mod`(R33). `secret[T]`는 §7.4 typing rule의 대상.

## A.7 Blocks / statements
```
block          ::= NEWLINE INDENT stmt+ DEDENT
                 | expr NEWLINE                                (* single-expression body *)
stmt           ::= let_stmt | var_stmt | assign_stmt | for_stmt
                 | while_stmt | return_stmt | expr_stmt
let_stmt       ::= "let" pattern (":" type)? "=" expr NEWLINE
var_stmt       ::= "var" ident ":" type "=" expr NEWLINE
assign_stmt    ::= lvalue assign_op expr NEWLINE
assign_op      ::= "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "^=" | "&=" | "|="
lvalue         ::= ident | lvalue "[" expr "]" | lvalue "." ident
for_stmt       ::= "for" pattern "in" expr ":" block
while_stmt     ::= "while" expr ":" block                      (* General mode only; Total 금지 §7.7 *)
return_stmt    ::= "return" expr? NEWLINE
expr_stmt      ::= expr NEWLINE
```

## A.8 Expressions (precedence; 낮은 → 높은)
```
expr           ::= or_expr
or_expr        ::= and_expr ("or" and_expr)*
and_expr       ::= not_expr ("and" not_expr)*
not_expr       ::= "not" not_expr | cmp_expr
cmp_expr       ::= bitor_expr (cmp_op bitor_expr)*
cmp_op         ::= "==" | "!=" | "<" | "<=" | ">" | ">="
bitor_expr     ::= bitxor_expr ("|" bitxor_expr)*
bitxor_expr    ::= bitand_expr (("^"|"xor") bitand_expr)*
bitand_expr    ::= shift_expr ("&" shift_expr)*
shift_expr     ::= add_expr (("<<"|">>") add_expr)*
add_expr       ::= mul_expr (("+"|"-") mul_expr)*
mul_expr       ::= unary_expr (("*"|"/"|"//"|"%") unary_expr)*
unary_expr     ::= ("-"|"~") unary_expr | pow_expr
pow_expr       ::= postfix_expr ("**" unary_expr)?            (* right-assoc *)
postfix_expr   ::= primary ( call_suffix | index_suffix | field_suffix )*
call_suffix    ::= "(" args? ")"
index_suffix   ::= "[" expr "]"
field_suffix   ::= "." ident
args           ::= expr ("," expr)*

primary        ::= literal
                 | ident
                 | "(" expr ")"
                 | reduction_expr
                 | match_expr
                 | borrow_expr
                 | range_expr
borrow_expr    ::= "&" "mut"? expr | "own" expr | "move" "(" expr ")"
range_expr     ::= add_expr ".." "="? add_expr                (* a..b (반열림) | a..=b (닫힘) *)
```

## A.9 Structured reduction (1급 recognizer 입력) & match
```
reduction_expr ::= reduction_kw binder "in" domain ":" expr
reduction_kw   ::= "sum" | "prod" | "count" | "fold"
binder         ::= pattern | "(" pattern ("," pattern)* ")"
domain         ::= range_expr | set_domain | expr
set_domain     ::= "{" constraint ("," constraint)* "}"       (* affine domain → Barvinok(3b) *)
constraint     ::= expr cmp_op expr (cmp_op expr)?            (* 0 <= i <= j <= n *)

match_expr     ::= "match" expr ":" NEWLINE INDENT match_arm+ DEDENT
match_arm      ::= pattern "=>" (expr NEWLINE | block)
pattern        ::= "_" | literal | ident | ctor_pattern | nat_succ_pattern
ctor_pattern   ::= ident ( "(" pattern ("," pattern)* ")" )?
nat_succ_pattern ::= ident "+" int_literal                    (* m+2 : sized 분해, 종료 추적 *)
```
규칙: `sum/prod/count/fold`와 `set_domain`은 Layer 0 recognizer의 직접 진입점이다(PART 10.1). `set_domain`이 affine이면 `AffineTripCount`/Barvinok, 아니면 `non-affine-domain`.

## A.10 Literals
```
literal        ::= int_literal | float_literal | bool_literal | char_literal
                 | string_literal | mod_literal | rat_literal | bv_literal
int_literal    ::= digit+ int_suffix?
int_suffix     ::= "i8"|"i16"|"i32"|"i64"|"u8"|"u16"|"u32"|"u64"|"nat"
float_literal  ::= digit+ "." digit+ ("f32"|"f64")?
bool_literal   ::= "true" | "false"
char_literal   ::= "'" char "'"
string_literal ::= "\"" char* "\""
mod_literal    ::= digit+ "mod" digit+                         (* 5 mod 3329 *)
rat_literal    ::= digit+ "/" digit+                           (* 1/3 *)
bv_literal     ::= "0b" bin_digit+
digit          ::= "0".."9"
bin_digit      ::= "0" | "1"
```

## A.11 파서 산출물 & 에러
- 산출: `ast::Program`(APPENDIX B.1). 모든 노드에 `Span`(R37).
- 에러: `Result<Program, Vec<Diagnostic>>`. panic 금지(R38). 가능한 한 많은 진단 수집(R20). 진단은 PART 13 형식.

---
---

# APPENDIX B — PER-CRATE FULL REFERENCE SKELETONS

> PART 6은 jeff-cert / jeff-verify / jeff-recognizer / jeff-jlir / jeff-backend의 핵심을 다뤘다. 여기서는 *나머지 모든 크레이트*의 공개 API 표면과 핵심 내부 타입을 명시한다. skeleton은 시작점이다(0.2). 모든 미구현은 `unimplemented!("reason")`(R24). 모든 사용자 입력 에러는 `Result`/diagnostic, panic 아님(R38).

## B.1 jeff-syntax
```rust
pub mod lexer {
    pub fn lex(src: &str) -> Result<Vec<Token>, Vec<Diagnostic>>; // INDENT/DEDENT/NEWLINE 합성
    pub struct Token { pub kind: TokKind, pub span: Span }
    pub enum TokKind { Ident(String), Int(String, Option<NumSuffix>), Float(String),
        Str(String), Char(char), Kw(Keyword), Sym(Sym), Indent, Dedent, Newline }
}
pub mod ast {
    pub struct Program { pub module: Option<DottedName>, pub imports: Vec<Import>, pub items: Vec<Item> }
    pub struct Import { pub path: DottedName, pub names: Vec<Ident>, pub span: Span }
    pub enum Item { Fn(FnDecl), Data(DataDecl), Codata(CodataDecl), TypeAlias(TypeAlias), Const(ConstDecl) }

    pub struct FnDecl { pub attrs: Vec<Attr>, pub mode: Mode, pub name: Ident,
        pub generics: Vec<GenericParam>, pub params: Vec<Param>, pub ret: Option<Type>,
        pub effect: EffectRow, pub body: Block, pub span: Span }
    pub enum Mode { Total, General }
    pub struct Attr { pub name: Ident, pub args: Vec<AttrArg>, pub span: Span } // @collapse / @constant_time
    pub struct GenericParam { pub name: Ident, pub kind: Kind } // Kind::{Type, Nat}
    pub struct Param { pub name: Ident, pub ty: Type }

    pub enum Type { Base(BaseType), App(Ident, Vec<TypeArg>),
        Ref { mutable: bool, inner: Box<Type> }, Own(Box<Type>), Secret(Box<Type>),
        Refine { var: Ident, base: BaseType, pred: Box<Expr> } }
    pub enum TypeArg { Ty(Type), Term(Expr) } // Term = type-level nat (Vec[T, n])
    pub enum BaseType { I(IntWidth), U(IntWidth), Int, Nat, Rat, Bool, Char, Str,
        F32, F64, Mod(Box<Expr>), Bv(Box<Expr>), Vec, Fin, Mat }

    pub struct Block { pub stmts: Vec<Stmt>, pub span: Span } // single-expr body는 stmts=[Return-less Expr]
    pub enum Stmt { Let { pat: Pattern, ty: Option<Type>, init: Expr },
        Var { name: Ident, ty: Type, init: Expr },
        Assign { lhs: LValue, op: AssignOp, rhs: Expr },
        For { pat: Pattern, iter: Expr, body: Block },
        While { cond: Expr, body: Block },     // General only
        Return(Option<Expr>), Expr(Expr) }

    pub enum Expr { Lit(Lit), Var(Ident), Paren(Box<Expr>),
        Bin(BinOp, Box<Expr>, Box<Expr>), Un(UnOp, Box<Expr>),
        Call(Box<Expr>, Vec<Expr>), Index(Box<Expr>, Box<Expr>), Field(Box<Expr>, Ident),
        Range { lo: Box<Expr>, hi: Box<Expr>, inclusive: bool },
        Reduction { kind: RedKind, binder: Vec<Pattern>, domain: Domain, body: Box<Expr> },
        Match { scrut: Box<Expr>, arms: Vec<Arm> },
        Borrow { mutable: bool, inner: Box<Expr> }, Own(Box<Expr>), Move(Box<Expr>) }
    pub enum RedKind { Sum, Prod, Count, Fold }
    pub enum Domain { Range(Box<Expr>), Set(Vec<Constraint>), Expr(Box<Expr>) }
    pub struct Constraint { pub parts: Vec<(CmpOp, Expr)>, pub head: Expr } // 0<=i<=j<=n
    pub struct Arm { pub pat: Pattern, pub body: Block }
    pub enum Pattern { Wild, Lit(Lit), Var(Ident), Ctor(Ident, Vec<Pattern>), NatSucc(Ident, u64) }
    pub enum Lit { Int(String, Option<NumSuffix>), Float(String), Bool(bool), Char(char),
        Str(String), Mod(String, String), Rat(String, String), Bv(String) }
    // BinOp/UnOp/AssignOp/CmpOp/IntWidth/Kind/DottedName/Ident 생략 (자명)
}
pub fn parse(src: &str) -> Result<ast::Program, Vec<Diagnostic>>; // R20
```

## B.2 jeff-types
```rust
pub mod ty {
    pub enum Ty { Base(BaseTy), App(TyCon, Vec<TyArg>),
        Ref { mutable: bool, inner: Box<Ty> }, Own(Box<Ty>), Secret(Box<Ty>),
        Refine { base: Box<Ty>, pred: Predicate }, Fn(Box<Ty>, Box<Ty>, EffectRow) }
    pub enum TyArg { Ty(Ty), Nat(NatTerm) } // type-level nat
    pub struct EffectRow(pub BTreeSet<Effect>);
    pub enum Effect { IO, Div, Alloc, Rand, Unsafe }
    impl EffectRow { pub fn is_total(&self) -> bool /* Div,IO ∉ self */; pub fn union(&self, o:&Self)->Self; }
}
pub mod ctx {
    pub struct Context { /* linear/affine 사용 추적 */ }
    impl Context {
        pub fn lookup(&self, x: &Ident) -> Option<&ty::Ty>;
        pub fn use_value(&mut self, x: &Ident) -> Result<(), Diagnostic>; // own=정확히1회, affine=최대1회
        pub fn borrow_shared(&mut self, x: &Ident) -> Result<(), Diagnostic>;
        pub fn borrow_mut(&mut self, x: &Ident) -> Result<(), Diagnostic>; // 배타성 검사
    }
}
pub struct CheckResult { pub core: jeff_core_ir::CoreIr, pub diagnostics: Vec<Diagnostic> }
pub fn check(p: ast::Program) -> Result<CheckResult, Vec<Diagnostic>>; // 타입+effect+linearity+secret-taint+termination

// refinement → Z3 (PART 7.5)
pub struct RefineObligation { pub pred: Predicate, pub span: Span }
pub fn discharge_refinements(obs: &[RefineObligation]) -> Vec<(RefineObligation, VerifyResult)>;
// 증명 실패/Unknown → 런타임 검사 fallback (tagged, R1)

// termination (Total mode, R34/§7.7)
pub enum TerminationProof { Structural, Sized, Guarded, Fails(Diagnostic) }
pub fn check_termination(f: &FnDecl) -> TerminationProof;

// secret-taint (R6/§7.4)
pub fn check_secret_taint(f: &FnDecl) -> Vec<Diagnostic>; // branch/index/timing on secret[T] → 에러
```

## B.3 jeff-core-ir
```rust
pub struct CoreIr { pub funcs: Vec<CoreFn> }
pub struct CoreFn { pub name: Ident, pub params: Vec<(Var, ty::Ty)>, pub body: CoreExpr,
    pub effect: ty::EffectRow, pub total: bool, pub span: Span }
pub enum CoreExpr { Var(Var), Lit(Lit), App(Box<CoreExpr>, Vec<CoreExpr>),
    Lam(Vec<(Var, ty::Ty)>, Box<CoreExpr>), Let(Var, Box<CoreExpr>, Box<CoreExpr>),
    Match(Box<CoreExpr>, Vec<(CorePattern, CoreExpr)>),
    Reduction { kind: RedKind, binder: Vec<Var>, domain: CoreDomain, body: Box<CoreExpr> },
    Prim(PrimOp, Vec<CoreExpr>) }
pub enum CoreDomain { Range { lo: Box<CoreExpr>, hi: Box<CoreExpr>, inclusive: bool },
    AffineSet(AffineConstraints), Other(Box<CoreExpr>) }
pub fn verify_invariants(ir: &CoreIr); // R18 debug-assert: type 보존, linearity, effect 정합, span 보존
```

## B.4 jeff-absint
```rust
pub trait AbstractDomain: Clone {
    fn bottom() -> Self; fn join(&self, o: &Self) -> Self; fn widen(&self, o: &Self) -> Self;
    fn transfer(&self, e: &CoreExpr) -> Self;
}
pub struct Intervals;   pub struct Congruences;   pub struct Polyhedra { isl: IslCtx }
pub struct AnalysisResult {
    pub bounds: Bounds,
    pub dependence: Option<IslDependence>,   // 1회 생성 → JLIR meta로 전달 (백엔드 재계산 금지)
    pub affine_domain: Option<IslSet>,       // affine nest면 Some → Barvinok/B-loop
    pub data_dep: DataDepSummary,
}
pub fn analyze(ir: &CoreIr) -> AnalysisResult;
// HONEST_DEFER detection 보조 (PART 12)
pub fn is_unbounded_accumulation(d: &LoopCarriedDep) -> bool; // → data-dependent-omega-N
```

## B.5 jeff-recognizer (PART 6.3 확장)
```rust
pub mod rules {
    pub fn ruleset() -> Vec<Rewrite>; // CR(BWZ/van Engelen), Faulhaber, floor_sum 상호성,
                                      // residue_split, holonomic closure(sum/product/shift), cole_hopf
}
pub struct CostModel { pub weights: Weights } // PGO profile_weight로 파라미터화
impl CostModel { pub fn cost(&self, e: &EGraphNode) -> AsymptoticCost; }
pub fn recognize(ir: &CoreIr, budget: SaturationBudget) -> Recognized; // PART 6.3
pub fn classify(extracted: &Extracted, cost: &AsymptoticCost) -> DispatchTag;
```

## B.6 jeff-collapse-arith
```rust
pub struct CrCore;      // 기존 엔진 재사용 (Faulhaber, floor_sum, periodic, Sturm, residue, Brent, matrix-power-mod, Bostan-Mori)
pub struct Holonomic;   pub struct KernelPack;
impl Collapser for CrCore     { /* ... */ }
impl Collapser for Holonomic  { /* ... */ }
impl Collapser for KernelPack { /* ... */ }

pub mod gosper {   // indefinite Σ
    pub fn antidifference(term_ratio: &RatFn) -> Option<ClosedForm>;
    // GP normal form → Gosper eq a·x(k+1)−b·x(k)=c → degree-bounded polynomial solve; 해 없음 → None
    pub fn certificate(t: &Hypergeom, s: &ClosedForm) -> Evidence; // PolynomialIdentity( S(k+1)−S(k)−t(k)≡0 )
}
pub mod zeilberger { // definite Σ_k
    pub fn telescoper(f: &Hypergeom, max_order: usize) -> Option<(Operator, RatFn)>; // (L, G)
    pub fn certificate(f: &Hypergeom, l: &Operator, g: &RatFn) -> Evidence; // Telescoper{ l, r:g }
}
pub mod kernels {
    pub fn bostan_mori(rec: &LinRec, n: &BigUint, modulus: &BigUint) -> BigUint; // cert: NumericResidual
    pub fn matrix_power(a: &Mat, n: &BigUint) -> Mat;                            // cert: EigenCharpoly
    pub fn fft_ntt(a: &[ModInt], b: &[ModInt]) -> Vec<ModInt>;                   // exact mod q
    pub fn randomized_svd(a: &Mat<f64>, k: usize, p: usize) -> Option<Svd>;      // precond: low-rank; else None→constant-factor-only (R19)
    pub fn lanczos(a: &Sparse, k: usize) -> KrylovResult;                        // precond: sparse
    pub fn gmres(a: &Sparse, b: &[f64]) -> KrylovResult;                         // nonsymmetric sparse
    pub fn fmm(charges: &[Charge], kern: Kernel) -> Potentials;                  // precond: decaying kernel; O(N²)→O(N)
    pub fn cole_hopf(burgers: &Pde) -> HeatPde;                                  // exact linearization (recognizer rule 협동)
    pub fn koopman_dmd(traj: &[State], obs: &Observables) -> LinearModel;        // linearizable dynamics → linear-recurrence collapser
    pub fn riccati_schur(a:&Mat,b:&Mat,q:&Mat,r:&Mat) -> Mat;                     // LQR via Hamiltonian Schur
    pub fn anderson(g: &dyn Fn(&[f64])->Vec<f64>, m: usize) -> FixedPoint;        // 수렴 가속
    pub fn padé(series: &[Rat], l: usize, m: usize) -> RatFn;                     // 급수 → 유리함수
    pub fn rmt_denoise(a: &Mat<f64>) -> Mat<f64>;                                 // Wigner/Marchenko-Pastur 마스킹
}
// barrier: non-Gosper-summable / non-affine-domain(→3b) / groebner-blowup / constant-factor-only
```

## B.7 jeff-collapse-gf2
```rust
pub struct Gf2Folder;
impl Collapser for Gf2Folder { /* ... */ }
pub struct AffineRegion { pub gates: Vec<GateRef>, pub inputs: Vec<WireId>, pub outputs: Vec<WireId> }
pub fn partition(c: &BitCircuit) -> Vec<AffineRegion>;            // 2-input AND/OR/MUX에서 cut
pub fn accumulate(r: &AffineRegion) -> (Gf2Matrix, Gf2Vec);      // (M, b)
pub mod four_russians {                                          // clean-room (M4RI 링크 금지, R5)
    pub fn mat_vec(m: &Gf2Matrix, x: &Gf2Vec) -> Gf2Vec;         // Gray-code 테이블
    pub fn mat_mat(a: &Gf2Matrix, b: &Gf2Matrix) -> Gf2Matrix;
}
pub fn certificate(c: &BitCircuit, m: &Gf2Matrix, b: &Gf2Vec) -> Evidence; // basis {0,e1..en} → Gf2LinearIdentity
pub fn detect_nonlinearity(c: &BitCircuit) -> Option<GateRef>;   // → nonlinearity barrier
```

## B.8 jeff-collapse-holographic
```rust
pub struct Holographic;
impl Collapser for Holographic { /* ... */ }
pub fn is_planar(g: &CspGraph) -> bool;                          // linear-time; 실패 → non-planar
pub fn matchgrid(csp: &PlanarCsp) -> MatchGrid;
pub fn fkt_pfaffian(grid: &MatchGrid) -> BigInt;                 // Pfaffian orientation + Pfaffian
pub fn holant(grid: &MatchGrid) -> Count;
pub fn certificate(grid: &MatchGrid, count: &Count) -> Evidence; // PfaffianHolant → replay
// 경계: Cai–Lu trichotomy — matchgate = planar #CSP. 일반 그래프 #P-hard. 그 이상 주장 금지.
```

## B.9 jeff-collapse-tensor
```rust
pub struct TensorNet;
impl Collapser for TensorNet { /* ... */ }
pub enum TreewidthEstimate { Within(usize), Exceeds }
pub fn estimate_treewidth(net: &TensorNetwork, budget: usize) -> TreewidthEstimate; // min-degree/min-fill
pub fn contraction_order(net: &TensorNetwork) -> Option<Order>;  // 휴리스틱; 최적은 NP-complete
pub fn slice(net: &TensorNetwork, idx: &[Index]) -> Vec<SubNetwork>; // 시간↔메모리 fallback
pub fn certificate(net: &TensorNetwork, result: &Tensor) -> Evidence; // NumericResidual(exact ring replay)
// barrier: treewidth-blowup (est > budget)
```

## B.10 jeff-barvinok (Layer 3b; clean-room on MIT-isl/imath, R5)
```rust
pub fn count(domain: &IslSet, params: &IslSpace) -> Option<QuasiPolynomial>;
mod cone_decomp { /* clean-room Brion / signed unimodular cone decomposition */ }
mod gen_func   { /* short rational generating functions */ }
pub fn certificate(domain: &IslSet, qp: &QuasiPolynomial) -> Evidence;
// chamber decomposition(Presburger, Z3) + sample 파라미터 enumeration 일치(유한)
// barrier: non-affine-domain; 가변차원 → sharp-P-hard
```

## B.11 jeff-stdlib (모듈 구조; language-spec §10)
```
core        prelude; int/nat/rat/mod/bv/Vec/Mat/Fin; ownership 기본형; Option/Result
fold        faulhaber, floor_sum, periodic_quasipoly, residue_split,
            matrix_power_mod, bostan_mori, holonomic(gosper, zeilberger)  // 인식+호출 가능
numeric     randomized_svd[low-rank], krylov{lanczos,gmres,arnoldi}[sparse], fmm[decaying],
            chebyshev_filter, anderson_accel, sherman_morrison, wiener_khinchin,
            sinkhorn_ot, l1_cs, cole_hopf[burgers→heat], koopman_dmd[linearizable],
            riccati_schur[LQR], fft, ntt, symplectic_integrator, rmt_denoise
            // R19: 각 함수가 구조 전제부를 계약으로; 부재 시 constant-factor-only로 강등
crypto.pqc  ml_kem, ml_dsa (mod[q]+NTT 기반); 전 구간 secret[T] + @constant_time (R28)
proof       Certificate 재노출, Z3/Lean 헬퍼, refinement 헬퍼
poly        isl 기반 affine domain 헬퍼(분석/count)
```

## B.12 jeffc (driver)
```rust
pub struct Options { pub emit_certificates: Option<PathBuf>, pub collapse_report: bool,
    pub total: bool, pub const_time_audit: bool, pub emit_jlir: bool, pub emit_llvm: bool,
    pub opt_level: u8 }
pub fn compile(src: &str, opts: &Options) -> Result<Artifact, Vec<Diagnostic>>;
// pipeline (PART 8): parse → check → analyze(absint) → recognize → dispatch → collapse|defer
//                   → verify → JLIR → backend(Layer B) → codegen(LLVM)
//                   모든 경계 fallback (R1); 어떤 단계도 hang 금지 (R23)
pub fn collapse_report(art: &Artifact) -> Report; // per-function: collapsed(layer,cost,cert) | deferred(tag)
```

## B.13 jeff-test-oracles (DEV-ONLY)
```rust
// out-of-process only. release 의존성 그래프에 절대 미포함 (R5, ci/license_scan).
pub fn sage_sum(expr: &str) -> Option<String>;        // differential test
pub fn mathematica_sum(expr: &str) -> Option<String>;
pub fn barvinok_count(domain: &str) -> Option<String>; // GPL → 프로세스 호출만, 링크 금지
```

---
---

# APPENDIX C — COMPLETE TYPING-RULE CALCULUS (jeff-types)

> PART 7은 핵심 규칙을 요약했다. 여기서는 jeff-types 구현이 따라야 할 *완전한 추론 규칙*과 *soundness 정리*를 명시한다. 정리(T1–T6)는 구현이 지켜야 할 의무다 — 기계 증명(Lean)은 별도 추적이며, 미증명을 증명된 듯 주장하지 않는다(P1/DR2).

## C.1 판단 형식 (Judgments)
```
Γ ⊢ e : τ ! ε        e는 Γ 아래에서 타입 τ, effect ε
Γ ⊢ τ : κ            τ는 kind κ (Type | Nat)
Γ ⊨ φ ⟹ ψ           refinement 함의 (Z3로 discharge)
Γ ⊢ e : τ  total     e는 total (종료 ∧ {IO,Div} 부재)
```
Context Γ: 바인딩에 multiplicity 부착 — `x :^ω τ`(unrestricted), `x :^a τ`(affine, 최대 1회), `x :^1 τ`(linear, 정확히 1회). linear/affine 자원은 부분식 간 *분할*: `Γ = Γ₁ ⊎ Γ₂` (unrestricted 공유, affine/linear 분배). effect ε ⊆ {IO, Div, Alloc, Rand, Unsafe}.

## C.2 변수 / 리터럴 / 함수
```
x :^ω τ ∈ Γ
───────────────── [VAR-U]
Γ ⊢ x : τ ! ∅

──────────────────────── [VAR-LIN]
Γ, x:^1 τ ⊢ x : τ ! ∅        (x 소비; residual에서 제거)

──────────────── [LIT]
Γ ⊢ lit : ty(lit) ! ∅

Γ, x:τ ⊢ e : σ ! ε
──────────────────────────── [ABS]
Γ ⊢ λx:τ. e : (τ →ε σ) ! ∅

Γ₁ ⊢ e₁ : (τ →ε σ) ! ε₁     Γ₂ ⊢ e₂ : τ ! ε₂
───────────────────────────────────────────── [APP]
Γ₁ ⊎ Γ₂ ⊢ e₁ e₂ : σ ! ε₁ ∪ ε₂ ∪ ε

Γ₁ ⊢ e₁ : τ ! ε₁     Γ₂, x:τ ⊢ e₂ : σ ! ε₂
──────────────────────────────────────────── [LET]
Γ₁ ⊎ Γ₂ ⊢ (let x = e₁ in e₂) : σ ! ε₁ ∪ ε₂
```

## C.3 Ownership (linear/affine; §3.3/§7.3)
```
Γ ⊢ e : own τ ! ε
─────────────────────────────── [MOVE]
Γ ⊢ move(e) : own τ ! ε          (e의 바인딩 소비)

Γ ⊢ e : own τ ! ε     (e가 빌림 동안 move 불가)
──────────────────────────────────────────── [BORROW-SHARED]
Γ ⊢ &e : &τ ! ε

Γ ⊢ e : own τ ! ε     (배타: e의 다른 빌림 없음)
──────────────────────────────────────────── [BORROW-MUT]
Γ ⊢ &mut e : &mut τ ! ε
```
수명 종료점은 ownership analysis가 정적 계산 → `free` 주입(Layer B #4). GC 없음.

## C.4 Secret-taint (§3.4/§7.4; R6 핵심)
```
Γ ⊢ e : τ ! ε     (audited declassify의 역; 또는 secret 입력)
──────────────────────────────────────────────────── [SECRET-INTRO]
Γ ⊢ e : secret τ ! ε

Γ ⊢ e₁ : secret τ ! ε₁   Γ ⊢ e₂ : secret τ ! ε₂   op ∈ DataObliviousOps
──────────────────────────────────────────────────────────────────── [SECRET-OP]
Γ ⊢ e₁ op e₂ : secret τ ! ε₁ ∪ ε₂

Γ ⊢ c : secret bool ! ε
─────────────────────────── [SECRET-IF]   ✗ REJECT (R6)
if c then e₁ else e₂          (secret에 data-dependent branch → 컴파일 에러)

Γ ⊢ i : secret (Fin n) ! ε   Γ ⊢ v : Vec τ n ! ε'
──────────────────────────────────────────────── [SECRET-IDX]  ✗ REJECT (R6)
v[i]                          (secret에 data-dependent index → 컴파일 에러)

Γ ⊢ e : secret τ ! ε     at audited declassification point
────────────────────────────────────────────────────────── [DECLASSIFY]
Γ ⊢ declassify(e) : τ ! ε ∪ {Unsafe}      (로그; 사용 최소화; secret 값 미누설 R36)
```
`DataObliviousOps`: secret에서 timing이 데이터 독립인 연산 — 산술, constant-time select, 비교의 constant-time 형. 일반 비교/branch/index 제외.

## C.5 Refinement (§3.5/§7.5; Z3 discharge)
```
Γ ⊢ e : {x:β | φ} ! ε     Γ ⊨ φ ⟹ ψ        (Z3가 discharge)
──────────────────────────────────────────────────── [REFINE-SUB]
Γ ⊢ e : {x:β | ψ} ! ε
(Z3 실패/Unknown → 런타임 검사 삽입, tagged; R1/R31)

Γ ⊢ v : Vec τ n ! ε     Γ ⊢ i : {k:nat | k < n} ! ε'
───────────────────────────────────────────────────── [INDEX-SAFE]
Γ ⊢ v[i] : τ ! ε ∪ ε'        (경계 증명됨 → 런타임 검사 제거)
```

## C.6 Reduction / match (1급 collapse 입력)
```
Γ ⊢ D : Domain     Γ, b:idx(D) ⊢ e : τ ! ε     τ has commutative monoid
──────────────────────────────────────────────────────────────────── [SUM/PROD/COUNT]
Γ ⊢ (sum b in D: e) : τ ! ε
(fold는 추가로 associativity 요구 → 병렬 reduction 허용; Z3로 assoc 증명, §13)

Γ₁ ⊢ s : τ ! ε₀   ∀i: Γ₂, bind(pᵢ:τ) ⊢ eᵢ : σ ! εᵢ   patterns exhaustive
───────────────────────────────────────────────────────────────────── [MATCH]
Γ₁ ⊎ Γ₂ ⊢ (match s: pᵢ ⇒ eᵢ) : σ ! ε₀ ∪ ⋃ᵢ εᵢ
```

## C.7 Effect & total (§3.6/§7.6)
```
ε ⊆ ε'
──────────────────── [EFFECT-SUB]     (effect는 monotone subtyping)
Γ ⊢ e : τ ! ε  ⟹  Γ ⊢ e : τ ! ε'

Γ ⊢ e : τ ! ε   Div ∉ ε   IO ∉ ε   term(e) ∈ {Struct,Sized,Guard}
───────────────────────────────────────────────────────────────── [TOTAL]
Γ ⊢ e : τ  total
```

## C.8 Termination (Total mode; §4.1/§7.7; R34)
```
recursive call f(a),  a ◁ scrutinee  (구문적 부분항)
─────────────────────────────────────────────────── [TERM-STRUCT]
term(f) = Struct

|a'| <_wf |a|   (sized 인덱스 well-founded 감소)
──────────────────────────────────────────────── [TERM-SIZED]
term = Sized

corecursive call이 constructor 아래에 있음 (productivity)
─────────────────────────────────────────────────────── [TERM-GUARD]
term = Guard

위 셋 중 어느 것도 성립 안 함
────────────────────────────── [TERM-FAIL]   ✗ (General 강등 또는 --total시 에러)
term = Fail
```

## C.9 Soundness 정리 (구현 의무; 미증명을 증명된 듯 주장 금지)
- **T1 Progress.** well-typed closed `e`는 값이거나 step 한다.
- **T2 Preservation.** `Γ⊢e:τ!ε` ∧ `e→e'` ⟹ `Γ⊢e':τ!ε'`, `ε'⊆ε`.
- **T3 Linearity safety.** linear 자원은 정확히 1회, affine은 최대 1회 사용; use-after-move 불가능 (context-split 규율로 보장).
- **T4 Collapse-transparency (per certificate).** `K`가 VerifiedCertificate와 함께 `K'`로 collapse하면, 모든 입력에서 `⟦K⟧ = ⟦K'⟧` (반환값 + effect trace; secret 입력에 대해 timing class도). — language-spec §5.
- **T5 Secret non-interference.** secret 입력을 가진 `e`의 관측 가능 timing class는 secret 값에 *독립*. [SECRET-IF]/[SECRET-IDX] 거부 + 백엔드 secret-taint guard(R6/R29)로 강제.
- **T6 Total termination.** 모든 Total-mode 함수는 종료(structural/sized/guarded 타이핑 ⟹ 그 fragment의 strong normalization).

**증명 상태(정직):** T1–T3은 표준 진행/보존 + 선형 논리 기법으로 구현 검증(테스트+일부 기계 증명). T4는 *per-certificate*로 Z3/Lean이 매 인스턴스 검증(전역 메타정리는 아님; 이게 proof-carrying의 핵심). T5는 타입 규칙+백엔드 가드로 강제하되 timing 모델은 명시된 추상화 수준에서만 보장(하드웨어 미세 타이밍은 범위 밖, 문서화). T6은 termination 타이핑의 건전성으로 환원; 완전 기계 증명은 Lean으로 추적(미완은 미완이라 표기, R24/R30).

---
---

# APPENDIX D — EXPANDED FIXTURE CORPUS

> 회귀·골든 테스트의 권위 코퍼스(R25). `+` = collapse (closed form + certificate kind). `−` = defer (barrier tag). 모든 positive는 산술이 검증됨. 새 collapser는 자기 positive/negative를 추가(R3). differential은 dev-only oracle로 교차검증(R5).

## D.1 Layer 1 — Arithmetic / Holonomic
```
ID    SOURCE (.jeff)                              EXPECTED                        EVIDENCE / TAG
A01 + sum i in 0..=n: i                           n*(n+1)/2                       PolynomialIdentity
A02 + sum i in 0..=n: i*i                          n*(n+1)*(2*n+1)/6               PolynomialIdentity
A03 + sum i in 0..=n: i**3                         (n*(n+1)/2)**2                  PolynomialIdentity
A04 + sum i in 0..=n: i**4                          n(n+1)(2n+1)(3n²+3n−1)/30       PolynomialIdentity
A05 + sum i in 0..n: 2**i                          2**n - 1                        EigenCharpoly/LinState
A06 + sum i in 0..n: r**i                          (r**n - 1)/(r - 1)   (r≠1)      EigenCharpoly
A07 + sum i in 0..n: i*2**i                         (n-2)*2**n + 2                 EigenCharpoly (arith-geom)
A08 + fib(n)  (matrix power / linear rec)           Bostan-Mori closed             EigenCharpoly + NumericResidual
A09 + sum k in 0..=n: C(n,k)                        2**n                           Telescoper (Zeilberger)
A10 + sum k in 0..=n: C(n,k)**2                     C(2*n, n)                       Telescoper
A11 + sum k in 0..=n: k*C(n,k)                      n*2**(n-1)                     Telescoper
A12 + catalan(n) = C(2n,n)/(n+1)                    closed                          Telescoper
A13 + sum i in 0..n: floor((a*i+b)/m)               AtCoder floor_sum              CR (Sturm/residue)
A14 + sum i in 0..n: (a*i+b) % m                    periodic quasi-poly            CR residue_split
A15 + matrix_power_mod(A, n, q)                     A**n mod q  (O(log n))         EigenCharpoly mod q
A16 − sum k in 1..=n: 1/k                           HONEST_DEFER                   non-Gosper-summable (harmonic)
A17 − sum k in 0..=n: 1/(k*k+1)                     HONEST_DEFER                   non-Gosper-summable
A18 − sum i in 0..=n: f(a[i])    (a,f unknown)      HONEST_DEFER                   data-dependent-omega-N
A19 − sum i in 0..=n: g(i) where g recurses on data HONEST_DEFER                  data-dependent-omega-N
A20 + prod i in 1..=n: i                            factorial via CR/holonomic     Telescoper (또는 정의 전개)
```
검증: A01–A04 power sums는 Faulhaber; certificate는 S(n)−S(n−1)−term ≡ 0 + base (PART 11 예시 1, APPENDIX F.1). A09–A12는 Zeilberger telescoper. 샘플 확인: A10 n=3 → 1+9+9+1=20=C(6,3)=20 ✓; A11 n=3 → 0+3+6+3=12=3·4 ✓; A07 n=3 → 0+2+8=10=(1)·8+2 ✓.

## D.2 Layer 3b — Barvinok (affine counting)
```
B01 + count (i,j) in {0<=i<=j<=n}: 1               (n+1)*(n+2)/2                   chamber+sample
B02 + count (i,j) in {0<=i<n, 0<=j<n}: 1            n*n                            chamber+sample
B03 + count (i,j,k) in {0<=i<=j<=k<=n}: 1           (n+1)(n+2)(n+3)/6              chamber+sample
B04 + count i in {0<=i<n, i%2==0}: 1               quasi-poly period 2 (⌈n/2⌉)     chamber+sample
B05 + count (i,j) in {0<=i<n, 0<=j<=i}: 1           n*(n+1)/2                      chamber+sample
B06 − count (i,j) in {i*i + j*j <= n}: 1            HONEST_DEFER                   non-affine-domain
B07 − count over parametric variable-dim simplex    HONEST_DEFER                   sharp-P-hard
```
검증: B01 n=3 → i≤j in {0..3} = 10 = 4·5/2 ✓. B03 n=2 → C(5,3)=10; enumerate 0≤i≤j≤k≤2 = 10 ✓.

## D.3 Layer 2 — GF(2)
```
G01 + y = rotl(x,1) ^ x            (bv[64], linear)  y = M·x                       Gf2LinearIdentity
G02 + parity / linear checksum (bv[n])               y = M·x                       Gf2LinearIdentity
G03 + affine: y = M·x ^ const                          (M, b)                      Gf2LinearIdentity
G04 + CRC step (linear feedback)                      y = M·x                      Gf2LinearIdentity
G05 − AES round (SubBytes S-box 포함)                 linear layer collapses;       nonlinearity (at S-box)
                                                       S-box → defer
G06 − any circuit with 2-input AND on inputs          HONEST_DEFER at AND           nonlinearity
```
규칙: G05에서 ShiftRows/MixColumns(선형)는 (M,b)로 collapse하되 SubBytes(비선형 S-box)에서 `nonlinearity` defer — 이게 보안적으로 옳다(linearization 방어).

## D.4 Layer 3 / Layer 4 — Holographic / Tensor
```
H01 + planar perfect matching count                  Pfaffian = |Pf(A)|             PfaffianHolant
H02 + planar #CSP (matchgate-expressible)             Holant collapse               PfaffianHolant
H03 − non-planar #CSP (K5 / K3,3 minor)               HONEST_DEFER                  non-planar
T01 + bounded-treewidth tensor net (tw <= budget)     contraction collapse          NumericResidual
T02 − dense random circuit (tw > budget)              HONEST_DEFER                  treewidth-blowup
T03 + tensor net with slicing (tw slightly > mem)     sliced contraction            NumericResidual
```

## D.5 PQC (crypto.pqc)
```
P01 + @constant_time poly_mul(secret a, b) over mod[3329]   NTT collapse           NumericResidual(exact mod) + const-time-audit green
P02 − if sk[0] == 0 { .. }   (branch on secret)             COMPILE ERROR          [SECRET-IF] (R6)
P03 − table[sk_byte]         (index by secret)              COMPILE ERROR          [SECRET-IDX] (R6)
P04 + ml_kem keygen/encaps/decaps (secret 전 구간)          collapse where exact;  const-time-audit green
                                                            else defer + tag
```

## D.6 Rejected (PART 18 non-goals)
```
R01 − @collapse solve(phi) = argmin_ising(encode(phi))      COMPILE ERROR           np-hard
R02 − "geometric O(1)" arbitrary-search request             rejected at design      physics-counterfactual
R03 − classical NLSE search w/ inject_oracle_target(secret) rejected (circular)     physics-counterfactual
R04 − memory-hard KDF "collapse" (Argon2)                   HONEST_DEFER            memory-hard
```

---
---

# APPENDIX E — PER-COLLAPSER DETAILED IMPLEMENTATION

> 가장 까다롭고 가치 높은 collapser들의 알고리즘 수준 명세. 참고문헌은 spec(architecture). 모든 collapser는 checker-first(R2), fallback 필수(R1), half-collapse 금지(R32).

## E.1 Gosper (indefinite hypergeometric summation)
**입력:** hypergeometric term `t_k` (즉 `t_{k+1}/t_k = r(k) ∈ ℚ(k)`). **출력:** closed antidifference `S(k)` (with `S(k+1)−S(k)=t_k`) 또는 `non-Gosper-summable`.
```
fn gosper(t):
    r(k) = simplify(t(k+1) / t(k))                       # ∈ ℚ(k); 아니면 defer (not hypergeometric)
    (a, b, c) = gosper_petkovsek_normal_form(r)          # r = (a(k)/b(k)) · (c(k+1)/c(k))
        # 조건: gcd(a(k), b(k+j)) = 1 ∀ j ∈ ℤ≥0
        # 구현: 정수근 N = { j≥0 : Res_k(a(k), b(k+j)) = 0 } 를 resultant로 구해 c로 이전
    deg_x = gosper_degree_bound(a, b, c)                  # 아래
    if deg_x < 0: return Defer(non_gosper_summable)
    # Gosper 방정식: a(k)·x(k+1) − b(k−1)·x(k) = c(k),  deg x ≤ deg_x
    x = solve_poly_linear_system(a, b, c, deg_x)          # 미정계수 → 선형계
    if x is None: return Defer(non_gosper_summable)
    S(k) = (b(k−1) / c(k)) · x(k) · t(k)
    cert = PolynomialIdentity( clear_denoms( S(k+1) − S(k) − t(k) ) ≡ 0 )  # → Z3 (APPENDIX F.1)
    return Collapsed(S, verify(cert))                     # verify 실패 → Defer (R31)
```
**degree bound (Petkovšek–Wilf–Zeilberger, "A=B" Ch.5):**
```
let da = deg a, db = deg b
if da ≠ db:
    deg_x = deg c − max(da, db)
else:                                  # da = db = d
    A = lc(a), B = lc(b)               # leading coeffs
    if A ≠ B:
        deg_x = deg c − d
    else:                              # A = B: 다음 차수 항 비교
        a1 = coeff(a, d−1); b1 = coeff(b, d−1)   # b는 b(k−1)로 평가했음에 주의
        ell = (b1 − a1) / A            # 정수여야 함
        deg_x = max(deg c − d, ell)
if deg_x < 0: NOT_SUMMABLE
```
**certificate (DR1):** 반드시 실제 Z3 통과. rational identity → 분모 제거 → ℤ[k] 단일 다항식 ≡ 0 → 계수 0 검사 또는 Z3 NRA. unknown → defer.

## E.2 Zeilberger (creative telescoping / definite sum)
**입력:** proper hypergeometric `F(n,k)`. **출력:** telescoper `L = Σ_{i=0}^{J} a_i(n) N^i` (합의 recurrence) + certificate rational `R(n,k)`.
```
fn zeilberger(F, max_order):
    for J in 0..=max_order:
        # ansatz: (S_k − 1)[G] = (L F),  G = R(n,k)·F,  미지수 a_0..a_J(n), R
        # (L F)(n,k) = Σ_i a_i(n) F(n+i, k)
        sys = build_gosper_with_parameters(F, J)   # a_i를 미지 파라미터로 둔 Gosper
        sol = solve_for_params_and_R(sys)
        if sol exists:
            L = Σ_i sol.a_i(n) · N^i
            R = sol.R(n,k)
            cert = Telescoper{ l: L, r: R }
            # 검증식: R(n,k+1)F(n,k+1) − R(n,k)F(n,k) = Σ_i a_i(n) F(n+i,k)
            #   양변 / F(n,k) → ℚ(n,k) 유리 항등식 (binomial 비율 치환) → 분모 제거 → 다항 항등식 → Z3
            return Collapsed(recurrence_from(L), verify(cert))   # 또는 Lean (operator induction)
    return Defer(non_gosper_summable)                # 차수 한계 내 telescoper 없음
```
**경계항(boundary):** definite 합이면 telescoping이 양 끝에서 사라지는지 확인(natural boundary 또는 명시 보정). cert.boundaries에 기록(R16).

## E.3 Four-Russians (GF(2) — clean-room; M4RI 링크 금지 R5)
**용도:** (a) 고정 (M,b) 맵의 반복 적용, (b) 선형 레이어 합성/제곱(M4RM).
```
# (a) mat-vec, word-parallel: y_i = popcount(M_row_i & x) mod 2   → O(mn/w)
# (b) M4RM (boolean C = A·B, A:m×p, B:p×n) — Gray-code 테이블:
fn m4rm(A, B):
    k = floor(log2(max(p, 2)))                  # 스트라이프 폭
    C = zeros(m, n)
    for stripe in chunks(0..p, k):              # A의 세로 스트라이프 / B의 가로 블록
        T = build_gray_table(B, stripe, k)      # 2^k 행: T[g] = Σ_{bit set in Gray(g)} B[stripe[bit]]
                                                # Gray code → 각 새 행 = 직전 행 ⊕ 한 줄 (1 XOR/row)
        for i in 0..m:
            idx = bits_of(A[i], stripe)         # k-bit 인덱스
            C[i] ^= T[idx]                       # 룩업 1회
    return C
# 복잡도: O(mn·p/k) + 테이블 O(2^k · n / k) ≈ O(n³ / log n) (한 레벨)
```
**certificate:** basis `{0, e_1..e_n}` (n+1점) 평가로 affine map 결정 → `Gf2LinearIdentity{M,b}` → Z3 over GF(2) (APPENDIX F.3). nonlinear gate 만나면 partition이 거기서 cut + `nonlinearity` defer.

## E.4 Bostan–Mori (N-th term of linear recurrence)
**입력:** 선형점화 char poly `Q(x)` (deg d), 생성함수 `P(x)/Q(x)`. **출력:** `[x^N] P/Q` in `O(M(d) log N)`.
```
fn bostan_mori(P, Q, N, modulus):                # 모든 연산 mod q (exact)
    while N > 0:
        Qm = Q(−x)                               # 부호 교대
        U  = P * Qm                               # 분자
        V  = Q * Qm                               # = V(x²) : 짝수 차수만 남음
        # V(x) = Ve(x²);  U(x) = Ue(x²) + x·Uo(x²)
        if N even: P = even_part(U); 
        else:      P = odd_part(U)
        Q = even_part(V) as poly in x            # deg 유지
        N = floor(N / 2)
    return (P(0) / Q(0)) mod modulus              # [x^0]
```
**certificate:** `NumericResidual` — 작은 N들에서 직접 점화 전개와 일치(modular exact). 큰 N은 두 번째 modulus로 교차(확률적 오류 무시 가능 수준; 또는 CRT).

## E.5 FKT / Pfaffian orientation (planar #matchings → Pfaffian)
**입력:** planar graph `G` (가중치 옵션). **출력:** perfect matching 수 `= |Pf(A)|` (가중합 `= Pf(A)`).
```
fn fkt(G):
    assert is_planar(G)                          # 아니면 caller가 non_planar defer
    emb = planar_embedding(G)                    # 면(face) 구조
    T   = spanning_tree(G); orient_arbitrary(T)
    # Pfaffian orientation: 각 면이 시계방향 간선 홀수 개가 되도록 비-트리 간선 방향 결정
    for f in faces_in_BFS_order(emb):            # 트리 쌍대에서 leaf→root
        e = unique_unoriented_edge(f)
        orient e so that clockwise_count(f) is odd
    A = skew_symmetric_matrix(G, orientation)    # a_ij = ±w_ij (방향), a_ji = −a_ij
    pf = pfaffian(A)                             # FKT: |#PM| = |Pf|; det(A) = Pf(A)²
    return pf
```
**certificate:** `PfaffianHolant` — (1) orientation이 Pfaffian인지(모든 면 홀수) 재검, (2) `det(A) = Pf(A)²` 재계산, (3) Holant 등식. exact ring(int)으로 replay.
**경계:** `is_planar` 실패 → `non-planar` (그러면 #P-hard, Cai–Lu). matchgate 표현 불가 #CSP도 defer.

## E.6 Treewidth estimation (min-degree / min-fill)
**입력:** interaction graph `G`, budget `B`. **출력:** `Within(tw)` 또는 `Exceeds`.
```
fn estimate_treewidth(G, B):
    H = G.clone(); tw = 0
    while H.has_vertices():
        v = pick(H)                              # min-degree: deg 최소 / min-fill: fill 최소
        nb = neighbors(H, v)
        tw = max(tw, |nb|)                        # 제거 시 clique 크기 − 1
        if tw > B: return Exceeds                  # 조기 종료 (budget, DR8 보수적)
        make_clique(H, nb)                         # fill edges
        H.remove(v)
    return Within(tw)
# min-degree: pick = argmin deg(v)
# min-fill:   pick = argmin (제거 시 새로 생기는 간선 수)
```
**사용:** `Exceeds` → `treewidth-blowup` defer. `Within`이면 contraction order로 진행; 중간 tensor가 메모리 초과면 `slice`(시간↔메모리). **certificate:** contraction 결과를 exact ring으로 replay(`NumericResidual`).

## E.7 공통: collapser 골격 (R1/R2/R31/R32 준수)
```rust
fn try_collapse(&self, ir: &CoreIr) -> CollapseOutcome {
    let Some(form) = self.derive_closed_form(ir) else {
        return CollapseOutcome::Defer(self.barrier(ir));     // 구조 없음 → tag
    };
    let cert = self.build_certificate(ir, &form);            // checker는 이미 구현됨 (R2)
    match jeff_verify::verify(cert) {                         // 실제 검증 (DR1)
        Some(vc) => CollapseOutcome::Collapsed(Collapsed::new(form, vc)),  // 전체만 (R32)
        None     => CollapseOutcome::Defer(self.barrier(ir)), // Unknown/Invalid → fallback (R31)
    }
}
```

---
---

# APPENDIX F — Z3 / LEAN CERTIFICATE PROOF WALKTHROUGHS

> jeff-verify가 각 evidence kind를 어떻게 실제로 discharge하는지의 구체 스크립트. 모든 collapse는 이 검증을 *실제로* 통과해야 한다(DR1). `unsat`(부정의 불충족) = 항등식 성립 = `Valid`. `unknown`/`timeout` = `Valid 아님` → 원본 fallback(R31).

## F.1 PolynomialIdentity — Faulhaber (Z3, NRA)
`S(n)=n(n+1)(2n+1)/6` 가 `S(n)−S(n−1)=n²` 와 `S(0)=0` 을 만족함을 증명(→ 합과 동치):
```smt2
(declare-const n Real)
(define-fun S ((k Real)) Real (/ (* k (+ k 1.0) (+ (* 2.0 k) 1.0)) 6.0))
(push) (assert (not (= (- (S n) (S (- n 1.0))) (* n n)))) (check-sat) (pop)  ; expect: unsat
(push) (assert (not (= (S 0.0) 0.0)))                      (check-sat) (pop)  ; expect: unsat
```
두 `unsat` → 차분+초기값으로 합 동치 확정. (계수-0 검사 변형: 차분 다항식을 전개해 모든 계수=0을 유리수 산술로 확인 — quantifier-free, 더 견고.)

## F.2 Telescoper — Central binomial `Σ C(n,k)² = C(2n,n)` (Z3, 그리고 Lean 대안)
GACC 경로: 합과 `C(2n,n)`이 *같은 1차 점화 + 같은 초기값*을 만족 → 동치. 점화 `(n+1)a(n+1)=(4n+2)a(n)` 를 비율로 검증:
```smt2
; C(2n+2,n+1)/C(2n,n) = (2n+1)(2n+2)/((n+1)^2)  이므로
; (n+1)*C(2n+2,n+1) = (4n+2)*C(2n,n)  ⇔  (2n+1)(2n+2)/(n+1) = 4n+2
(declare-const n Real) (assert (>= n 0.0))
(assert (not (= (/ (* (+ (* 2.0 n) 1.0) (+ (* 2.0 n) 2.0)) (+ n 1.0)) (+ (* 4.0 n) 2.0))))
(check-sat)   ; expect: unsat   ((2n+1)·2(n+1)/(n+1) = 2(2n+1) = 4n+2 ✓)
```
합 쪽 점화는 Zeilberger telescoper `Telescoper{L,R}`로: `R(n,k+1)F(n,k+1)−R(n,k)F(n,k) = Σ_i a_i(n)F(n+i,k)` 를 binomial 비율 `C(n+1,k)/C(n,k)=(n+1)/(n+1−k)`, `C(n,k+1)/C(n,k)=(n−k)/(k+1)` 로 치환 → 분모 제거 → ℤ[n,k] 다항 항등식 → Z3.
**Lean 대안 (operator induction; 더 강한 보증):**
```lean
import Mathlib
open Nat
-- mathlib: Nat.succ_mul_centralBinom_succ : (n+1) * centralBinom (n+1) = 2*(2*n+1) * centralBinom n
example (n : ℕ) : (n + 1) * centralBinom (n + 1) = (4 * n + 2) * centralBinom n := by
  have h := Nat.succ_mul_centralBinom_succ n
  ring_nf at h ⊢
  linarith [h]   -- 2*(2n+1) = 4n+2
-- 합 = centralBinom 은 Vandermonde 항등식으로 (mathlib Finset.sum); 정확 lemma는 빌드시 확정.
```

## F.3 Gf2LinearIdentity — 선형 비트회로 (Z3, BitVec)
`circ(x) = (x<<<1) ^ x` (bv[4], 선형)이 `M·x` 와 동치임을 검증. M은 basis로 결정:
```smt2
(define-fun circ ((x (_ BitVec 4))) (_ BitVec 4) (bvxor ((_ rotate_left 1) x) x))
; M의 열 = circ(e_i): col0=circ(0001), col1=circ(0010), col2=circ(0100), col3=circ(1000)
(define-fun col0 () (_ BitVec 4) (circ #b0001))
(define-fun col1 () (_ BitVec 4) (circ #b0010))
(define-fun col2 () (_ BitVec 4) (circ #b0100))
(define-fun col3 () (_ BitVec 4) (circ #b1000))
; M·x = ⊕_{i: x_i=1} col_i  (선형성)
(define-fun Mx ((x (_ BitVec 4))) (_ BitVec 4)
  (bvxor (ite (= #b0001 (bvand x #b0001)) col0 #b0000)
         (bvxor (ite (= #b0010 (bvand x #b0010)) col1 #b0000)
                (bvxor (ite (= #b0100 (bvand x #b0100)) col2 #b0000)
                       (ite (= #b1000 (bvand x #b1000)) col3 #b0000)))))
(declare-const x (_ BitVec 4))
(assert (not (= (circ x) (Mx x))))
(check-sat)   ; expect: unsat  → circ 는 GF(2)-선형, M = [col0..col3]
```
affine(b≠0)면 `circ(0)=b` 를 더해 검증. nonlinear gate가 있으면 partition이 거기서 cut, 이 회로는 collapse 안 함 → `nonlinearity`.

## F.4 Barvinok chamber — affine count (Z3 Int + sample replay)
`count {0≤i≤j≤n} = (n+1)(n+2)/2`:
```smt2
; (a) 유한 sample (정확): n∈{0,1,2,3} → 1,3,6,10 = f(n) ✓  (테스트로 직접 enumeration)
; (b) parametric 검증: f(n)−f(n−1) = n+1  (새 쌍 (i,n), 0≤i≤n 의 개수)
(declare-const n Int) (assert (>= n 0))
(assert (not (= (- (div (* (+ n 1) (+ n 2)) 2) (div (* n (+ n 1)) 2)) (+ n 1))))
(check-sat)   ; expect: unsat   (단일 chamber n≥0; 정수 div 짝수성은 (n+1)(n+2) 연속곱으로 보장)
```
quasi-poly(B04 같은 주기성)면 chamber/residue별로 분할 검증.

## F.5 EigenCharpoly — matrix power (Z3, ring identity; Cayley–Hamilton)
2×2 `A`의 `A² − tr·A + det·I = 0` 를 entrywise로 검증 → `A^n` 은 `p(A)=0` 점화로 닫힘(eigen/Bostan–Mori):
```smt2
(declare-const a Real)(declare-const b Real)(declare-const c Real)(declare-const d Real)
; (A² − tr·A + det·I)_11 = (a²+bc) − (a+d)a + (ad−bc)
(assert (not (= (+ (- (+ (* a a) (* b c)) (* (+ a d) a)) (- (* a d) (* b c))) 0.0)))
(check-sat)   ; expect: unsat   ; (1,2),(2,1),(2,2) 엔트리도 동일하게 반복
```

## F.6 검증 라우팅 (recap) & unknown 처리
```
Evidence::PolynomialIdentity  → Z3Checker (NRA / 계수-0)        [F.1, F.4, F.5]
Evidence::Gf2LinearIdentity   → Z3Checker (BitVec / GF(2))      [F.3]
Evidence::EigenCharpoly       → Z3Checker (ring identity)        [F.5]
Evidence::Telescoper          → Z3Checker (poly identity) | LeanChecker (operator induction)  [F.2]
Evidence::NumericResidual     → ReplayChecker (exact modular/int recompute)   [Bostan-Mori, tensor]
Evidence::PfaffianHolant      → ReplayChecker (Pfaffian/orientation/Holant replay)  [FKT]
```
규칙: `verify` 가 `Valid` 가 아니면(`Invalid`/`Unknown`/`timeout`) `VerifiedCertificate` 생성 안 함 → collapser는 원본 fallback(R31). solver timeout은 명시 설정 + fallback(R23). secret 값은 어떤 SMT 모델/로그에도 출력 금지(R36).

---
---

# APPENDIX G — RUNTIME, MEMORY LAYOUT, ABI, CONCURRENCY

> codegen(jeff-codegen)과 런타임의 권위. backend 문서와 정합. secret 경로는 R6/R29/R36 강제.

## G.1 값 표현 (Value representations)
| 타입 | 런타임 표현 | 라이브러리 (R5: MIT/Apache/BSD만) |
|---|---|---|
| `i8..i64`, `u8..u64` | native machine int | (내장) |
| `int`, `nat` | arbitrary-precision bignum | **num-bigint (MIT/Apache)**. ⚠ GMP/`rug`은 LGPL → **금지**(R5). |
| `rat` | bignum 분수 (기약) | num-rational (MIT/Apache) |
| `mod[q]` | Montgomery 표현 (q 고정폭이면 native, 아니면 bignum) | in-house Montgomery |
| `bv[n]` | packed words (⌈n/64⌉ × u64) | in-house |
| `f32/f64` | IEEE-754 | (내장) |
| `Vec[T,n]` | 연속 버퍼, 64B 정렬 | in-house |
| `Mat[T,m,n]` | row-major, 64B 정렬 | in-house |
규칙: collapse 경로 산술은 `int/nat/rat/mod`로 *정확*(R33). closed form이 bignum이어도 오버플로 없음(Faulhaber 결과 등).

## G.2 Ownership → free codegen (GC 없음; Layer B #4)
```
ownership analysis → 각 own 값의 마지막 사용 지점(last-use) 계산
codegen: last-use 직후 drop 삽입 (RAII식). linear=정확히 1 drop, affine=≤1 drop.
빌림(&/&mut)은 drop 안 함(소유 아님). 순환참조 불가능(소유 트리) → leak 없음, refcount 없음.
```
panic/조기 return 경로에도 drop 보장(unwinding 또는 명시적 cleanup). secret 자원은 G.4.

## G.3 Region/arena fallback (General mode)
ownership를 정적으로 못 푸는 영역(예: 복잡한 aliasing) → region/arena로 일괄 할당·일괄 해제, **tagged**(`alloc-region`). 절대 미세누락 없음. Total mode는 region 불필요(소유 완결).

## G.4 Secret 메모리 (R6/R29/R36)
- **zeroize-on-drop:** secret 버퍼는 drop 시 zero로 덮어씀. 컴파일러가 최적화로 제거 못 하게(volatile write 또는 `zeroize` crate, MIT/Apache).
- **oblivious access:** secret 인덱싱/branch 금지(타입 차원 [SECRET-IDX]/[SECRET-IF]). 필요 시 constant-time select(전체 스캔 + 마스크).
- **누설 금지:** secret 값은 로그·에러·SMT 모델·디버그 덤프에 출력 안 함(R36) — 이름/타입만.
- **백엔드 가드:** secret_taint 영역에 data-oblivious 변환만(R29). timing 보장은 *명시된 추상화 수준*(IR/소스); 하드웨어 미세 타이밍·캐시는 범위 밖, 문서화(정직, DR3).

## G.5 Numeric 버퍼 레이아웃
- `Vec`/`Mat` 64B(cache line) 정렬, stride-1 우선.
- AoS→SoA(Layer B #7): struct 배열을 field 배열로 변환(metadata `layout_hint`); vectorize 전 적용.
- big `Mat`는 tiling(Layer B, isl dependence-legal)로 캐시 블로킹.

## G.6 ABI / calling convention
- JEFF fn → LLVM fn. 인자/반환은 타입별 표현(G.1). bignum은 포인터+메타.
- `total` fn: 순수+종료 → 자유 재배치/CSE/병렬화 안전(effect 없음).
- effect 있는 fn: effect row가 최적화 가능 범위를 제한(IO/Div 순서 보존).
- C interop은 `extern` + `Unsafe` effect로 명시(범위 밖 기본).

## G.7 동시성 (§13; 정직한 범위)
- **데이터 경합 없음:** `&mut` 배타성으로 정적 보장(ownership). 공유는 `&`(불변)만.
- **병렬 reduction:** associative `fold`/`sum`(C.6에서 assoc 증명됨) → divide-and-conquer 분할(third homomorphism theorem). span `O(N)→O(N/P + log N)`. associativity 미증명이면 순차.
- **정직한 한계:** 구조적 동시성 모델이지, "lock 제거 마법"이 아니다. lock-free 가속 류 과장 금지(R30). 게이지/대칭으로 동기화 소멸 류는 PART 18 non-goal.

---
---

# APPENDIX H — DIAGNOSTICS, COLLAPSE-REPORT, CERTIFICATE OUTPUT FORMATS

> DX(P5)의 권위. 진단·리포트는 사람 + 기계(JSON) 양쪽. secret 값 미출력(R36). 모든 항목 source span 보존(R37).

## H.1 Diagnostic 구조 & 렌더링
```rust
pub struct Diagnostic {
    pub severity: Severity,    // Error | Warning | Note
    pub code: DiagCode,        // 예: E0301 (secret-branch), E0401 (termination)
    pub span: Span,
    pub message: String,       // 무슨 일
    pub notes: Vec<String>,    // 왜
    pub help: Option<String>,  // 무엇을 할 수 있나
}
```
렌더링 예:
```
error[E0301]: data-dependent branch on secret value 'sk'
  ┌─ kyber/decap.jeff:42:9
  │
42│         if sk[0] == 0:
  │            ^^^^^^^^^^ branch condition depends on secret[u8]
  = note: constant-time is required here (#[constant_time]); a secret-dependent
          branch can leak via timing/cache (PART 3 R6).
  = help: rewrite data-obliviously, e.g. `let m = ct_eq(sk[0], 0); y = select(m, a, b)`.
```

## H.2 `--collapse-report` (per-function)
**텍스트:**
```
fn s2            collapsed  layer=1(arith/holonomic)  O(1)        cert=ok(Z3)
fn fib           collapsed  layer=1(eigen)            O(log n)    cert=ok(Z3)
fn lattice_count collapsed  layer=3b(barvinok)        O(1)        cert=ok(Z3:chamber+sample)
fn checksum      deferred   HONEST_DEFER[data-dependent-omega-N]  backend=SIMD(const-factor)
fn aes_round     partial    linear→collapsed; HONEST_DEFER[nonlinearity] at SubBytes
fn solve_sat     refused    HONEST_DEFER[np-hard]  (#[collapse] → error)
```
**JSON (기계가독):**
```json
{ "functions": [
  { "name":"s2","mode":"total","status":"collapsed","layer":"arith/holonomic",
    "cost":"O(1)","certificate":{"verified":true,"checker":"z3","kind":"PolynomialIdentity"} },
  { "name":"checksum","mode":"general","status":"deferred",
    "barrier":"data-dependent-omega-N","backend":["simd"],"speedup":{"kind":"constant-factor"} }
]}
```
규칙: `speedup`는 측정값만, kernel+N과 함께, non-uniform(R8/DR2). 추정·과장 금지.

## H.3 `--emit-certificates <dir>` 레이아웃
```
<dir>/
├─ manifest.json            # { build_hash, jeffc_version, functions:[{name, cert_files:[...]}] }
├─ s2.cert.json             # Certificate (PART 6.1 schema, serde)
├─ s2.smt2                  # Z3에 던진 실제 스크립트 (재현)
├─ fib.cert.json
├─ central.cert.json
├─ central.lean             # Lean 사용 시 증명 스크립트
└─ lattice.cert.json
```
`s2.cert.json` 예:
```json
{ "collapser_id":"arith/faulhaber",
  "source":{"node":1207,"span":"s2.jeff:3:5-3:28"},
  "collapsed":{"node":1240,"span":"s2.jeff:3:5"},
  "obligation":"forall n:nat. 6*sum_{i=0}^{n} i^2 == n*(n+1)*(2n+1)",
  "evidence":{"kind":"PolynomialIdentity","poly":"6*S(n)-n*(n+1)*(2n+1)"},
  "boundaries":["S(0)=0"],
  "verified":{"checker":"z3","result":"valid","script":"s2.smt2"},
  "fallback":{"node":1207} }
```
**재현성(R11):** 같은 입력+플래그 → 동일 manifest(cert 포함). `ci/determinism.sh`가 강제.

## H.4 `--const-time-audit` 리포트
```
fn poly_mul   OK    no secret-dependent branch/index/timing on secret[Vec[mod[3329],256]]
fn decap      FAIL  E0301 at decap.jeff:42 (branch on sk[0]); E0302 at :58 (index by secret)
```
FAIL이면 CI 실패(R6). secret 값 자체는 출력 안 함(R36).

## H.5 Diagnostic code 표 (발췌)
```
E01xx  parse/lex
E02xx  type mismatch / kind / arity
E0301  secret-dependent branch        (R6, [SECRET-IF])
E0302  secret-dependent index         (R6, [SECRET-IDX])
E0303  illegal declassify context     (R6)
E0401  termination not provable       (R34, Total mode)
E0402  guarded-corecursion violation  (R34)
E05xx  linearity (use-after-move / unconsumed linear) (R3/T3)
E0601  #[collapse] required but barrier hit  (PART 12 tag 첨부)
E07xx  refinement obligation failed (런타임 검사로 fallback; Warning 가능)
E0901  license violation (GPL/LGPL link)     (R5, CI)
W09xx  unrecognized attribute / deprecated
```

---
---

# APPENDIX I — STDLIB API REFERENCE (jeff-stdlib)

> language-spec §10 권위. `numeric`의 모든 함수는 *활용 구조*를 계약(`requires:`)으로 명시하고, 구조 부재 시 `constant-factor-only`로 강등 + 진단(R19). PQC는 전 구간 `secret[T]` + `@constant_time`(R28).

## I.1 `core`
```jeff
total fn len(v: &Vec[T, n]) -> nat
total fn get(v: &Vec[T, n], i: { k: nat | k < n }) -> &T          # 경계검사 타입에 박힘
fn map(v: own Vec[T, n], f: (T) -> U) -> Vec[U, n]
fn zip(a: own Vec[A, n], b: own Vec[B, n]) -> Vec[(A,B), n]
# Option / Result
data Option[T]: None ; Some(T)
data Result[T, E]: Ok(T) ; Err(E)
# mod[q] (NTT/PQC 기반)
total fn madd(a: mod[q], b: mod[q]) -> mod[q]
total fn mmul(a: mod[q], b: mod[q]) -> mod[q]
total fn mpow(a: mod[q], e: nat) -> mod[q]                        # O(log e), collapse
total fn minv(a: mod[q]) -> mod[q]                                # q 소수 가정
# bv[n] (GF(2))
total fn bxor(a: bv[n], b: bv[n]) -> bv[n]
total fn rotl(a: bv[n], k: nat) -> bv[n]
total fn popcount(a: bv[n]) -> nat
```

## I.2 `fold` (인식 + 검증된 reference 구현)
```jeff
total fn faulhaber(p: nat, n: nat) -> nat            # Σ_{i=0}^{n} i^p, closed (PolynomialIdentity)
total fn floor_sum(n: nat, a: int, b: int, m: nat) -> int   # Σ_{i=0}^{n-1} floor((a*i+b)/m) (AtCoder)
total fn matrix_power_mod(A: Mat[mod[q], d, d], e: nat) -> Mat[mod[q], d, d]   # O(log e)
total fn bostan_mori(rec: LinRec, n: nat, q: nat) -> mod[q]   # 선형점화 N-th term, O(M(d) log n)
# holonomic-recognized 합은 평범히 `sum k in D: F(n,k)` 로 작성 → recognizer가 collapse
```

## I.3 `numeric` (kernel pack; 구조 계약 R19)
```jeff
# --- 저차원/희소 선형대수 ---
fn randomized_svd(a: &Mat[f64, m, n], k: nat) -> Svd
    # requires: a is numerically low-rank.  else → constant-factor-only + 진단
fn lanczos(a: &Sparse[f64], k: nat) -> Krylov          # requires: symmetric sparse
fn gmres(a: &Sparse[f64], b: &Vec[f64, n]) -> Vec[f64, n]   # requires: sparse
fn sherman_morrison(ainv: &Mat[f64,n,n], u: &Vec[f64,n], v: &Vec[f64,n]) -> Mat[f64,n,n]
    # rank-1 update, O(n²)
fn rmt_denoise(a: &Mat[f64, m, n]) -> Mat[f64, m, n]   # Wigner/Marchenko-Pastur 마스킹
# --- N-body / 변환 ---
fn fmm(src: &[Charge], kern: Kernel) -> Vec[f64]       # requires: decaying kernel; O(N²)→O(N)
fn fft(x: &Vec[Complex, n]) -> Vec[Complex, n]
fn ntt(x: &Vec[mod[q], n]) -> Vec[mod[q], n]           # requires: q NTT-friendly ∧ n | (q-1); exact
fn wiener_khinchin(x: &Vec[f64, n]) -> Vec[f64, n]     # autocorrelation, O(N²)→O(N log N)
# --- 비선형 → 선형 / 동역학 ---
fn cole_hopf(burgers: Pde) -> HeatPde                  # exact linearization (Burgers only)
fn koopman_dmd(traj: &[State], obs: Observables) -> LinearModel   # requires: linearizable dynamics
fn riccati_schur(a: &Mat[f64,n,n], b: &Mat[f64,n,m],
                 q: &Mat[f64,n,n], r: &Mat[f64,m,m]) -> Mat[f64,n,n]   # LQR via Hamiltonian Schur
# --- 수렴/급수 가속 ---
fn anderson(g: (&Vec[f64,n]) -> Vec[f64,n], m: nat) -> FixedPoint
fn shanks(seq: &[f64]) -> f64
fn pade(series: &[rat], l: nat, m: nat) -> RatFn
fn chebyshev_filter(a: &Sparse[f64], deg: nat) -> Vec[f64]   # 스펙트럼 필터
# --- 최적수송/희소복원 ---
fn sinkhorn(cost: &Mat[f64, m, n], eps: f64) -> Transport
fn l1_min(a: &Mat[f64, m, n], y: &Vec[f64, m]) -> Vec[f64, n]   # requires: sparse solution
# --- 적분기 ---
fn symplectic_step(q: &Vec[f64,d], p: &Vec[f64,d], h: f64, grad_v: ...) -> (Vec[f64,d], Vec[f64,d])
```
규칙: 위 계약(`requires:`)을 정적/동적으로 검사. 충족 시 점근 가속(certificate); 미충족 시 `constant-factor-only`로 강등하고 *조용히 거짓 약속하지 않는다*(R19/DR2).

## I.4 `crypto.pqc` (secret 전 구간 + @constant_time; R28)
```jeff
@constant_time fn ml_kem_keygen(seed: secret[Vec[u8, 32]])
    -> (PublicKey, secret[SecretKey])
@constant_time fn ml_kem_encaps(pk: &PublicKey, coins: secret[Vec[u8, 32]])
    -> (Ciphertext, secret[SharedKey])
@constant_time fn ml_kem_decaps(sk: &secret[SecretKey], ct: &Ciphertext)
    -> secret[SharedKey]
@constant_time fn ml_dsa_sign(sk: &secret[SigningKey], msg: &[u8]) -> Signature
fn               ml_dsa_verify(pk: &PublicKey, msg: &[u8], sig: &Signature) -> bool
# 내부 poly_mul over mod[3329]은 ntt()로 Layer 1 exact collapse.
# 모든 secret 경로는 const-time-audit 통과(H.4) — 미통과 시 CI 실패(R6).
```

## I.5 `proof` & `poly`
```jeff
# proof
pub use jeff_cert::{ Certificate, Evidence, VerifiedCertificate }
fn verify(c: Certificate) -> Option[VerifiedCertificate]      # Valid만 Some (R31)
# poly (Barvinok 프런트엔드)
fn affine_domain(cs: &[Constraint]) -> Option[IslSet]         # affine이면 Some, else None→non-affine-domain
fn count(domain: &IslSet) -> Option[QuasiPolynomial]          # Layer 3b
```

---
---

# APPENDIX J — WORKED COMPILATION TRACES (stage by stage)

> 파이프라인(PART 8)이 실제로 무엇을 하는지의 end-to-end 트레이스. 디버깅·테스트 기대치의 기준. 모든 경계 fallback(R1).

## J.1 Faulhaber — 완전 collapse
```
SOURCE
  total fn s2(n: nat) -> nat: sum i in 0..=n: i*i

AST            FnDecl{mode:Total, name:s2, params:[(n,nat)], ret:nat,
                 body: Reduction{Sum, [i], Range(0,n,incl), Bin(*, Var i, Var i)}}
TYPE/EFFECT    n:nat ⊢ body : nat ! ∅ ;  total (Div,IO ∉ ε) ;  termination: n/a (no recursion)
ABSINT         affine_domain = {0<=i<=n} ;  no unbounded accumulation
RECOGNIZE      egglog: power-sum 패턴 매치 → DispatchTag::AffineTripCount (∨ Holonomic)
               cost: 원본 Linear, 후보 Const
COLLAPSE(L1)   arith/faulhaber → 후보 n*(n+1)*(2n+1)/6
CERTIFICATE    PolynomialIdentity( 6*S(n) − n*(n+1)*(2n+1) ≡ 0, S(0)=0 )
VERIFY         Z3 (F.1): unsat,unsat → Valid → VerifiedCertificate
JLIR           Origin::Collapsed{layer:1, cert:#…} ; residual = closed form ; (백엔드 inert)
BACKEND        no-op on residual (이미 O(1))
LLVM           정수 산술: n*(n+1)*(2n+1)/6  (bignum nat)
REPORT         "s2: collapsed layer=1 O(1) cert=ok(Z3)"
```

## J.2 FNV checksum — defer + 백엔드 상수배
```
SOURCE
  fn checksum(xs: &[u8]) -> u64:
      var h: u64 = 1469598103934665603
      for x in xs: h = (h ^ x) * 1099511628211

TYPE/EFFECT    general (var/for) ; effect {} (순수하지만 General)
ABSINT         loop-carried dep on h ;  is_unbounded_accumulation(h) = true
RECOGNIZE      DispatchTag::None  (FNV는 비선형 누적; closed form 없음)
COLLAPSE       모든 collapser entry=false 또는 Defer
DETECT         data-dependent-omega-N
JLIR           Origin::Deferred{tag} ; 원본 루프 보존 ; secret_taint = ∅
BACKEND        B-loop: dependence-legal? (직렬 carry) → vectorize 불가;
               B-mid: bit-slice/SIMD on x 로드 + FMA로 상수배;  Θ(N) 유지
LLVM           최적화된 직렬 루프 + SIMD 로드
REPORT         "checksum: HONEST_DEFER[data-dependent-omega-N]; backend=SIMD(const-factor)"
```

## J.3 PQC poly_mul — NTT collapse + secret-taint
```
SOURCE
  @constant_time
  fn poly_mul(a: secret[Vec[mod[3329],256]], b: Vec[mod[3329],256]) -> secret[Vec[mod[3329],256]]:
      intt(ntt(a) .* ntt(b))

TYPE/EFFECT    a: secret[Vec[mod[3329],256]] ;  secret-taint 전파 검사:
               분기/인덱스 없음 → [SECRET-*] 위반 없음 ✓
RECOGNIZE      convolution(ntt/intt) 인식 → DispatchTag::Convolution
COLLAPSE(L1)   numeric/ntt: exact over mod[3329] (256 | 3328) → O(n log n)
CERTIFICATE    NumericResidual(exact mod) — 작은 입력 직접 합성곱과 일치
VERIFY         ReplayChecker: Valid
JLIR           Origin::Collapsed{layer:1} ; secret_taint = {a 유래 SSA 전부}
BACKEND        secret_taint 영역: data-oblivious만 (R29) — secret addressing 변환 금지
CONST-TIME     const-time-audit: OK (분기/인덱스 secret 의존 없음)  (H.4)
LLVM           NTT butterfly (const-time), Montgomery mul
REPORT         "poly_mul: collapsed layer=1 O(n log n) cert=ok ; const-time=OK"
```

## J.4 SAT-as-Ising — 거부
```
SOURCE
  @collapse fn solve(phi: CNF) -> Assignment: argmin_ising(encode(phi))

RECOGNIZE      목적이 Ising ground state로 환원 (Lucas 2014)
DETECT         np-hard
@collapse      요구됨 → COMPILE ERROR (E0601):
               "#[collapse] required but HONEST_DEFER[np-hard]: Ising ground state is
                NP-hard (Lucas 2014). No polynomial collapse exists. Remove #[collapse]."
(주: encode/argmin은 작성 가능하나 — 지수 비용으로 실행될 뿐 — collapse는 거부. P3/P4.)
```

---
---

# APPENDIX K — TEST HARNESS TEMPLATES & FULL CI SCRIPTS

> PART 14·15 구현. 새 collapser/커널은 이 템플릿들을 채운다(R3). secret 미누설(R36).

## K.1 Rust 테스트 템플릿
```rust
// (1) unit
#[test] fn faulhaber_closed_form() {
    let out = compile("total fn s2(n: nat)->nat: sum i in 0..=n: i*i").unwrap();
    assert_eq!(eval(&out, &[("n", 3)]), 14); // 0+1+4+9
}

// (2) property: collapse ≡ original (DR7)
proptest! {
    #[test] fn collapse_equiv(n in 0u64..2000) {
        let collapsed = run_collapsed("sum i in 0..=n: i*i", n);
        let naive     = (0..=n).map(|i| i*i).sum::<u128>();
        prop_assert_eq!(collapsed, naive);   // 단 1개라도 다르면 실패 (R7/DR7)
    }
}

// (3) certificate-replay (R25)
#[test] fn cert_replays() {
    let art = compile_with_certs("total fn s2(n: nat)->nat: sum i in 0..=n: i*i");
    for c in art.certificates() {
        assert_eq!(jeff_verify::verify(c.clone().into_unverified()),
                   Some(/* re-verified */), "cert {} failed independent replay", c.id());
    }
}

// (4) fallback: 검증 강제 실패 → 원본 (R1/R31)
#[test] fn fallback_on_verify_failure() {
    let art = compile_with(FORCE_VERIFY_FAILURE, "total fn s2(n: nat)->nat: sum i in 0..=n: i*i");
    assert!(art.region("s2").origin.is_deferred_or_original());
    assert_eq!(eval(&art, &[("n", 100)]), (0u128..=100).map(|i| i*i).sum());
}

// (5) golden (R25): 입력→collapsed form→cert 고정
#[test] fn golden_s2() { assert_golden("golden/s2.txt", compile_report("…")); }

// (6) negative: defer + 정확한 tag (PART 12)
#[test] fn defers_data_dependent() {
    let r = compile_report("fn c(xs: &[u8])->u64 { var h:u64=0; for x in xs: h=(h^x)*3 }");
    assert_eq!(r.barrier("c"), Some(BarrierTag::DataDependentOmegaN));
}

// (7) const-time (R28): timing-class 동치 / secret 분기 거부
#[test] fn rejects_secret_branch() {
    let e = compile("@constant_time fn d(sk: secret[Vec[u8,4]])->u8: if sk[0]==0 { 1 } else { 0 }");
    assert!(matches!(e, Err(ds) if ds.iter().any(|d| d.code == DiagCode::E0301)));
}

// (8) differential (dev-only, out-of-process oracle; R5)
#[cfg(feature = "oracles")]
#[test] fn diff_against_sage() {
    let ours = run_collapsed("sum k in 0..=n: C(n,k)*C(n,k)", 7);
    let sage = jeff_test_oracles::sage_sum("sum(binomial(7,k)^2, k, 0, 7)").unwrap();
    assert_eq!(ours.to_string(), sage); // 20… ; C(14,7)=3432
}
```

## K.2 CI 스크립트 (full)
```bash
# ci/run.sh — 모든 게이트 (PART 15)
set -euo pipefail
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test  --workspace
cargo test  --workspace --features proptest
./ci/license_scan.sh
./ci/const_time_audit.sh
./ci/cert_replay.sh
./ci/determinism.sh
./ci/bench_honesty.sh
./ci/coverage.sh --min-line 0.85 --min-branch 0.75
echo "ALL GATES GREEN"
```
```bash
# ci/license_scan.sh — R5: GPL/LGPL 링크 0
set -euo pipefail
# cargo-deny(권장): deny.toml 에 GPL-*, LGPL-* 를 deny.
cargo deny check licenses
# 금지 크레이트가 release graph에 없는지 직접 확인 (dev-deps의 jeff-test-oracles는 예외)
FORBIDDEN="barvinok latte ppl gmp-mpfr-sys rug m4ri"
for c in $FORBIDDEN; do
  if cargo tree -e no-dev -i "$c" >/dev/null 2>&1; then
    echo "error[license]: forbidden crate '$c' in non-dev dependency graph (R5)"; exit 1
  fi
done
echo "license-scan OK"
```
```bash
# ci/const_time_audit.sh — R6
set -euo pipefail
for f in $(git ls-files '*.jeff' | grep -E 'crypto|pqc|secret'); do
  jeffc build "$f" --const-time-audit || { echo "const-time FAIL in $f (R6)"; exit 1; }
done
echo "const-time-audit OK"
```
```bash
# ci/cert_replay.sh — R25
set -euo pipefail
rm -rf /tmp/jeffcerts && mkdir -p /tmp/jeffcerts
for f in $(git ls-files 'tests/**/*.jeff'); do
  jeffc build "$f" --emit-certificates /tmp/jeffcerts/$(basename "$f")
done
cargo run -p jeff-verify --bin replay -- /tmp/jeffcerts   # 모든 cert 독립 재검증
echo "cert-replay OK"
```
```bash
# ci/determinism.sh — R11
set -euo pipefail
F=tests/e2e/triangular.jeff
jeffc build "$F" --emit-certificates /tmp/o1 >/tmp/log1 2>&1
jeffc build "$F" --emit-certificates /tmp/o2 >/tmp/log2 2>&1
diff -r /tmp/o1 /tmp/o2 || { echo "non-deterministic output (R11)"; exit 1; }
echo "determinism OK"
```
```bash
# ci/bench_honesty.sh — R8: 벤치마크 수치 하드코딩 금지
set -euo pipefail
# criterion 결과는 측정에서만; 소스에 하드코딩된 "Nx faster" 류 금지
if grep -REn '([0-9]{2,}) *[x×] *(faster|speedup)' benches/ src/ 2>/dev/null; then
  echo "error: hardcoded speedup claim (R8/R30)"; exit 1
fi
echo "bench-honesty OK"
```
```toml
# deny.toml (발췌)
[licenses]
allow = ["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "Apache-2.0 WITH LLVM-exception"]
deny  = ["GPL-2.0", "GPL-3.0", "LGPL-2.1", "LGPL-3.0", "AGPL-3.0"]
```

---
---

# APPENDIX L — CONCRETE PER-STAGE TASK TICKETS

> PART 9의 각 Stage를 실행 가능한 티켓으로 분해. Claude Code는 하나씩 집어, 작은 검증된 증분으로 처리하고(R13), 2.4 형식으로 보고한다. 각 티켓은 DoD(PART 17) 통과해야 닫힌다. `dep:` = 선행 티켓.

## Stage 0 — 검증 spine
```
T0.1 workspace scaffold (crates/ 전부 stub, Cargo.toml, docs/ 3 spec 배치)         dep: —
T0.2 jeff-cert: Certificate/Evidence/Collapsed/VerifiedCertificate (type-level gate) dep: T0.1
     accept: VerifiedCertificate 없이 Collapsed 생성 *불가능*(컴파일 실패) 테스트
T0.3 jeff-verify: Z3Checker (PolynomialIdentity, Gf2LinearIdentity) + timeout+fallback dep: T0.2
     accept: F.1/F.3 스크립트가 Valid; 강제 Unknown → None (R31)
T0.4 jeff-jlir + jeff-codegen: fallback harness (verify 실패→원본 codegen)           dep: T0.2,T0.3
     accept: hang/miscompile 없음; 강제 실패 시 원본 실행 일치
T0.5 ci/: build,clippy,test,license_scan,determinism (APPENDIX K.2)                  dep: T0.1
     accept: 빈 프로젝트에서 전 게이트 green
T0.6 e2e: triangular parse→recognize(stub)→collapse(stub)→verify→codegen→run        dep: T0.2..T0.5
     accept: collapse 경로 + fallback 경로 둘 다 테스트 (J.1 흐름)
T0.7 테스트 스켈레톤 (K.1 (1)(3)(4)(5))                                              dep: T0.6
```
**Stage 0 exit:** PART 9 Stage 0 DoD.

## Stage 1 — Holonomic
```
T1.1 hypergeometric term ratio 추출 (t_{k+1}/t_k ∈ ℚ(k) 판정)                       dep: Stage0
T1.2 Gosper–Petkovšek normal form (resultant로 정수근 peel → (a,b,c))               dep: T1.1
T1.3 Gosper degree bound (E.1 공식; da≠db / da=db & A≠B / A=B)                      dep: T1.2
T1.4 Gosper 방정식 다항 선형계 solve (미정계수)                                      dep: T1.3
T1.5 Gosper certificate(PolynomialIdentity) + checker 우선 구현+테스트 (R2)          dep: T0.3,T1.4
T1.6 Zeilberger: parametrized Gosper, J 증가 루프 (E.2)                              dep: T1.4
T1.7 Telescoper certificate(poly identity→Z3; Lean stub) + boundary 기록            dep: T1.5,T1.6
T1.8 fixtures A09–A12 positive (telescoper)                                          dep: T1.7
T1.9 non-Gosper defer 경로 + fixtures A16–A17 negative                              dep: T1.5
T1.10 CR 코어 통합(기존 엔진 재사용, 44/44 회귀 스위트 편입)                          dep: T0.6
```

## Stage 2 — Recognizer
```
T2.1 egglog e-graph build/saturate(budget)/extract 골격                             dep: Stage1
T2.2 ruleset: CR, Faulhaber, floor_sum 상호성, residue_split                         dep: T2.1
T2.3 ruleset: holonomic closure(sum/product/shift), cole_hopf rule                   dep: T2.1,T1.7
T2.4 asymptotic cost model + classify → DispatchTag (6.3)                            dep: T2.1
T2.5 dispatch 결정 절차(PART 7) — tag→collapser 라우팅                               dep: T2.4
T2.6 fixtures A01–A20, B*, G* 가 올바른 tag/collapser로 라우팅                       dep: T2.5
```

## Stage 3 — Kernel pack + absint
```
T3.1 absint: intervals, congruences (+ is_unbounded_accumulation, 12 detection)      dep: Stage2
T3.2 absint: polyhedra(isl) → dependence(1회 생성) + affine_domain                   dep: T3.1
T3.3 eigen/Jordan/matrix-exp + EigenCharpoly cert (F.5)                              dep: T1.5
T3.4 Bostan–Mori (E.4) + NumericResidual cert                                        dep: T3.3
T3.5 FFT/NTT/Walsh–Hadamard (exact mod q)                                            dep: Stage2
T3.6 (B) 커널: randomized_svd/krylov/fmm — 구조 계약 + 강등(R19)                      dep: T3.2
T3.7 (B) 커널: cole_hopf/koopman_dmd/riccati_schur/anderson/shanks/padé              dep: T3.6
T3.8 fixtures A05–A08,A13–A15 + 커널 정확/오차/강등 테스트                           dep: T3.4,T3.6
```

## Stage 4 — Barvinok
```
T4.1 isl 빌드(--with-int=imath) + 프런트엔드(affine_domain→IslSet)                   dep: T3.2
T4.2 clean-room cone decomposition(Brion/signed unimodular)                          dep: T4.1
     accept: GPL barvinok 미링크(license_scan green)
T4.3 short rational generating functions → quasi-polynomial                          dep: T4.2
T4.4 certificate(chamber Presburger + sample) (F.4)                                  dep: T4.3,T0.3
T4.5 fixtures B01–B07 (positive + non-affine-domain/sharp-P-hard)                    dep: T4.4
```

## Stage 5 — GF(2) + PQC
```
T5.1 BitCircuit IR + partition(2-input nonlinear에서 cut) (E.3)                      dep: Stage2
T5.2 (M,b) 누적 + clean-room Four-Russians(M4RM, Gray-code)                          dep: T5.1
     accept: M4RI 미링크
T5.3 Gf2LinearIdentity cert(basis) + checker (F.3)                                   dep: T5.2,T0.3
T5.4 nonlinearity detect + defer; fixtures G01–G06                                   dep: T5.3
T5.5 mod[q] Montgomery + ntt() exact; poly_mul collapse                              dep: T3.5
T5.6 secret-taint 전파(C.4) + const-time-audit(H.4) + ml_kem/ml_dsa skeleton         dep: T5.5
     accept: P01 green, P02/P03 컴파일 에러(E0301/E0302)
```

## Stage 6 — Backend Layer B
```
T6.1 JLIR metadata envelope 완성(6.5) + invariant assert(R18)                        dep: Stage3
T6.2 B-cf: SMT branch fold(Z3 재사용; decidable만)                                   dep: T0.3,T6.1
T6.3 B-loop: isl tiling/vectorize(dependence-legal; secret addressing 금지)          dep: T3.2,T6.1
T6.4 B-mid: devirt(call_targets), AoS→SoA(layout_hint), lifetime→free(G.2)           dep: T6.1
T6.5 B-pre: comptime PE(const_inputs)                                                dep: T6.1
T6.6 LLVM finish(codegen) + PGO 가중치 → cost model 파라미터화                        dep: T6.2..T6.5
T6.7 secret_taint 가드 테스트(R29) + collapsed residual inert 테스트                 dep: T6.3
```

## Stage 7 — Tensor-network
```
T7.1 TensorNetwork IR + treewidth 추정(min-degree/min-fill) (E.6)                    dep: Stage3
T7.2 contraction ordering 휴리스틱 + treewidth-blowup defer                          dep: T7.1
T7.3 slicing fallback(시간↔메모리)                                                   dep: T7.2
T7.4 NumericResidual cert(exact ring replay); fixtures T01–T03                       dep: T7.3,T0.3
```

## Stage 8 — Holographic + Gröbner
```
T8.1 planarity 검사 + non-planar defer                                               dep: Stage4
T8.2 matchgrid + FKT Pfaffian orientation (E.5)                                      dep: T8.1
T8.3 Pfaffian/Holant + PfaffianHolant cert(replay); fixtures H01–H03                 dep: T8.2,T0.3
T8.4 Gröbner(Buchberger) budget-gated + groebner-blowup                              dep: Stage3
```

## 진행 규율
- 한 번에 한 티켓. 닫기 전 DoD(PART 17). 막히면 2.4 형식 보고(가짜 금지, DR5).
- 각 티켓 PR은 전 CI 게이트 green(PART 15) + spec 인용(R22).
- 의심되면 멈추고 물어라. defer는 실패가 아니라 정직이다.

---
---

# APPENDIX M — DESIGN RATIONALE / DECISION RECORDS

> 각 주요 결정의 *왜*. 명시되지 않은 상황에서 Claude Code가 이 근거에 맞춰 일관되게 선택하도록. 형식: 결정 / 근거 / 기각한 대안.

**D1 — 구현 언어 Rust.**
근거: egg/egglog(recognizer)가 Rust+MIT → 재사용; 메모리 안전·강타입은 컴파일러 정확성에 유리; cargo workspace로 크레이트 격리; LLVM 바인딩 성숙.
기각: C++(메모리 안전 약함), OCaml(LLVM/egglog 생태계 약함), Haskell(systems 통합·성능 예측성 약함).

**D2 — 이중 모드(Total/General).**
근거: Rice를 *우회*할 수 없으므로(보존법칙), Turing-complete를 *벗어난* sub-Turing Total mode에서 termination을 결정 가능한 타입 속성으로 만든다(Agda/Idris/Coq 노선). 검증 커널은 Total, orchestration은 General.
기각: 전부 General(종료 미보장 → collapse 안전성 약화); 전부 dependent(표현력↑이나 결정성·진입장벽 문제).

**D3 — Proof-carrying collapse (per-instance certificate).**
근거: 전역 컴파일러 정확성 증명(CompCert식)은 거대·경직. JEFF는 *매 collapse 인스턴스*가 Z3/Lean으로 검증되는 certificate를 들고 다니고, 실패 시 fallback. "never-miscompile"을 전역 메타정리 없이 달성.
기각: verified-compiler-once(범위가 collapse 변환에 안 맞음); 무검증 best-effort(P0/P1 위반).

**D4 — 우선순위 P0 never-miscompile > P1 honesty > P2 proof > P3 conservation > P4 perf.**
근거: 컴파일러의 1차 계약은 정확성. JEFF의 차별점은 정직성(honesty가 가치 제안). 성능은 그 위에서만 의미. 충돌 시 항상 낮은 번호 승(PART 1.4).
기각: perf-first(틀리거나 거짓이면 무가치); honesty와 correctness 동급(불명확 — correctness가 더 근본).

**D5 — Recognizer = egg/egglog e-graph.**
근거: equality saturation으로 정규형·다중 rewrite를 원칙적으로; cost extraction으로 점근 최적 선택; MIT.
기각: ad-hoc 패턴 매칭(확장성·정규성 약함, 기존 엔진의 한계였음).

**D6 — Z3 기본 + Lean(holonomic operator induction).**
근거: 대부분 certificate는 다항/비트벡터 항등식 → Z3가 빠르고 충분. holonomic operator induction은 SMT 너머 → Lean. 둘 다 permissive(MIT/Apache).
기각: 전부 Lean(느림·자동화 약함); 전부 Z3(induction 한계).

**D7 — isl + clean-room Barvinok (GPL barvinok 링크 안 함).**
근거: isl은 MIT, `--with-int=imath`로 GMP(LGPL) 회피. counting(3b)과 scheduling(Layer B)이 같은 isl 공유. Barvinok decomposition은 알고리즘이 공개 → clean-room.
기각: GPL barvinok 링크(R5 위반 — 배포 산출물 오염).

**D8 — Four-Russians clean-room (M4RI 링크 안 함).**
근거: M4RI는 GPL. Gray-code 테이블 알고리즘은 공개 → 직접 구현.
기각: M4RI 링크(R5).

**D9 — bignum = num-bigint (GMP/rug 안 씀).**
근거: `int/nat/rat`는 정확 산술 필요. num-bigint/num-rational은 MIT/Apache. GMP·`rug`는 LGPL → R5 위반.
기각: GMP/rug(라이선스).

**D10 — secret-taint = 타입 modality(`secret[T]`).**
근거: 타입 차원에서 data-dependent branch/index를 *컴파일 에러*로(C.4). lint/annotation보다 강건 — 우회 불가, 백엔드까지 전파(R29).
기각: lint(우회 가능); 런타임 검사(타이밍 누설 못 막음).

**D11 — linear/affine 타입으로 GC 제거.**
근거: ownership로 수명 종료점 정적 계산 → free 주입. 런타임 GC/refcount 없음(zero-cost; PQC 타이밍 예측성에도 유리).
기각: GC(타이밍 비결정·일시정지), refcount(원자연산 비용·순환).

**D12 — Layer 분해(0 recognizer / 1 arith+holonomic / 2 GF(2) / 3 holographic / 3b Barvinok / 4 tensor / B backend).**
근거: 각 layer가 *정리적으로 명확한 구조 클래스*에 대응(holonomic, affine count, GF(2)-linear, planar #CSP, bounded treewidth). 경계가 정리(Gosper/Barvinok/Cai–Lu/Markov–Shi)로 정의되어 barrier가 정직.
기각: 단일 만능 collapser(경계 불명확 → over-claim 위험); layer 무한 증식(중복).

**D13 — HONEST_DEFER 1급.**
근거: 정직성이 가치 제안. "왜 안 되는지"를 named tag + 근거 정리로(PART 12). 사용자가 구조를 바꿔 collapse를 얻거나, 한계를 이해.
기각: silent best-effort(P1 위반 — 거짓 인상).

**D14 — Python식 표면 문법.**
근거: 진입장벽↓, 가독성↑(P5). 단 타입 규율은 엄격(점진적→strict).
기각: ML/Lisp 문법(대상 사용자 친숙도 낮음).

**D15 — effect rows(Koka/Frank).**
근거: `total` 판정(IO/Div 부재)과 최적화 안전성을 타입으로. row는 조합 용이.
기각: monad transformer(무거움), effect 무시(total 판정 불가).

**D16 — JLIR = collapsed residual + deferred loop 공통 SSA.**
근거: 백엔드가 둘 다 같은 IR에서 처리 → universal. metadata(dependence/secret_taint/…)가 collapse와 backend를 잇는다.
기각: 분리 IR(중복·정합성 비용).

**D17 — physics-fantasy tier 거부(PART 18).**
근거: 보존법칙(Ω(N)/Rice/NP/#P/BBBV). 채택 대 거부 경계 = "구조 활용" 대 "구조 없는 불가능 주장". 이게 JEFF의 정체성이자 신뢰의 근거.
기각: "기하학적 O(1)"·CTC·비선형-QM-on-classical·Ising-마법(전부 보존법칙 위반 또는 물리 반사실).

---
---

# APPENDIX N — COMPLETE WORKED EXAMPLE PROGRAMS

> 언어를 end-to-end로 보여주는 주석 달린 프로그램. GACC가 각 부분을 어떻게 처리하는지(`# →`) 명시. 테스트 fixture 겸 문서.

## N.1 검증된 수치 커널 (collapse + refinement)
```jeff
module examples.numerics
import core.fold (faulhaber)

# 다항 합: recognizer가 Faulhaber로 인식 → closed form + Z3 certificate
total fn power_sum(p: nat, n: nat) -> nat:
    sum i in 0..=n: i**p
# → Layer 1: p가 작은 상수면 closed (PolynomialIdentity); 일반 p는 holonomic/CR

# refinement: 분모 0 아님이 타입에 박혀 런타임 검사 제거
total fn safe_mean(xs: &Vec[rat, n], n_pos: { k: nat | k > 0 }) -> rat:
    let total: rat = sum i in 0..n: *get(xs, /* i < n 증명됨 */ i)
    total / (n_pos as rat)            # → 분모>0 (REFINE-SUB, Z3) → 검사 없음

# 선형 점화: eigen/Bostan-Mori로 O(log n) collapse
total fn fib(n: nat) -> nat:
    match n:
        0 => 0
        1 => 1
        m+2 => fib(m+1) + fib(m)      # → recognizer: linear-state-transition → matrix power, EigenCharpoly cert
```

## N.2 PQC: secret-taint + constant-time (collapse where exact)
```jeff
module examples.kyber
import core (mod, Vec)

# 전 구간 secret + @constant_time. NTT 곱은 Layer 1 exact collapse.
@constant_time
fn poly_mul(a: secret[Vec[mod[3329], 256]],
            b: secret[Vec[mod[3329], 256]]) -> secret[Vec[mod[3329], 256]]:
    let na = ntt(a)                    # → numeric.ntt, exact over mod[3329] (256 | 3328)
    let nb = ntt(b)
    intt(pointwise_mul(na, nb))        # → O(n log n) collapse; const-time-audit OK

# ✗ 컴파일 에러 예시 (주석 처리) — secret 분기/인덱스
# fn leaky(sk: secret[Vec[u8, 32]]) -> u8:
#     if sk[0] == 0: 1 else: 0         # E0301 [SECRET-IF] (R6)
#     table[sk[1]]                     # E0302 [SECRET-IDX] (R6)
```

## N.3 Total mode: 구조적 재귀 + 종료 보장
```jeff
module examples.total

data Tree[T]:
    Leaf(T)
    Node(Tree[T], Tree[T])

# structural recursion → 종료 자동 증명 (TERM-STRUCT)
total fn tree_sum(t: &Tree[int]) -> int:
    match t:
        Leaf(x) => x
        Node(l, r) => tree_sum(l) + tree_sum(r)   # 인자가 구조적 부분항

# guarded corecursion (codata) — productivity
codata Stream[T]:
    head: T
    tail: Stream[T]

total fn nats_from(n: nat) -> Stream[nat]:
    Stream { head: n, tail: nats_from(n+1) }      # corecursive call이 constructor 아래 (TERM-GUARD)
```

## N.4 혼합: collapse + 정직한 defer (한 프로그램에서)
```jeff
module examples.mixed

# 이 부분은 collapse: affine count → Barvinok closed form
total fn count_pairs(n: nat) -> nat:
    count (i, j) in {0 <= i <= j <= n}: 1         # → (n+1)(n+2)/2 (Layer 3b cert)

# 이 부분은 defer: 데이터 의존 누적 → Ω(N), 백엔드 상수배
fn rolling_hash(xs: &[u8]) -> u64:
    var h: u64 = 0
    for x in xs:
        h = (h * 257 + (x as u64)) % 1000000007    # → HONEST_DEFER[data-dependent-omega-N]
                                                    #   backend: SIMD 상수배 (Θ(N) 유지)

# 이 부분은 거부됨 (주석) — NP-hard
# @collapse fn solve(c: CNF) -> Assignment:
#     argmin_ising(encode(c))                       # E0601: HONEST_DEFER[np-hard]
```
**N.4 collapse-report 기대치:**
```
count_pairs   collapsed  layer=3b(barvinok)  O(1)        cert=ok(Z3:chamber+sample)
rolling_hash  deferred   HONEST_DEFER[data-dependent-omega-N]  backend=SIMD(const-factor)
```

---
---

# APPENDIX O — FOUNDATIONAL BIBLIOGRAPHY (각 layer 근거)

> Claude Code가 막히면 *여기로* 가라. 각 기법의 1차 문헌. 이 문서의 모든 정리적 주장은 아래에 근거한다 — 추측이 아니라 인용(R30). 정확한 페이지·판본은 원문 확인; 아래는 저자·발표처·핵심 결과 수준.

**CR / 폴드엔진 (Layer 1 코어)**
- Bachmann, Wang, Zima — "Chains of Recurrences," ISSAC 1994. (CR 대수·닫힌형.)
- van Engelen — "Symbolic Evaluation of Chains of Recurrences for Loop Optimization," CC 2001.

**Holonomic summation (Layer 1)**
- Gosper — "Decision procedure for indefinite hypergeometric summation," PNAS 1978.
- Zeilberger — "The method of creative telescoping," J. Symbolic Computation 1991.
- Petkovšek, Wilf, Zeilberger — *A = B*, 1996. (Gosper/Zeilberger 표준 레퍼런스, degree bound — APPENDIX E 근거.)
- Chyzak — holonomic systems / 일반화된 creative telescoping.

**Lattice counting (Layer 3b)**
- Barvinok — "A polynomial time algorithm for counting integral points in polyhedra," Math. of OR 1994. (고정차원 다항시간.)
- Ehrhart — quasi-polynomial 이론 (parametric counting).
- Brion — vertex cone 생성함수 분해 (APPENDIX E.8 근거).

**Holographic / Pfaffian (Layer 3)**
- Valiant — "The complexity of computing the permanent," TCS 1979. (#P-completeness.)
- Valiant — "Holographic Algorithms," SIAM J. Computing 2008. (matchgate/Holant.)
- Kasteleyn / Fisher–Temperley (FKT) — planar perfect matching via Pfaffian.
- Cai, Lu — matchgate/Holant dichotomy (planar #CSP 정확 포착, 일반 #P-hard).

**Tensor network (Layer 4)**
- Markov, Shi — "Simulating quantum computation by contracting tensor networks," SIAM J. Computing 2008. (비용 = treewidth.)

**GF(2) (Layer 2)**
- Arlazarov, Dinic, Kronrod, Faradzhev — Method of Four Russians. (O(n³/log n) boolean matmul — APPENDIX E.6 근거.)
- AES S-box affine layer가 linearization 방어 — 설계 의도 (표준 cryptanalysis 문헌).

**복잡도 벽 / 거부 근거 (PART 18)**
- Bennett, Bernstein, Brassard, Vazirani (BBBV) — "Strengths and weaknesses of quantum computing," SIAM J. Computing 1997. (√N lower bound.)
- Lucas — "Ising formulations of many NP problems," Frontiers in Physics 2014. (Karp 21 → Ising; ground state NP-hard.)
- Mayr, Meyer — ideal membership EXPSPACE-complete. (Gröbner 최악.)
- Aaronson — "Read the fine print," Nature Physics 2015. (양자 선형대수 caveat.)
- Abrams, Lloyd — nonlinear QM이 NP를 푼다 (단 *물리적* 비선형 QM 필요; 실험적으로 배제 → PART 18 #4 근거).
- Toda — PH ⊆ P^#P. (#P의 힘.)

**PQC / side-channel**
- IACR ePrint 2023/1866 — NTT side-channel (APPENDIX E.10 근거).
- RFC 9106 — Argon2 (memory-hardness; collapse 불가가 *설계 목적* → `memory-hard`).
- Gidney, Ekerå 2021 / Gidney 2025 — Shor 자원 추정 (PQC 동기).

**언어/타입/검증 기반**
- Cousot, Cousot — Abstract Interpretation, POPL 1977. (sound over-approximation; Rice 우회 아님.)
- Rice (1953); Turing (1936). (undecidability 벽.)
- Abel — sized types; Agda/Idris/Coq — total functional programming (종료성 by typing → PART 7.7 / APPENDIX C 근거).

---
---

# APPENDIX P — METATHEORY & EDGE CASES

> "진짜 끝까지 판" 프롬프트의 마지막 1%: 정리의 정확한 진술과, 사람들이 빠뜨리는 엣지케이스. 여기를 안 지키면 P0가 조용히 깨진다.

## P.1 Collapse-Transparency — 정확한 진술 + 증명 의무
**정리.** 커널 K (Total mode, 닫힌 의미 ⟦K⟧)가 collapse(K)=K'를 내고 verify(cert(K,K'))=Valid이면, 모든 유효 입력 x에서 `⟦K⟧(x) = ⟦K'⟧(x)` (관측 동치: 반환값 ∧ effect trace ∧ secret 경로 timing class).
**증명 의무 (각 collapser가 책임).**
1. **soundness of evidence:** evidence가 주장하는 항등식이 `⟦K⟧=⟦K'⟧`를 *함의*함 (checker가 검증).
2. **completeness of boundary:** 경계조건(domain 끝, base case)이 cert에 포함·검증됨.
3. **type/effect 보존:** K'의 타입·effect가 K와 동일 (no new effect, no widened type).
4. **fallback:** verify≠Valid → emit(K). (P0)
General mode는 정리 약화: "K' 도달 시 동치"만, 종료는 보장 안 함(R35).

## P.2 산술 엣지케이스 (collapse 경로, R33 정확성)
- **overflow:** collapse 경로는 `int/nat/rat`(임의정밀) — closed form 평가가 머신 정수 오버플로로 *틀린 답*을 내면 안 됨. 머신 정수 결과 필요 시 마지막에 refinement로 범위 검증 후 narrow.
- **empty domain:** `sum` 빈 domain → 0; `prod` → 1; `count` → 0. 닫힌형이 이를 만족하는지 cert 경계에서 확인(예 n<0).
- **division:** rational closed form의 분모 0(예 등비합 r=1) → 별도 case (r≠1 전제; r=1이면 n+1). cert가 분모 비0을 boundary로.
- **base cases:** n=0,1 등 작은 값에서 closed form과 원본 일치 명시 검증 (APPENDIX D의 모든 A 항목).
- **negative/zero parameters:** nat 파라미터의 0; 음수는 타입이 배제(nat); General int면 domain 부호 분기.

## P.3 GF(2) / 암호 엣지케이스
- **affine vs linear:** b≠0(NOT/const) 처리 — 순수 선형 아닌 affine. basis에 0 포함해 b 추출.
- **S-box 경계:** 선형 layer는 collapse, S-box는 *반드시* defer[nonlinearity] — 근사·생략 금지. 보안적으로 옳다(linearization 방어가 설계 목적).
- **constant-time:** collapse·백엔드 변환이 secret 경로 timing을 바꾸면 동치 cert가 있어도 거부(R29). NTT butterfly·modular reduction의 data-independence 검증.

## P.4 counting / planar / tensor 엣지케이스
- **trivial graphs:** 빈 그래프 perfect matching = 1(빈 곱); 홀수 정점 = 0; 단일 정점 = 0.
- **planarity 경계:** K₅/K₃,₃ minor → non-planar → defer. 평면성 검사 O(V).
- **treewidth 추정 오차:** 휴리스틱 과소추정 시 메모리 폭발 위험 → 보수적 budget; 중간 tensor 크기 모니터, 초과 시 즉시 slicing 또는 defer(DR8).
- **empty tensor net / scalar:** 즉시 상수.

## P.5 recognizer 엣지케이스
- **saturation 비종료:** node/time budget 필수(R23류). 초과 → 현재 best 추출 또는 None tag(graceful).
- **다중 매칭:** 한 식이 여러 collapse-class 후보 → cost model로 최선 1개; 동률이면 결정적 tie-break(R11).
- **중첩 구조:** affine 안의 holonomic 등 — dispatch가 outer부터; 안 되면 inner. half-collapse 금지(R32) — 전체 검증 or 전체 fallback.

## P.6 "정직성" 메타 엣지케이스 (너 자신)
- **부분 증명:** Lean `sorry`, Z3 `unknown`, timeout → 전부 *비검증* → fallback. "거의 됨"은 "안 됨"(DR7).
- **벤치마크:** 측정값만, kernel+N 명시, non-uniform 표시(R8). 캐시·워밍업 정직히.
- **강등 보고:** kernel이 구조 전제부 불충족으로 강등되면 조용히 넘기지 말고 `constant-factor-only` 보고(R19).
- **거부 보고:** PART 18 요청이 오면 정중히 거부 + 이유. 거부는 실패가 아니라 정확성이다.

---
---

> **문서 끝.** 이로써 JEFF build constitution은 규율(PART 0–22)과 구현 명세(APPENDIX A–P)를 모두 담는다: 우선순위·룰·정직성·타입 calculus·전 크레이트 skeleton·문법·레이어별 알고리즘·certificate 증명·런타임/ABI·진단/리포트 형식·stdlib·컴파일 트레이스·테스트/CI·단계별 티켓·설계 근거·예제·근거 문헌·메타이론. 줄 수가 아니라 이 규율이 목표다.
>
> 세 줄로: (1) **P0–P5 위계를 절대 뒤집지 마라** — never-miscompile > honesty > proof-carrying > conservation > performance > DX. (2) **구조는 certificate와 함께 붕괴, 나머지는 하드웨어 한계로, 불가능은 이름 붙여 거부** — 결코 틀린 답 없이. (3) **defer는 실패가 아니라 정직이다.**
>
> **Stage 0 / 티켓 T0.1(APPENDIX L)부터 시작하라.** 막히면 2.4 형식으로 정직하게 보고하라. JEFF가 다른 컴파일러와 다른 단 하나의 이유 — 속도가 아니라 **정직하게 빠르다는 것**이다. 그 정직성이 곧 이 문서다.
