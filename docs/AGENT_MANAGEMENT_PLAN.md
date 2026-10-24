# 범용 에이전트 관리 단계별 계획

작성일: 2026-10-23. 1~3단계와 에이전트 등록 단계가 구현됐고, 새 에이전트 실행 연결과 프로필 저장은 계획이야. 현재 동작은 [ARCHITECTURE.md](ARCHITECTURE.md), 검증 범위는 [PROGRESS.md](PROGRESS.md)를 기준으로 해.

## 현재 코드에서 확인한 출발점

| 항목 | 현재 구현과 근거 |
|---|---|
| Codex Lead | `src-tauri/src/cli.rs`의 `Rpc`가 App Server stdio JSON-RPC 연결과 thread/turn을 유지해. `thread/start`에 `request_arch_review` Dynamic Tool을 등록하고 공개 답변 delta·도구 호출 이벤트를 `engine.rs`에서 처리해. |
| Antigravity Arch | `cli::arch`가 검토마다 `agy`를 새로 실행해 `stream-json`의 최종 구조화 응답을 검증해. 현재 앱은 Arch 대화 재개에 `--conversation`을 사용하지 않아. |
| 공통 부분 | 실행 파일 탐색, 자식 프로세스 종료, 중단·오류·audit 기록, SQLite 상태 갱신이 있어. 세션 수명과 입출력 프로토콜은 서로 달라. |
| 상태·화면 | `store.rs`의 `agents(id,status,detail,updated_at)`에는 고정된 두 행이 있어. Agents 화면은 기존 실행 상태와 별개로 읽기 전용 설치 점검 결과, 인증·실행 가능 여부를 표시해. |
| 실행 경계 | `Engine`의 단일 worker가 Codex 턴을 직렬 처리해. Arch 검토 중 Codex의 열린 도구 호출을 기다리게 하고 같은 RPC 호출에 결과를 반환해. `tool_deliveries`는 prepared/sent/confirmed/unknown을 기록해. |

## 역할별 인터페이스 — 2단계 구현

`src-tauri/src/agents.rs`의 `AgentAdapter`는 식별·표시 정보, 연결 방식, 역할, `AgentCapabilities`를 제공하고 실행 계약은 역할별로 분리해. 현재 `probe`는 실행 파일 경로만 읽어 설치 상태를 반환해. 모든 어댑터에 지속 세션이나 검토 메서드를 강제하지 않아.

```text
AgentAdapter
  id / displayName / connection / roles / capabilities / probe(읽기 전용)

LeadAdapter
  connect(store) -> LeadSession
LeadSession
  openOrResume(localSession) / beginTurn / nextEvent
  replyToToolCall(원래 RPC 호출, result)
  cancelTurn / close

ReviewAdapter
  review(store, question, conversationVersion, cancellation) -> ReviewResult
```

`AgentCapabilities`는 `leadConversation`, `delegatedReview`, `persistentSession`, `streaming`, `cancellation`, `modelCatalog`, `contextCompaction`, `toolCallBridge` 등의 지원 여부를 나타내. Codex는 Lead와 열린 도구 호출 연결, Antigravity는 Reviewer와 취소를 지원해. Arch의 `stream-json`은 내부 결과 판독 형식이며 공개 답변 스트리밍 capability로 표시하지 않아. **기능 지원, 설치, 인증, 실행 허가는 서로 다른 값**이야. Codex의 열린 도구 호출 수명과 중복 방지 키는 기존 `Rpc`/Engine 경계에서 보존해. Engine이 이벤트를 읽으며 같은 턴을 재개하는 현재 구조를 유지하려고 Lead 계약을 `beginTurn`과 `nextEvent`로 나눴어.

## 상태와 저장 모델 — 3단계 구현

| 차원 | 제안 상태 | 원칙 |
|---|---|---|
| 설치 | installed / missing / unknown / error | 실행 파일 탐색 결과. 경로 부재와 점검 실패를 구분해. |
| 인증 | authenticated / required / unknown / error | 안전한 인증 점검 또는 실제 연결 결과만 사용해. 설치만으로 인증 성공이라 하지 않아. |
| 실행 가능 | ready / unavailable / unknown + 사유 | 필요한 기능·설정·설치·인증을 합쳐 판단해. 인증을 확인할 방법이 없으면 unknown을 허용해. |
| 현재 실행 | idle / starting / running / waiting / cancelling / completed / failed / unknown | 실행 ID·로컬 세션 ID·에이전트 ID와 연결해. 재시작 시 열린 호출을 자동 재개하지 않아. |

