# HANDOFF — MR.JEFFREY / HARAN  (다음 세션 AI에게 주는 전부)

> 너는 **새 세션**이다. 이전 세션의 기억이 없다. 이 파일이 너의 전부다 — 규율·가치·지금까지 만든 것·
> 현재 상태·지금 할 일. **천천히, 전부 읽어라.** 한 줄도 장식이 아니다(그게 이 프로젝트의 규율이다).
>
> 한 줄 요약: **Claude가 코드를 짜고, MR.JEFFREY가 그걸 수학적으로 *증명*하고 반례로 고치고 최적화한다.
> 사용자가 계속 지시하며 다듬는 대화형 검증 코딩 제품이다. 무기는 속도가 아니라 — 정직하게 증명된
> 정확성이다.**

---

## 0. 지금 당장 할 일 (이것 때문에 새 세션을 켰다)

깨끗한 웹앱 `haran-web/` (33개 모듈 + 에셋, 아래 5장)을 사용자의 새 GitHub 레포
**`mg225111061-design/Projectharan`** (철자 정확히: 대문자 P, 나머지 소문자 — "Projectharan") 로
push 하는 것.

**이전 세션은 이걸 못 했다. 이유는 정직하게: 그 세션의 "허용 레포 목록"이 `jabanjiwan` 하나뿐이라
Projectharan에 git proxy/GitHub 연동 둘 다 `Access denied / repository not authorized`로 막혔다.**
GitHub 쪽 권한 문제가 아니라 *세션 스코프* 문제였다.

→ **네가 가장 먼저 할 일:** Projectharan에 접근 되는지 *실제로 확인*하라(추측 금지).
```bash
# 1) 현재 git proxy 포트 확인
git -C /home/user/jabanjiwan remote get-url origin     # http://local_proxy@127.0.0.1:<PORT>/git/...
# 2) Projectharan 접근 테스트 (PORT 바꿔서)
git ls-remote "http://local_proxy@127.0.0.1:<PORT>/git/mg225111061-design/Projectharan"
```
- **되면(exit 0):** 아래 "push 절차"대로 올리고, 올라갔는지 확인 후 정직히 보고.
- **또 막히면(`not authorized`/`Access denied`):** ★막혔다고 정확히 보고하고★ 사용자에게 두 길을 줘라:
  (A) 이 세션 환경의 repository scope에 Projectharan을 넣고 새 세션,
  (B) 사용자가 본인 터미널/Codespace에서 직접 5줄 실행(아래 "사용자 직접 push"). **가짜로 됐다고 하지 마라.**

### push 절차 (접근 될 때)
`haran-web/`는 그 자체로 완결된 standalone 앱이다. 새 git 히스토리로 올린다(원본 레포 히스토리 안 섞음):
```bash
cd /home/user/jabanjiwan/haran-web
git init && git add . && git commit -m "MR.JEFFREY / HARAN web app — clean standalone"
git branch -M main
git remote add origin "http://local_proxy@127.0.0.1:<PORT>/git/mg225111061-design/Projectharan"
git push -u origin main
# 확인: git ls-remote origin   (HEAD 보이면 성공) → 파일 수/커밋 보고
```
(GitHub MCP가 Projectharan에 권한 있으면 `push_files`로 올려도 된다. 단 33+파일이라 git push가 깔끔하다.)

### 사용자 직접 push (네가 못 올릴 때 사용자에게 줄 정확한 명령)
```bash
tar xf mrjeffrey-web.tar          # (이전 세션이 보낸 tar) 또는 Projectharan Codespace 열기
cd haran-web
git init && git add . && git commit -m "MR.JEFFREY / HARAN web app — clean standalone"
git branch -M main
git remote add origin https://github.com/mg225111061-design/Projectharan.git
git push -u origin main
```

**★ 새 기능을 만들지 마라. ★** 사용자가 명시적으로 시키기 전까지, 지금 할 일은 *이전(이미 만든 것). push와 검증*뿐이다.

---

## 1. ★★★ 정직성 — 절대 규율 (이 프로젝트의 심장. 다른 모든 것을 이긴다) ★★★

이 프로젝트의 가치 제안 전체가 **"정직하게 증명된 정확성"** 위에 있다. 가짜 하나가 들어오는 순간
MR.JEFFREY는 그냥 거짓말하는 또 하나의 코드 도구가 된다. 정직성은 *제약*이 아니라 *기능*이다.
**아래는 협상 불가다. 빠르거나 멋있어도, 정직하지 않으면 실패다.**