각 점검 결과에는 `checkedAt`을 둬 오래된 성공을 현재 성공으로 표시하지 않아. 기존 `agents` 표의 행과 ID는 유지하고 실행 상태 호환용으로 사용해. 설치·인증·실행 가능 probe는 별도 Tauri 명령으로 조회해 화면 메모리에만 보관해. 인증 정보는 읽지 않으므로 설치된 에이전트의 인증·실행 가능 여부는 `unknown`이야. 사용자 정의 프로필 저장이 실제로 필요해질 때 보존형 SQLite v4→v5 마이그레이션을 설계해. 기존 `messages`, `chat_sessions`, `tool_deliveries`, `session_context`, 앱 식별자와 데이터 경로를 변경·초기화하지 않아. 새 에이전트 ID를 저장할 때 `Store::status`가 갱신 행 0개를 성공으로 처리하지 않게 점검해.

## Agents 화면 데이터 흐름 — 3단계 구현

1. Rust 등록 목록의 두 어댑터가 `probe_agents`에서 ID·연결 방식, 설치·인증·실행 가능·점검 시각·실패 사유를 반환해. 추가 provider 등록은 없어.
2. Agents 화면을 처음 열거나 사용자가 새로고침하면 비파괴 점검 명령을 호출해. 실행 파일 경로만 확인하고 인증 파일·설정·CLI는 읽거나 실행하지 않아.
3. 기존 `get_snapshot`과 `chat-updated`는 실행 상태·현재 실행 ID를 갱신해. 활성 실행은 `activeRun`의 로컬 세션·실행 ID와 연결하고, 종료된 상태를 다른 세션의 활성 실행으로 표시하지 않아.
4. 네 상태를 별도 항목으로 표시하고 `점검 중`·`점검 전`·실패 사유·연결 방식·시각을 보여. 인증이 불명인 설치 상태를 실행 가능으로 승격하지 않아.

## 단계별 구현·검증

1. **현재 동작 고정:** 아래 1단계 회귀 표를 먼저 통과시켜. 이 단계는 기존 실행 계약을 관찰하고 테스트만 추가하며 어댑터·새 연결·DB 스키마·화면을 바꾸지 않아.
2. **어댑터 래핑 완료:** 기존 `Rpc`와 `cli::arch`를 역할별 인터페이스 뒤로 옮겼어. 단일 worker, 기존 Dynamic Tool 이름, 같은 RPC 응답, timeout·중단·종료 순서와 SQLite 쓰기 경로를 유지하고 추가 provider는 등록하지 않았어. 검증은 [PROGRESS.md](PROGRESS.md)에 기록했어.
3. **상태 점검·관리 화면 완료:** 고정된 두 어댑터의 네 상태를 분리해 표시해. 안전한 인증 점검 수단이 없어 unknown으로 둬. 수동 새로고침·오류·점검 시각을 실제 WebView에서 검사했어. 근거는 [PROGRESS.md](PROGRESS.md)에 기록했어.
4. **필요할 때 프로필 저장:** 역할·활성 여부 등 사용자가 편집할 항목과 권한 경계를 먼저 확정한 뒤 v5 추가 마이그레이션과 구버전 DB 복구 검사를 수행해. Claude Code·Gemini CLI 등 새 연동은 별도 단계야.

테스트 전용 `Mock Agent`는 공개 답변, 열린 도구 호출, 늦거나 중복된 응답, 잘못된 요청 ID, 인증 불명, 실패·시간 초과·중단·프로세스 종료 불명확을 스크립트 이벤트로 재현하게 해. 단일 worker 직렬성, `latestUserRequirements` 우선 분리, `reviewIsStale`, `tool_deliveries` 상태, 재시작 후 unknown, 기존 DB 보존을 검사해. Mock 통과를 실제 Codex·Antigravity 연결 성공으로 집계하지 않고 기존 실제 CLI/WebView 회귀를 별도로 실행해.

### 1단계 회귀 범위와 판정

| 계약 | 격리 단위·Mock 검사 | 실제 CLI 통합 검사 |
|---|---|---|
| Codex 사용자 턴·세션 재개 | 세션/thread 결합, 턴·메시지 복구와 세션별 버전 고정 (`characterization_tests.rs`, 기존 `workspace.rs`) | `verify_workspace.py`, `verify_restore.py`의 신규 테스트 세션·복사본만 사용 |
| Codex → Arch → 같은 Codex 턴 | 가짜 stdio 상대에서 원래 RPC ID로 응답하고 중복 전송 차단 (`cli.rs`), 리뷰 요청·답변 상관 ID 보존 | `verify_live.py`: 실제 도구 호출 2회, 같은 thread/turn 복귀, 최종 답변 |
| 도구 결과 전달 | `prepared → sent → confirmed`, 중복 차단, 쓰기 실패 `unknown`, 재시작 `unknown`과 기록 보존 | `verify_live.py`의 호출당 wire 응답 1회와 완료 event; 프로세스 장애 전체의 exactly-once 증명은 아님 |
| 위임 중 취소·시간 초과·실패 | 대기 입력 취소·다른 세션 보존, 제한 시간 범위·잘못된 리뷰 결과 거부 (`store.rs`, `cli.rs`) | `verify_cancel.py`의 취소·프로세스 종료; `verify_live.py`의 CLI 부재 실패. 실제 30초 Arch 시간 초과 유도는 별도 검증 공백 |
| 세션별 실행 상태 | 선택/실행 세션의 메시지·컨텍스트 격리, 실행 중 cursor는 재시작 후 복구하지 않음 (`characterization_tests.rs`, `context.rs`) | `verify_workspace.py`, `verify_restore.py` |
| SQLite 기록 보존 | 격리 DB 재개방 후 메시지 순서·ID·본문, 세션/thread, 협업 ID, `tool_deliveries`, schema v4 검사 | 각 통합 검사에서 `ESH_DATA_DIR` 격리 DB와 복원 snapshot 비교 |
| 수동 컨텍스트 압축 | 동일 thread·turn·item ID 검사, 중복 예약, 완료·failed·unknown 뒤 재요청 (`context.rs`, `engine.rs`) | `verify_compact.py`의 완료 경로; 실제 CLI 실패·시간 초과 강제 재현은 검증 공백 |

단위·Mock은 `mvp-app/`의 `npm test`로 반복 실행해. 실제 CLI 스크립트는 설치·로그인·모델 사용량과 실제 Tauri WebView가 필요하며 검증용 외부 Codex thread가 남을 수 있어. 테스트 DB만 `ESH_DATA_DIR`로 격리돼. 과거 증거와 이번 실행 결과를 구분해 기록하고, 건드리지 않은 원본 세션의 `resume`으로 검증하지 않아.

2026-10-23 1단계 실행에서 `verify_workspace.py`가 세션 전환 입력이 `queued`로 남아 실패했어. 후속 세션 전환 수정 후 실제 A→B→A 회귀와 단위·Mock 26개가 통과했어. 이전 실패와 현재 근거는 [PROGRESS.md](PROGRESS.md)에 구분해 기록했어. 실제 연결 장애 경합 등 남은 검증 공백까지 모두 해결된 것으로 넓혀 해석하지 않아.

## 주요 위험과 범위 경계

- 범용화를 이유로 Blocking Delegation을 비동기 병렬 작업으로 바꾸면 같은 Codex 도구 호출에 결과를 돌려주는 계약이 깨져. 기존 호출을 연 채 Arch 종료를 기다려야 해.
- 설치 성공을 인증·실행 가능으로 오인하거나, 앱 전역 `agents.status`를 선택 세션의 실행 상태로 오인하면 관리 화면이 잘못 안내해.
- 현재 Codex는 MCP·apps를 비활성화하고 읽기 전용 대화 세션으로 시작해. Antigravity의 plan 모드·도구 금지 지침도 OS 수준 샌드박스가 아니야. 이 계획은 Skills/MCP, 파일·셸 권한, Strata/OpenCode 설정을 활성화하거나 변경하지 않아.
- 도구 응답의 `confirmed`는 확인된 단일 연결 결과야. 비정상 종료까지 포함한 exactly-once 보장으로 넓히지 않아.
# 현재 구현 메모 (2026-10-24)

아래 1~3단계 기록은 당시 계획과 검증 범위를 설명해. 이후 Agents 등록 단계에서 SQLite v5 `agent_instances`, Claude Code/Gemini CLI/OpenCode 파일 탐지·수동 경로 등록·해제가 추가됐어. 새 CLI 실행 어댑터는 없고 기존 단일 worker·Blocking Delegation은 유지해. 현재 동작과 검증 근거는 [ARCHITECTURE.md](ARCHITECTURE.md), [PROGRESS.md](PROGRESS.md)를 봐.