1. **가짜 통과 0.** 테스트를 통과하려고 정답 하드코딩·검사 우회·oracle 우회 금지. 막히면 *막혔다고
   말하고* HONEST_DEFER(정직한 보류)하라. "통과한 것처럼 보이게" 만드는 충동이 들면 — 멈춰라.
2. **측정값만 보고.** 속도·배수·퍼센트는 *실제로 측정*했을 때만, 커널+N과 함께. 안 했으면
   **`[TBD: 측정필요]`** 플레이스홀더. "수천 배"·"지수적"·"즉시"를 증명/측정 없이 쓰지 마라.
3. **신기루 금지.** homology / TDA / persistent homology / 양자(quantum) / 상대론(relativity) /
   유체역학(fluid dynamics) / cohomology / manifold-learning 류로 임의 계산을 마법처럼 줄인다는
   주장 0. 실제로 측정·증명된 기법(Z3 / fold 닫힌형태 / Coq / 추상해석)만.
4. **키 보안 — LEVEL 1 (절대).** Claude API 키는 *매번 입력*, 어디에도 저장 0 (env / 파일 / 로그 /
   캐시 / DB / localStorage 전부). 받아서 *그 호출에만* 쓰고 즉시 폐기. 에러 메시지에도 누설 0(redact).
   `claude_agent.py`는 `os`조차 import 안 한다(구조적으로 env/파일 못 건드림). 화면 노로그 정책 문구는
   **실제 코드가 그래야** 한다(문구만 띄우면 거짓말). grep로 증명하라.
5. **코딩 답 ≠ 대화 답 (라벨 절대 안 섞음).** 코드 = HARAN이 *검증*함 → "PROVEN / 반례" 라벨 붙는다.
   잡담/질문 = Claude 일반 답 → **검증 라벨 절대 안 붙임**(검증 안 했으니까). 섞으면 거짓.
6. **진행 표시는 진짜 단계만.** "Claude 호출중 / 검증중 / 최적화중"은 *실제로 그 단계일 때만* 표시.
   안 하는 단계를 하는 척하는 가짜 진행바 0. 잡담 턴은 검증중/최적화중을 안 띄운다.
7. **범위/의도 정직 (Rice).** 검증은 *명세(`ensures`) 대비*지 *의도* 대비가 아니다. "PROVEN"은 코드가
   *명세를 만족*한다는 뜻이지 "네가 원한 걸 맞췄다"가 아니다. 큰 프로그램을 무에서 생성·검증은 불가능
   (Rice) → 통짜 요청엔 정직한 scope 응답(가짜로 "검증된 백엔드" 내놓지 마라).
8. **시각/체감은 사용자 확인 대상.** 예쁨·둥근 정도·색감·전환 부드러움·스트리밍 체감은 자동 판단 불가.
   테스트 통과 ≠ 화면 완성. **화면을 "완성"이라 주장하지 마라** — "로직/백엔드 완성 + 화면은 사용자
   확인 대기"가 정직한 표현.
9. **너 자신에게 정직하라.** 막히면 "막혔다, 이유는 X"라고 *정확히* 보고. 권한 막힘은 502/Access
   denied 같은 *실제 증거*와 함께. 추측 단계를 사실처럼 말하지 마라.
10. **마케팅 카피 분리.** 화면 광고 문구("압도적인" 등 추상 카피)는 소스에 `// marketing copy` 주석.
    HARAN 실제 결과(PROVEN/REFUTED/측정 ms)는 진짜 측정값만. 둘을 섞지 마라.

> 작업 보고 형식(매 작업 끝):
> `DONE: 무엇 / VERIFIED: 어떤 테스트가 green / DEFERRED|BLOCKED: 안 된 것 + 정확한 이유 / NEXT: 다음`.
> "전부 됐다"는 진짜 전부일 때만. 아니면 무엇이 남았는지 명시.

---

## 2. ★ 코드의 강력성·엄밀성 (필수) ★

정직성이 1번이라면, 그 정직성을 *값지게* 만드는 건 **진짜 강한 코드**다. MR.JEFFREY가 "압도적"인 건
과장이 아니라 *실제 동작* 때문이어야 한다:

- **검증 우선(verification-first).** Claude의 휴리스틱을 믿지 않고 *증명*한다. fold(C-finite/
  hypergeometric 재귀 → 닫힌형태 → O(1) 검증), Z3(SMT), Coq(무한 ∀ 귀납), 추상해석. 증명 못 하면
  PROVEN이라 안 한다 → TESTED/UNKNOWN/반례로 정직히.
- **write → verify → fix (v22의 심장).** Claude가 짜면 HARAN이 검증, 틀리면 *구체적 반례*("n=1에서
  2≠1")를 줘서 고치게 하고 재검증 — 증명될 때까지(상한 N). 약한 모델도 반례로 수렴한다. 못 고치면
  *정직히 실패*(가짜 성공 0).
- **fold 최적화.** 증명된 구조적 루프를 닫힌형태로 붕괴(Σk → n(n+1)/2, O(1)). 단 *증명된 변환*만 —
  빠르고 *정확*. 구조 없으면 강등(constant-factor만) + 정직 보고.
- **두 모드.** 일반(빠름/얕음, 단 절대 오답 0) vs 확장(깊은 증명/최적화, 더 많이 해결). 둘 다 *오답 0*.
- **증분 재검증.** 바뀐 함수만 다시 검증(Merkle) → 후속 라운드 체감 0.
- **엄밀성 규율:** 모든 변환은 의미 보존(틀린 답 절대 금지 — 최악도 "놓친 최적화"지 "틀린 답"이 아니다).
  검증 실패 시 항상 원본 fallback. 결정성(같은 입력 → 같은 결과). 테스트 없는 기능 금지.

---

## 3. 프로젝트 정체성

- **이름:** 화면 브랜드 = **MR.JEFFREY** (로고 `MR.JEFFREY`, 탭 제목 MR.JEFFREY). 엔진/내부 코드명은
  HARAN. (Mr.Jeffrey는 원래 Z3 기반 검증기 이름이었고, 이제 제품 전체 브랜드.)
- **무엇:** Claude(생성) → 특수 검증 엔진(강화/증명/최적화) → 사용자가 *계속 지시*하는 대화형 제품.
- **대상:** 작은~중간 코드 + 명세/예시. 큰 프로그램 무에서 생성은 범위 밖(Rice, 정직).
- **차별점:** 다른 AI는 코드를 *생성*만 한다. MR.JEFFREY는 *증명*한다(명세 대비). 가짜 없이 그 자체로 강하다.

---

## 4. 지금까지 만든 것 전부 (v22 → v25. ★새로 만들 것 아님 — 이미 만든 것의 기록★)

브랜치 `claude/funny-maxwell-im9x07` (레포 `mg225111061-design/jabanjiwan`), 코드는 `mr_jeffrey/`에.
v0~v21은 검증 엔진(Mr.Jeffrey/Z3, fold 엔진, Caesar/HeyVL 관계검증, Coq 8.18, 추상해석, 검증속도
최적화 체감0+두 모드, 보안/동시성/메모리/종료)으로 이미 완성(401 체크). v22~v25가 이번 작업:

- **v22 (Part S, S1~S7) — LLM+수학 에이전틱 코딩:**
  `claude_agent.py`(레벨1 키, mock/live, 공식 Anthropic SDK, 기본 모델 `claude-opus-4-8`),
  `agentic.py`(`write_verify`, `write_verify_fix`=심장, `optimize`=fold, `compare_modes`,
  `verify_typeA`=명세 박힌 증명 등급, `agentic_code`=통합 엔트리, `agentic_stream`=스트리밍,
  `measure_agentic`=정직 측정). 측정(mock): 루프 수렴·폴드 O(1)·반례로 잡은 버그.
- **v23 (Part T, T1~T10) — 웹:** `haran.html`(단일 파일 프론트: 한/영, 둥근, 두 모드, 거대 예시,
  피드백 루프), `server.py`(FastAPI + SSE `stream_events` + `handle_generate`), Docker/compose/
  requirements. 키 레벨1(받아 쓰고 폐기·로그 0).
- **v24 (Part U, U1~U10) — 의도/대화/진행/색/실연결:** `intent.py`(`classify_intent`=키워드 우선
  로컬 sub-ms + 애매하면 Claude / `assess_clarity`=명확하면 진행, 애매하면 예상 질문 / `chat_reply`=
  비코딩 일반 답, 검증 라벨 0 / `route`=통합 라우터 code|chat|ask). 진행 단계 SSE(분류중/Claude
  호출중/검증중/반례 수정중/최적화중 — 진짜 단계만). 흑백 대신 당시 cool/warm → v25에서 흑/백으로.
  칩 숨김/펴기, 예상질문 접기/펴기. 친절+키안전 에러.
- **v25 (Part V, V1~V8) — 깨끗한 레포 + 배포 + 키 UX:**
  `build_clean_repo.py`(server.py 의존성 추적 → 33개 모듈+에셋만 `haran-web/`로 복사, 228개 중 33개,
  원본 보존). 배포: localhost:8000 **실측 작동 확인**(라이브 curl). ★실행 중 진짜 버그 발견·수정★:
  `from __future__ import annotations` 때문에 FastAPI가 `req: Request`를 쿼리파라미터로 오인 →
  `Request`를 모듈 스코프에 노출해 고침. `/health` 추가. **키 마스킹(●●●●)** + 키칸 옆 **강조색 `*`**
  클릭 시 **노로그 정책 팝오버** ("Mr.Jeffrey는 API Key 노-로그 정책을 고수합니다. 여기에 넣으면
  어디에도 유출되지 않고 저장되지 않습니다."). 노로그 **grep 실증**(env-키 읽던 휴면 함수 2개도 제거).
  흑(일반)↔백(확장) 테마 + 그라데이션 크로스페이드(가독성 보장).

---

## 5. 현재 상태 (사실, 확인됨)

- **테스트 131/131 green** (mr_jeffrey/test_*.py 전체). 단 `test_stage2.py`·`test_w5.py`는 *부하 민감
  타이밍 flake* — 단독 실행 시 통과, v22~v25 코드와 무관(import 안 함). 가짜 아님, 환경 부하 탓.
- **`haran-web/` standalone 작동 확인:** `/health`→200 `{"ok":true}`, `/`→200 페이지, chat/code/ask
  라우팅 정상. (이전 세션이 TestClient + 라이브 curl로 검증.)
- **localhost 실행:** `cd haran-web && pip install -r requirements.txt && python server.py` →
  http://localhost:8000 (env: HARAN_HOST/HARAN_PORT). 키 없으면 mock(SIM 라벨), 키 넣으면 live.
- **원본 보존:** `mr_jeffrey/`(약 233 .py)와 기존 `haran-web/` 그대로. (이전 세션이 보낸 산출물:
  `mrjeffrey-web.tar` = haran-web 그대로.)
- **배포 URL 없음:** 공개 배포는 사용자 계정 필요(샌드박스에 docker 데몬·클라우드 계정 없음). 명령만 README에.

---

## 6. 핵심 파일 지도 (haran-web/ — 웹앱에 실제 필요한 33개)

- `server.py` — FastAPI. `create_app()`(GET `/`,`/health`; POST `/api/generate`,`/api/stream`SSE),
  `handle_route`(분류→code/chat/ask 직렬화), `stream_events`(진짜 단계 SSE), `reverify_incremental`.
  ★`Request`는 모듈 스코프 import 필수(PEP 563 버그)★. 키 레벨1, env는 HARAN_HOST/PORT만(키 절대 X).
- `intent.py` — `classify_intent`/`assess_clarity`/`chat_reply`/`route`/`is_scope`. 키워드 우선,
  애매하면 Claude(중립 JSON 시스템 프롬프트). os import 없음.
- `claude_agent.py` — `claude_generate(prompt, api_key=None, ...)`. live=공식 SDK(claude-opus-4-8,
  adaptive thinking, streaming), 없으면 mock(source="mock-sim"). **os import 없음 / 로깅 없음 /
  키 저장 0 / 에러 redact(`_friendly_error`,`redact_key`)**.
- `agentic.py` — write→verify→fix 루프, fold 최적화, 두 모드, verify_typeA, agentic_code/stream.
- `haran.html` — 단일 파일 프론트(브랜드 MR.JEFFREY, 한/영 i18n parity, 흑/백 테마, 진행 스피너,
  칩 토글, 예상질문 패널, 키 마스킹 ●●●● + 강조 * 노로그 팝오버, SSE 소비자 + mock fallback).
- 나머지: `mr_haran/prove_exact/closure_classifier/fusion/haran_eval/haran_parser/haran_ast/
  haran_to_obligations/haran_cache/hir/property_test/properties/.../z3_adapter/...` = 검증 엔진 핵심.
- `requirements.txt`(fastapi,uvicorn,anthropic,sympy,z3-solver) · `Dockerfile` · `docker-compose.yml`
  · `.gitignore` · `.dockerignore` · `README.md`.

---

## 7. 검증·실행·재생성 방법 (직접 돌려서 확인하라 — 추측 금지)

```bash
# 전체 회귀 (mr_jeffrey/)
cd /home/user/jabanjiwan/mr_jeffrey
for t in test_*.py; do python3 "$t" >/dev/null 2>&1 && echo "PASS $t" || echo "FAIL $t"; done

# 깨끗한 웹앱 재생성 (의존성 추적 → haran-web/)
python3 build_clean_repo.py        # 228개 중 33개만 ../haran-web/로

# 웹앱 실행 + 헬스 체크 (포트 충돌 시 다른 포트)
cd /home/user/jabanjiwan/haran-web
pip install -r requirements.txt
HARAN_PORT=8000 python3 server.py &     # 백그라운드면 PID 저장, 죽일 땐 lsof -ti:8000 | xargs kill
curl -s http://127.0.0.1:8000/health    # {"ok":true,...}
# 프론트 JS 문법: node --check (haran.html의 <script> 추출 후)
```
- 서버 죽일 때 `pkill -f server.py` **쓰지 마라** — `-f`가 자기 셸까지 죽인다(이전 세션이 당함).
  `lsof -ti:8000 | xargs -r kill` 처럼 포트로 죽여라.

---

## 8. 절대 하지 말 것 (NEVER)

1. 가짜 통과·정답 하드코딩·검사/oracle 우회. 2. 키를 env/파일/로그/localStorage 등 어디든 저장.
3. 신기루(§1.3) 주장. 4. 코딩/대화 라벨 섞기. 5. 안 하는 단계의 가짜 진행바. 6. 미측정 수치를
측정값인 척. 7. 화면을 "완성"이라 주장(사용자 확인 대상). 8. 권한 막힘을 숨기거나 추측으로 우회한 척.
9. **시키지 않은 새 기능 추가**(지금 임무는 push+검증). 10. 원본(mr_jeffrey/, 기존 haran-web/) 삭제·이동.

---

## 9. 새 AI(너)에게 하는 말

이전 세션이 너에게 남긴 진심이다:

- **정직이 전부다.** 이 제품의 단 하나의 차별점은 "정직하게 빠르고 정확하다"는 것이다. 네가 단 한 번
  가짜로 통과시키거나, 막힌 걸 안 막힌 척하면 — 이 프로젝트의 존재 이유가 사라진다. 막히면 막혔다고,
  증거(에러 메시지)와 함께, 그리고 사용자가 *실제로 할 수 있는 정확한 다음 절차*와 함께 말하라.
- **실제로 돌려서 확인하라.** 구조만 보고 "될 것"이라 하지 마라. 이전 세션은 서버를 *실제로 띄워서*
  치명적 FastAPI 버그를 잡았다(`req: Request`). 테스트를 돌리고, 서버를 띄우고, curl 하라.
- **강한 코드를 정직하게 드러내라.** MR.JEFFREY의 증명·반례 수정·fold O(1)·두 모드는 진짜 강하다.
  과장 없이, 측정값과 PROVEN 라벨로, 그 강함을 *최대한* 보여줘라. 단 한 숫자도 지어내지 말고.
- **지금 임무는 단순하다:** `haran-web/`를 Projectharan에 올리고(되면), 안 되면 정확히 왜 안 되는지 +
  사용자가 칠 명령을 주는 것. 그게 전부다. 새로 뭘 만들지 마라.
- **사용자는 정직을 원한다.** 빠른 거짓보다 느린 진실을. 화려한 주장보다 검증된 사실을. 그 신뢰를 지켜라.

> 마지막으로, 세 줄로:
> 1) **정직성 > 그 무엇.** 가짜 0, 측정만, 막히면 정확히 보고.
> 2) **코드는 진짜로 강하게** — 증명·반례·fold·두 모드, 과장 없이.
> 3) **지금은 push + 검증만.** 이미 만든 것을 옮기고 확인하라. 새 기능 금지.
>
> — 이전 세션 (v22~v25를 만든 AI)가, 너에게.
