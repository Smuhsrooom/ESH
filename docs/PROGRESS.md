# ESH 진행 상태와 개발 인계

## 세션 종류를 등록에서 고르게 하고 Claude 세션 경로 추가 (2026-10-26, 3단계 다섯 번째 조각)

- 선택한 작업: 3단계(추가 CLI 어댑터)의 다음 조각으로 지난 회차가 남긴 (b) "`workspace.rs`의 `create_claude_session` + `main.rs` `create_session`의 provider 분기". `create_session` 명령이 `agent_instance_id`를 **무조건** `create_opencode_session`으로 보내고 있어서, Claude Code 등록으로 세션을 만들면 OpenCode 문구로 실패했고 세션 종류가 한 provider에 고정돼 있었어.
- **이번 실행의 특이 사항(먼저 밝힘)**: 시작 시점에 `.automation.lock`이 `2026-10-26 00:58:55`로 남아 있었고(약 17시간 28분 경과, 90분 초과) 작업 트리에 커밋되지 않은 변경 3파일이 있었어. 즉 00:58에 시작한 회차가 검증·커밋 전에 중단된 상태야. 그 변경이 바로 이번에 선택한 (b) 작업과 일치해(내용을 읽어 확인) **버리지 않고 이어받아** 검증·수정·커밋했어. 이전 회차가 이미 작성한 코드라는 점을 기록해 둬 — 이번 회차가 처음부터 작성한 것이 아니야.
- 변경 파일: `mvp-app/src-tauri/src/workspace.rs`, `mvp-app/src-tauri/src/main.rs`, `mvp-app/src-tauri/src/claude.rs`(모듈 주석만).
  - `create_agent_session(room_id, value, instance_id)` 추가 — 등록 행의 `provider`를 읽어 `opencode`는 `create_opencode_session`, `claude_code`는 `create_claude_session`으로 보내고 그 밖(`codex`·`antigravity`·`mock_agent`)은 `이 에이전트로는 대화 세션을 만들 수 없습니다`로 거절해. `create_session` 명령은 이 함수 하나만 호출해.
  - `create_claude_session(room_id, value, instance_id)` 추가 — `provider == "claude_code"`, `role == "lead"`, `installation == "installed"`를 확인하고 **`connection == claude::CONNECTION_KIND`까지** 확인해. `active_role`이 아니라 `connection`을 보는 이유는 등록 조각이 아직 없어 `active_role`이 `None`이기 때문이야(지난 회차의 `claude_runner`가 `role`을 보는 것과 같은 이유). 지금 `register_agent_instance`는 `not_configured`만 쓰므로 **어떤 등록도 이 경로를 통과하지 못하고**, 어댑터가 `claude_code_cli`를 부여하는 순간 이 경로가 스스로 열려.
  - `create_lead_session(room_id, value, instance_id, provider)` 추출 — 두 CLI Lead가 공유하는 쓰기야. `add_session` 뒤 `provider`·`agent_instance_id`를 **한 문장**으로 쓰고 같은 트랜잭션에서 `meta.revision`을 올리며, 갱신 행이 1이 아니면 오류로 롤백해. 이전 `create_opencode_session`은 검사 없는 `execute` 두 번이었어.
  - `claude.rs`는 모듈 주석만 갱신 — "세션 경로가 생겼으나 등록이 없어 도달 불가"로 사실을 맞췄어. 계약·파서·`run` 시그니처·`CONNECTION_KIND` 값은 그대로야.
- 동작 보존: OpenCode 세션 생성은 검사·최종 행 값이 같고(쓰기가 트랜잭션 + `changed != 1` 가드로 바뀐 것만 다름), `create_session`에서 `agent_instance_id` 없이 부르는 경로(Codex 기본 세션)는 그대로야. Claude 등록으로 세션을 만들려 하면 이전에는 `실행 가능한 OpenCode Lead 등록이 아닙니다`, 지금은 `Claude Code 실행 연결이 설정되지 않은 등록입니다`로 **둘 다 실패**하고 세션은 만들어지지 않아 — 살아 있는 동작 변화는 없어. `support`/`active_role`/Agents 화면 값도 그대로야(등록을 하지 않았으므로). Blocking Delegation, `request_arch_review`, `archReview`/`latestUserRequirements` 구조적 분리, `tool_deliveries`, 종료 순서, Arch 타임아웃 120초(범위 30~600), 단일 worker, DB 스키마·`user_version=6`, 실행 파일 이름 `esh-chat-mvp4` 모두 그대로고 새 Tauri 명령·새 DB 쓰기·프런트 변경은 없어.
- 검증 (실제 출력):
  - `cargo test --bin esh-chat-mvp4` (프로젝트 상위 `.tools`의 cargo를 `src-tauri/`에서 직접 실행) → `test result: ok. 84 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.84s` (기존 83 → 84, 순증 1). 신규 `workspace::tests::a_claude_session_needs_a_registration_the_adapter_claims` → `ok`. Rust 경고는 기존 2건(`agents.rs`의 `Authenticated`·`Required`·`Error`, `Ready` 미생성 variant)뿐이고 `workspace.rs`·`main.rs`·`claude.rs`에서 새 경고는 없어.
  - `node scripts/verify_external_threads.mjs` → `{"passed":true,"recordedRows":27,"scratchRowsHidden":9,"normalConversationsPreserved":true}`
  - `npm run build` → `✓ built in 5.09s` (`dist/assets/index-B_6vxOOa.js` 433.29 kB, `index-YBSTmYxd.css` 52.24 kB, `guiProbe-djswdxyz.js` 45.64 kB — 프런트를 건드리지 않아 해시 동일)
  - 경고 재확인: `scripts/rust.ps1`은 이 환경에서 cargo의 stderr 진행 메시지에 끊겨 exit 1로 보이므로 `.tools\cargo\bin\cargo.exe`를 `CARGO_HOME`/`RUSTUP_HOME`/`PATH`를 `.tools`로 맞춰 직접 실행했어. **작업 디렉터리는 `mvp-app/`이 아니라 `mvp-app/src-tauri/`여야 해** — `mvp-app/`에서 돌리면 `could not find Cargo.toml`로 즉시 실패해(이번에 한 번 실패한 뒤 경로를 고쳤어).
- 신규 테스트가 실제로 확인한 것: (1) `claude_code` Lead 등록을 만들어도 `connection == not_configured`이면 `create_claude_session`·`create_agent_session` **둘 다** `실행 연결이 설정되지 않은`으로 거절하고, `support`는 `unsupported`, `active_role`은 `None`으로 남는다는 것, (2) 어댑터가 부여할 `connection_kind`를 DB에 직접 넣은 뒤에는 세션이 `provider='claude_code'`로 만들어지고 **반환값과 실제 저장 행 모두** 등록 ID를 갖는다는 것, (3) 세션을 만든 것만으로 `support`/`active_role`이 올라가지 않는다는 것(등록 순서 규칙 유지), (4) 세션이 등록을 고정하므로 `unregister_agent_instance`가 계속 거절된다는 것, (5) `role == reviewer`인 Claude 등록, 실행 파일이 사라진 등록, `codex`/`antigravity`/없는 ID는 각각 거절된다는 것, (6) 재시작(`Store::open`) 후에도 `provider`가 `claude_code`로 남는다는 것.
- 이 검증의 경계(중요):
  - `create_session` **Tauri 명령 자체**는 `Engine`(`AppHandle`)을 들어 단위 테스트로 호출하지 못했어. 테스트는 그 명령이 호출하는 `create_agent_session`/`create_claude_session`/`create_lead_session`을 실제 SQLite로 검사한 것이고, 명령 배선은 컴파일과 코드 확인으로만 검증했어.
  - **`claude_code` 세션은 여전히 실제로 만들 수 없어.** 등록이 `claude_code_cli`를 받는 경로가 없기 때문이야(어댑터 미등록). 이번 조각은 "등록이 그 값을 받는 순간 열릴 세션 경로"를 만든 것이고, "Claude Code 연결"이나 "Claude 턴 실행"이 아니야.
  - 실제 `claude -p` 1회, `python scripts/verify_*.py`, WebView, 데스크톱 빌드는 실행하지 않았어(변경이 Rust 백엔드이고 실행 중인 앱을 건드리지 않으려고).
- 남은 문제:
  1. **승인 대기**: 실제 `claude -p` 1회로 stream-json 원문 캡처(외부 데이터 전송 + 계정 사용량). 이때까지 `parse_events`는 공개 문서 기준이야.
  2. 등록 조각 미구현 — `ConnectionKind::ClaudeCodeCli`·`ClaudeCodeAdapter` descriptor·`registered()` 추가(`agents.rs`), `register_agent_instance`에서 `claude_code` Lead + 설치된 경로일 때 `connection_kind = claude_code_cli` 부여와 `active_role` 매핑(`registry.rs`). **이 조각이 들어가면 위 세션 경로가 실제로 열리므로** 그때 `claude_runner`의 검사를 `role` → `active_role == Some("lead")`로 좁히고 `support`를 실행 경로와 일치시켜야 해.
  3. `create_lead_session`의 두 커밋 — `add_session`이 먼저 커밋되므로 그 사이에 읽는 소비자는 아직 `provider='codex'`인 세션을 볼 수 있어. provider·등록 ID와 revision은 한 문장으로 묶었지만 세션 행 생성까지 한 트랜잭션으로 합치지는 않았어(기존 OpenCode 경로도 같았음). 실제 관측된 문제는 없고, 필요하면 다음에 `add_session`을 같은 트랜잭션으로 흡수해야 해.
  4. 도구 정책 미확정 — `--permission-mode plan`이 대화 전용 계약을 실제로 지키는지 미검증.
  5. 1단계 잔여 4건(GUI 실행 확인, `tauri-plugin-single-instance` 내부 경합, 원본 세션 전역 잠금=승인 대기, 종료 중 거절의 WebView 재현)은 그대로 차단 상태야.
- 다음 작업: (c) `agents.rs`+`registry.rs` 등록 조각 — `claude_code` Lead 등록에 `claude_code_cli` 연결을 부여하고 `ClaudeCodeAdapter`를 `registered()`에 넣은 뒤, 같은 조각에서 `claude_runner`를 `active_role` 기준으로 좁혀 `support` 표기가 실행 경로와 일치하게 맞출 것. 승인을 받으면 그 전에 실제 `claude -p` 캡처로 `parse_events`를 확정하는 편이 좋아.
- 실행 메모(이 환경): `.tools\cargo\bin\cargo.exe`는 **`mvp-app/src-tauri/`에서** 실행할 것(`mvp-app/`에는 `Cargo.toml`이 없어 `could not find Cargo.toml`). `Out-File -Encoding utf8` 로그는 BOM 때문에 `grep`이 첫 줄을 못 잡을 수 있으니 `grep -a`로 확인할 것.

## 실행 분기를 허용 목록으로 전환하고 Claude 턴 분기 추가 (2026-10-26, 3단계 네 번째 조각)

- 선택한 작업: 3단계(추가 CLI 어댑터)의 다음 조각으로, 17회차가 남긴 (a) "`worker`의 target 분기를 `turn_route`로 바꾸고 `claude_turn` 추가". 17회차가 세운 가드는 `submit`에만 있었고 `worker`는 여전히 `input.target` 문자열을 비교해 마지막 분기가 Codex 턴이었어. 즉 세션 provider와 target이 어긋난 입력이 큐에 남아 있으면(예: 큐에 넣은 뒤 provider가 바뀐 경우) Codex가 답하고 그 세션 에이전트가 답한 것처럼 저장될 수 있었어. 이번에 분기 기준을 판정 함수 결과로 바꿔 그 경로를 없앴어.
- 변경 파일: `mvp-app/src-tauri/src/engine.rs`, `mvp-app/src-tauri/src/claude.rs`(모듈 주석만).
  - `TurnRoute::Claude` 추가 + `turn_route`에 `"claude_code" => (target == "claude_code").then_some(TurnRoute::Claude)` 한 줄. Codex 세션의 허용 목록(`codex`/`antigravity`)과 OpenCode 세션(`opencode`만)은 그대로야.
  - `worker`가 `self.store.session(...)`으로 읽은 세션과 target으로 `route`를 한 번 계산하고, `route == Some(TurnRoute::Arch/OpenCode/Claude/Codex)`로 분기해. 문자열 비교 3곳(`antigravity`·`opencode`·`codex`)이 사라졌고, **어떤 route에도 해당하지 않는 입력은 Codex로 떨어지지 않고 오류로 끝나**. 오류 경로의 에이전트 키도 route를 따라가 — CLI lead의 상태 행은 provider 이름이 아니라 등록 ID로 잡히므로, Claude/OpenCode 실패는 `agent_key`(등록 ID), Arch는 `antigravity`, Codex는 `codex`에 기록돼. 이전에는 `input.target`을 그대로 써서 `claude_code` 이름의 상태 행을 찾다 실패했을 자리야.
  - `commit_opencode_answer` → `commit_lead_answer(store, speaker, session_id, user_id, external, text)`로 일반화. 두 CLI lead가 **확정 → 외부 세션 바인딩 → 답변 저장** 순서를 한 곳에서 공유하고, 답변 작성자(`opencode`/`claude_code`)만 달라. 14회차에 정한 순서 규칙은 그대로야.
  - `claude_runner(store, session)` 추가 — provider가 `claude_code`인지, 등록 ID가 있는지, 등록이 존재하는지, `role == "lead"`이고 실행 파일이 지금도 있는지 확인해 (등록 ID, 경로)를 돌려줘. `active_role`이 아니라 `role`을 보는 이유는 아래 남은 문제 2번에 적었어.
  - `claude_turn(user, stop)` 추가 — `claude::run(경로, data_dir, 저장된 세션 ID, 모델, 사용자 메시지, stop)` → 중단 확인 → `commit_lead_answer("claude_code", ...)` → 상태 `completed`. OpenCode와 같은 형태야.
  - `claude.rs`는 모듈 주석만 갱신했어("등록·배선 없음" → "Engine 분기는 있으나 도달 불가, 등록·세션 경로가 남음"). 계약·파서·`run` 시그니처는 그대로고 `CONNECTION_KIND`는 여전히 미사용이라 `#![allow(dead_code)]`를 유지했어.
- 동작 보존: 도달 가능한 세 조합(`codex`→`codex`, `codex`→`antigravity`, `opencode`→`opencode`)은 같은 분기로 가고 거절·오류 문구도 같아. `claude_code` 세션은 만들 수 없고 `submit`도 `claude_code` target을 거부하므로(`registered()`에 그런 어댑터가 없음) 살아 있는 동작 변화는 없어. Blocking Delegation, `request_arch_review`, `archReview`/`latestUserRequirements` 구조적 분리, `tool_deliveries`, 종료 순서, Arch 타임아웃 120초(범위 30~600), 단일 worker, DB 스키마·`user_version=6`, 실행 파일 이름 `esh-chat-mvp4` 모두 그대로야. 새 Tauri 명령·새 DB 쓰기·프런트 변경은 없어.
- 검증 (실제 출력):
  - `cargo test --bin esh-chat-mvp4` (프로젝트 상위 `.tools`의 cargo를 직접 실행) → `test result: ok. 83 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.86s` (기존 79 → 83, 순증 4). 신규: `engine::cli_commit_tests::a_finished_claude_turn_uses_the_same_rows_under_its_own_speaker`, `engine::claude_runner_tests::only_a_claude_session_with_a_runnable_lead_registration_resolves`, `engine::claude_runner_tests::a_registration_without_a_lead_role_or_a_present_file_does_not_run`, `engine::session_route_tests::a_claude_session_owns_only_its_own_cli` — 모두 `ok`. 재작성: `a_finished_opencode_turn_binds_the_external_session_and_stores_the_answer`(일반화된 함수 시그니처), `a_cancelled_turn_neither_moves_the_external_session_nor_stores_an_answer`(두 작성자 반복), `a_provider_without_a_branch_never_falls_through_to_codex`(`claude_code`는 이제 자기 분기를 가지므로 목록에서 빼고 나머지 provider로 가드 유지). 기존 `engine::*` 나머지(compaction·envelope·switch)도 모두 `ok`.
  - `node scripts/verify_external_threads.mjs` → `{"passed":true,"recordedRows":27,"scratchRowsHidden":9,"normalConversationsPreserved":true}`
  - `npm run build` → `✓ built in 3.30s` (`dist/assets/index-B_6vxOOa.js` 433.29 kB, `index-YBSTmYxd.css` 52.24 kB, `guiProbe-djswdxyz.js` 45.64 kB — 프런트를 건드리지 않아 해시 동일)
  - Rust 경고는 기존 2건(`agents.rs`의 `Authenticated`·`Required`·`Error`, `Ready` 미생성 variant)뿐이고 `engine.rs`·`claude.rs`에서 새 경고는 없어. `claude::run`이 이제 호출부를 가져 `claude.rs`의 미사용 항목은 `CONNECTION_KIND` 하나로 줄었어.
- 신규 테스트가 실제로 확인한 것: (1) 같은 `commit_lead_answer`가 `opencode`와 `claude_code` 각각에서 같은 행·같은 순서로 답변을 저장하되 **작성자를 섞지 않는다**는 것, (2) 취소된 턴은 두 작성자 모두에서 외부 세션을 옮기지 않고 답변도 남기지 않는다는 것, (3) `claude_runner`가 Codex 세션·등록 ID 없음·없는 등록·reviewer 역할·디스크에서 사라진 실행 파일을 각각 거절하고 정상 등록에서만 (등록 ID, 정규화 경로)를 돌려준다는 것, (4) `claude_code` 세션은 `claude_code` target만 통과하고 `codex`·`antigravity`·`opencode`·`gemini_cli`·빈 문자열은 거절되며, `gemini_cli`·`mock_agent`·`unknown`은 여전히 **모든** target이 거절된다는 것(가드 유지 확인).
- 이 검증의 경계(중요):
  - `claude_turn`은 **이번에도 실제로 실행되지 않았어.** 두 겹으로 막혀 있어: ① `claude_code` provider 세션을 만드는 명령이 없고 ② `submit`이 `target="claude_code"`를 `registered()` 목록에 없어서 거부해. 따라서 이번 변경은 "Claude Code 연결"이 아니라 **분기 자리와 순서 규칙**을 만든 것이야. 실제 `claude -p`는 여전히 승인 대기야.
  - `Engine`이 `AppHandle`을 들고 있어 `claude_turn`·`worker` 자체는 단위 테스트로 호출하지 못했어. 테스트는 `claude_turn`이 읽는 입력(세션 행·등록 행)과 판정 함수(`turn_route`)·마감 함수(`commit_lead_answer`)·해석 함수(`claude_runner`)를 검사한 것이고, 배선은 컴파일과 코드 확인으로만 검증했어.
  - `python scripts/verify_*.py`, WebView, 데스크톱 빌드는 실행하지 않았어(변경이 Rust 백엔드이고 실행 중인 앱을 건드리지 않으려고).
- 남은 문제:
  1. **승인 대기**: 실제 `claude -p` 1회로 stream-json 원문 캡처(외부 데이터 전송 + 계정 사용량). 이때까지 `parse_events`는 공개 문서 기준이야.
  2. 등록·세션 경로 미구현 — `ConnectionKind::ClaudeCodeCli`·`ClaudeCodeAdapter`·`registered()` 추가(`agents.rs`), `active_role`의 연결 매핑과 등록 시 `connection_kind` 부여(`registry.rs`), `create_claude_session`(`workspace.rs`), `create_session` 명령의 provider 분기(`main.rs`)가 남아 있어. **`claude_runner`가 `role`을 보는 이유가 여기야** — 어댑터가 등록되기 전에는 `active_role`이 `None`이라 그 값으로는 턴을 찾을 수 없어. 등록 조각이 들어오면 `active_role == Some("lead")` 검사로 좁혀야 해.
  3. `support` 표기 — 등록만 먼저 하면 `active_role`이 `Some("lead")`가 되어 Agents 화면이 `supported`로 올라가는데, 세션 경로가 없으면 실행할 수 없어 거짓 표기가 돼. 그래서 **세션 경로를 먼저** 넣고(등록이 없어 `support`는 `unsupported` 그대로) **등록을 나중에** 넣어야 해.
  4. 도구 정책 미확정 — `--permission-mode plan`이 대화 전용 계약을 실제로 지키는지 미검증.
  5. 1단계 잔여 4건(GUI 실행 확인, `tauri-plugin-single-instance` 내부 경합, 원본 세션 전역 잠금=승인 대기, 종료 중 거절의 WebView 재현)은 그대로 차단 상태야.
- 다음 작업: (b) Claude 세션 경로 — `workspace.rs`의 `create_claude_session`(provider `claude_code`, 등록 검사는 `role`/`installation` 기준)과 `main.rs` `create_session` 명령의 provider 분기. 이 조각은 등록이 없어 `support`를 바꾸지 않으므로 안전해. 그다음 (c) `agents.rs`+`registry.rs` 등록 조각에서 `claude_runner`의 검사를 `active_role`로 좁히고 `support`를 실행 경로와 일치시킬 것. 승인을 받으면 그 전에 실제 `claude -p` 캡처로 `parse_events`를 확정하는 편이 좋아.
- 실행 메모(이 환경): `scripts/rust.ps1` 대신 `.tools\cargo\bin\cargo.exe`에 `CARGO_HOME`/`RUSTUP_HOME`/`PATH`를 `.tools`로 맞춰 직접 실행할 것(스크립트는 cargo의 stderr 진행 메시지에서 끊겨 컴파일 오류가 없어도 exit 1). 테스트 모듈에서 `fn store()` 헬퍼를 같은 함수 안에서 두 번 부르면 지역 변수에 가려 `E0618`이 나므로 헬퍼 이름을 달리할 것.

## 세션 provider와 실행 분기의 일치를 허용 목록으로 판정 (2026-10-25, 3단계 세 번째 조각)

- 선택한 작업: 3단계(추가 CLI 어댑터) 다음 조각의 **전제 조건**. 지난 회차가 "Engine `claude_turn` 배선 전까지는 `submit`이 그 세션을 Codex 턴으로 보내지 않도록 provider 검사부터 닫아 둘 것"이라고 남긴 항목이야. `submit`의 기존 검사는 두 조합만 막는 블랙리스트(`opencode` 세션→비opencode, `codex` 세션→opencode)여서 `provider='claude_code'`인 세션에 `target='codex'`가 그대로 통과했고, `worker`의 마지막 분기가 `codex_turn`이라 **Codex가 답한 뒤 세션 에이전트가 답한 것처럼 저장**되는 경로가 열려 있었어. Claude 인스턴스를 등록하면 그 경로가 실제로 열리므로 등록보다 먼저 닫았어.
- 변경 파일: `mvp-app/src-tauri/src/engine.rs` 1개.
  - `TurnRoute { Codex, OpenCode, Arch }` + `turn_route(session, target)` 추가 — 세션 provider 기준 **허용 목록**이야. `codex`는 `codex`(Codex 턴)와 `antigravity`(Arch 질의, 위임 검토이지 두 번째 Lead가 아님)만, `opencode`는 `opencode`만 통과하고 **그 밖의 provider는 모든 대상이 거절**돼. `worker`의 마지막 분기가 Codex 턴이라는 사실에 기대지 않고, 분기가 없는 provider를 호출 전에 막아.
  - `compact_thread(session)` 추가 — 수동 압축은 Codex 스레드 작업이야. `provider != "codex"`면 `Codex 세션이 아니어서 컨텍스트를 압축할 수 없습니다`로 거절하고, 그다음에 기존과 같은 `thread_id` 검사를 해. OpenCode 세션은 `thread_id`에 **자기 CLI 세션 ID**가 들어 있어(12회차 `commit_opencode_answer` → `bind_codex_session`) 기존 코드처럼 `thread_id`만 보면 그 값을 Codex 압축에 넘기게 돼.
  - `submit`은 `turn_route(&saved, &target).is_none()`이면 기존과 같은 메시지(`이 세션의 에이전트와 메시지 대상이 다릅니다`)로 거절하고, `request_compact`는 `session.thread_id.as_deref().ok_or(...)`를 `compact_thread(session)?`로 교체했어(스레드 없음 메시지 동일).
- 동작 보존: 정상 조합 세 가지(`codex`→`codex`, `codex`→`antigravity`, `opencode`→`opencode`)는 이전과 똑같이 통과하고, 거절 문구도 그대로야. 달라진 것은 (1) `codex`/`opencode`가 아닌 provider 세션의 **모든** 대상이 거절되고, (2) 비Codex 세션의 압축 요청이 거절된다는 두 가지뿐이야. 그런 세션은 아직 만들 수 없어(`create_session`은 codex, `create_opencode_session`은 opencode) 살아 있는 동작 변화는 없어. Blocking Delegation, `request_arch_review`, `archReview`/`latestUserRequirements` 구조적 분리, `tool_deliveries`, 종료 순서, Arch 타임아웃 120초(범위 30~600), 단일 worker, DB 스키마·`user_version=6`, 실행 파일 이름 `esh-chat-mvp4` 모두 그대로야. 새 Tauri 명령·새 DB 쓰기·프런트 변경은 없어.
- 검증 (실제 출력):
  - `cargo test --bin esh-chat-mvp4` (프로젝트 상위 `.tools`의 cargo를 직접 실행) → `test result: ok. 79 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.86s` (기존 75 + 신규 4). 신규: `engine::session_route_tests::a_codex_session_owns_the_codex_turn_and_the_arch_question_channel`, `an_opencode_session_only_answers_its_own_cli`, `a_provider_without_a_branch_never_falls_through_to_codex`, `manual_compaction_is_a_codex_thread_operation_only` — 모두 `ok`.
  - `node scripts/verify_external_threads.mjs` → `{"passed":true,"recordedRows":27,"scratchRowsHidden":9,"normalConversationsPreserved":true}`
  - `npm run build` → `✓ built in 3.60s` (`dist/assets/index-B_6vxOOa.js` 433.29 kB, `index-YBSTmYxd.css` 52.24 kB, `guiProbe-djswdxyz.js` 45.64 kB — 프런트를 건드리지 않아 해시 동일)
  - Rust 경고는 기존 2건(`agents.rs`의 `Authenticated`·`Required`·`Error`, `Ready` 미사용 variant)뿐이고 `engine.rs`에서 새 경고는 없어.
- 신규 테스트가 실제로 확인한 것: (1) 실제 `chat_sessions` 행(codex)에서 `codex`→Codex 턴, `antigravity`→Arch, `opencode`·`claude_code`·`gemini_cli`·빈 문자열→거절, (2) 실제로 등록한 OpenCode 인스턴스로 만든 세션은 `opencode`만 통과하고 `codex`·`antigravity`·`claude_code`는 거절, (3) 세션 provider를 `claude_code`·`gemini_cli`·`mock_agent`·`unknown`으로 바꾼 각 경우에 **어떤 대상도 Codex 턴으로 떨어지지 않음**(등록 전에 필요한 가드), (4) 압축은 Codex 세션이면서 `bind_codex_session`으로 스레드가 있을 때만 스레드 ID를 돌려주고, OpenCode 세션에 `thread_id`를 넣어도 `Codex 세션이 아니어서`로 거절.
- 이 검증의 경계(중요): `Engine`은 `AppHandle`을 들고 있어 `submit`·`request_compact` **자체**를 단위 테스트로 호출하지 못했어. 테스트는 두 함수가 읽는 `Session` 행과 판정 함수(`turn_route`·`compact_thread`)를 검사한 것이고, 배선(어느 지점에서 부르는지)은 컴파일과 코드 확인으로만 검증했어. 실제 WebView에서의 거절 표시, `python scripts/verify_*.py`, 데스크톱 빌드는 실행하지 않았어(변경이 Rust 백엔드이고 실행 중인 앱을 건드리지 않으려고). 이 결과를 "Claude Code 연결 성공"이나 "어댑터 동작"으로 집계하지 않아 — Claude 쪽 실행 경로는 여전히 `claude.rs` 안에만 있고 등록·배선되지 않았어.
- 남은 문제:
  1. **승인 대기**: 실제 `claude -p` 1회로 stream-json 원문 캡처(외부 데이터 전송 + 계정 사용량). 이때까지 `claude.rs`의 이벤트 형식은 공개 문서 기준이야.
  2. 어댑터 미등록·미배선 — `ConnectionKind::ClaudeCodeCli`·`ClaudeCodeAdapter`, `probe_install`의 claude 경로, `register_agent_instance`의 `claude_code`→`claude_code_cli` 연결, `worker`의 Claude 분기, `create_session`의 provider 선택, `claude_turn`이 모두 남아 있어. 등록을 먼저 하면 `support`가 `supported`로 올라가는데 실행 경로가 없어 거짓 표기가 되므로 **배선 뒤에 등록**해야 해.
  3. 도구 정책 미확정 — `--permission-mode plan`이 대화 전용 계약을 실제로 지키는지 미검증.
  4. 1단계 잔여 4건(GUI 실행 확인, `tauri-plugin-single-instance` 내부 경합, 원본 세션 전역 잠금=승인 대기, 종료 중 거절의 WebView 재현)은 그대로 차단 상태야.
- 다음 작업: 이제 분기 없는 provider가 Codex로 새지 않으므로 `claude_turn` 배선을 다음 조각으로 진행할 수 있어. 순서는 (a) `worker`의 target 분기를 `turn_route`로 바꿔 Claude 분기 자리를 만들고 `claude_turn`(세션의 등록 인스턴스 → `claude::run` → 답변 저장)을 추가, (b) `create_session`에서 Claude 인스턴스를 provider로 선택, (c) 그 뒤 `ClaudeCodeAdapter` 등록과 `claude_code_cli` 매핑. 승인을 받으면 (a) 전에 실제 `claude -p` 캡처로 `parse_events`를 확정하는 편이 좋아.
- 실행 메모(이 환경): `scripts/rust.ps1`은 `$ErrorActionPreference='Stop'`이라 cargo의 stderr 진행 메시지에서 끊겨 컴파일 오류가 없어도 exit 1로 보여. `.tools\cargo\bin\cargo.exe`를 `CARGO_HOME`/`RUSTUP_HOME`/`PATH`를 `.tools`로 맞춰 직접 실행할 것.

## Claude Code 턴 실행 경로 — 한 턴 = 한 프로세스, 취소·시간 초과 시 트리 종료 (2026-10-25, 3단계 두 번째 조각)

- 선택한 작업: 3단계(추가 CLI 어댑터)의 다음 조각. 지난 회차는 프로토콜 골격만 세우고 "실행 경로가 없는데 `supported`로 승격하지 않으려고" 어댑터를 등록하지 않았어. 등록(`ClaudeCodeAdapter` + `claude_code_cli` 매핑 + `claude_turn` 배선)은 `agents.rs`·`registry.rs`·`engine.rs`·`store.rs`를 함께 건드려 이번 회차의 1~3파일 범위를 넘고, 등록보다 **먼저 실행 경로가 있어야** `support` 표기가 정직해져. 그래서 이번에는 실행 경로 한 조각만 만들었어.
- 변경 파일: `mvp-app/src-tauri/src/claude.rs` 1개.
  - `turn_args(existing, model, prompt)` 추가 — `command_args` 뒤에 프롬프트를 **마지막 위치 인자**로 붙여. 설치본 `--help` 1행이 `Usage: claude [options] [command] [prompt]`, 7행이 `prompt  Your prompt`로 위치 인자를 선언하고 있어 그 형태를 그대로 써. 프롬프트가 플래그 값 자리에 끼어들지 않아.
  - `run(path, work_dir, existing, model, prompt, stop)` 추가 — `opencode::run`과 같은 형태야. 실행 파일이 파일이 아니면 시작 전에 거절하고, 빈 요청도 프로세스를 띄우기 전에 거절해. `current_dir(work_dir)`, stdin null, stdout/stderr 파이프, `kill_on_drop`, 창 숨김.
  - `collect(command, expected, stop, limit)` 분리 — 캡처·대기·판정만 담당하는 내부 함수야. `stop.changed()`나 180초 시간 초과에서 `cli::stop_tree`로 **프로세스 트리 전체**를 끝낸 뒤 오류를 돌려줘 고아 프로세스가 남지 않아. stdout 4,194,305 / stderr 65,537 바이트 상한을 넘으면 실패. 종료 코드가 0이 아니면 코드만 알리고 **stderr 본문은 호출자에게 전달하지 않아**(내부 맥락 유출 방지). 성공하면 `parse_events`로 분류.
- 동작 보존: `command_args`·`parse_events`·`Answer`·`CONNECTION_KIND`는 값·시그니처를 바꾸지 않았어. `main.rs`의 `mod claude;` 한 줄도 그대로고 새 모듈·새 Tauri 명령·새 DB 쓰기·스키마 변경은 없어. Blocking Delegation, `request_arch_review`, `archReview`/`latestUserRequirements` 분리, `tool_deliveries`, 종료 순서, Arch 타임아웃 120초(범위 30~600), 단일 worker, `user_version=6`, 실행 파일 이름 `esh-chat-mvp4` 모두 그대로야.
- **`support` 표기는 이번에도 바뀌지 않았어.** `run`이 아직 어디에서도 호출되지 않으므로(모듈 단위 `#![allow(dead_code)]` 유지) `registered()`·`active_role`·`probe_agents()`·Agents 화면·`snapshot.agents` 값은 이전과 같고, 등록된 Claude 인스턴스는 여전히 `unsupported`/`unavailable`이야. 실행 경로가 생겼다는 이유만으로 `supported`로 올리지 않았어 — Engine 배선이 없으면 실제 턴이 만들어지지 않기 때문이야.
- 검증 (실제 출력):
  - `cargo test --bin esh-chat-mvp4` (프로젝트 상위 `.tools`의 cargo를 직접 실행) → `test result: ok. 75 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.25s` (기존 70 + 신규 5). 신규: `claude::tests::the_prompt_is_the_trailing_positional_argument`, `a_stub_stream_is_captured_and_classified_through_the_real_process_path`, `a_failed_exit_reports_the_code_without_leaking_stderr`, `a_cancelled_or_slow_turn_stops_the_child_instead_of_waiting`, `a_missing_cli_or_an_empty_request_fails_before_any_process_starts` — 모두 `ok`. 기존 `claude::tests::*` 7개도 모두 `ok`.
  - `node scripts/verify_external_threads.mjs` → `{"passed":true,"recordedRows":27,"scratchRowsHidden":9,"normalConversationsPreserved":true}`
  - `npm run build` → `✓ built in 2.66s` (`dist/assets/index-B_6vxOOa.js` 433.29 kB, `index-YBSTmYxd.css` 52.24 kB, `guiProbe-djswdxyz.js` 45.64 kB — 프런트를 건드리지 않아 해시 동일)
  - Rust 경고는 기존 2건(`agents.rs`의 `Authenticated`·`Required`·`Error`, `Ready` 미사용 variant)뿐이고 `claude.rs`에서 새 경고는 없어.
- 신규 테스트가 실제로 확인한 것: (1) `turn_args`의 마지막 원소가 프롬프트이고 앞부분이 `command_args`와 같으며 프롬프트가 플래그 자리에 없다는 것, (2) 스텁 자식 프로세스(powershell)가 실제로 띄운 NDJSON을 캡처해 `stub answer`로 분류하고 기대 세션과 다르면 `세션 ID가 일치하지 않습니다`로 실패한다는 것, (3) 종료 코드 3이면 `종료 코드`를 포함한 오류가 나오고 스텁이 stderr에 쓴 `secret-context-value`는 메시지에 없다는 것, (4) 취소 신호와 200ms 제한에서 각각 `중단`·`시간 초과`로 끝나고 자식이 종료된다는 것, (5) 없는 실행 파일과 공백 프롬프트는 프로세스를 띄우기 전에 거절된다는 것(플레이스홀더 exe는 실행되지 않음).
- 이 검증의 경계(중요):
  - 프로세스 배관(스폰·캡처·종료 코드·취소·시간 초과)은 **스텁 자식 프로세스**로 검증한 것이고, 설치된 `claude.exe`를 실제로 실행한 결과가 아니야. 실제 `claude -p` 1회는 프롬프트가 외부 API로 나가고 계정 사용량을 쓰므로 **여전히 승인 대기**야. 따라서 이 경로를 "Claude Code 연결 성공"으로 집계하지 않아.
  - 스트림 이벤트 형식은 지난 회차와 같이 공개 headless/SDK 문서 기준이며 실제 출력으로 확정되지 않았어.
  - `python scripts/verify_*.py`와 데스크톱 빌드는 실행하지 않았어(변경이 Rust 모듈 내부이고 실행 중인 앱을 건드리지 않으려고). WebView·GUI 확인도 하지 않았어.
- 남은 문제:
  1. **승인 대기**: 실제 `claude -p` 1회로 stream-json 원문을 캡처해 `parse_events`와 `run`의 종료·출력 형태를 실제 형식으로 확정하는 일(외부 데이터 전송 + 계정 사용량).
  2. 어댑터 미등록·미배선 — `ConnectionKind::ClaudeCodeCli`·`ClaudeCodeAdapter`(descriptor), `register_agent_instance`의 `claude_code`→`claude_code_cli` 연결, `active_role` 매핑, 세션 provider(`create_session`)와 Engine `claude_turn` 분기가 모두 남아 있어. 4개 파일에 걸쳐 있어 다음 조각에서 등록·매핑과 배선을 나눠 진행해야 해.
  3. 도구 정책 미확정 — `--permission-mode plan`은 설치본이 선언한 모드 중 보수적 선택일 뿐, 대화 전용 계약을 실제로 지키는지 검증되지 않았어. `--permission-prompts none`·`--disallowedTools` 조합도 실제 실행 확인 뒤에 정해야 해.
  4. 1단계 잔여 4건(GUI 실행 확인, `tauri-plugin-single-instance` 내부 경합, 원본 세션 전역 잠금, 종료 중 거절의 WebView 재현)은 그대로 차단 상태야.
- 다음 작업: (a) 승인 시 `claude -p` 1회 캡처로 파서·실행 형태 확정, (b) 승인 없이는 `ConnectionKind::ClaudeCodeCli` + `ClaudeCodeAdapter` descriptor + `registry.rs`의 `claude_code_cli` 연결/`active_role` 매핑을 한 조각으로 하되, Engine `claude_turn` 배선 전까지는 `submit`이 그 세션을 Codex 턴으로 보내지 않도록 provider 검사부터 닫아 둘 것.
- 실행 메모(이 환경): `scripts/rust.ps1`은 `$ErrorActionPreference='Stop'`이라 cargo가 stderr로 쓰는 진행 메시지에서 `NativeCommandError`로 끊겨 **실제 컴파일 오류가 없어도 exit 1**로 보여. `.tools\cargo\bin\cargo.exe`를 직접 실행하면 정상 출력·종료 코드를 얻을 수 있어.

## Claude Code 호출 계약·stream-json 분류 골격 (2026-10-25, 3단계 첫 조각)

- 선택한 작업: 3단계(추가 CLI 어댑터) 첫 조각 — Claude Code Lead 어댑터의 **비파괴 probe + 프로토콜 골격**. 직전 결정대로 1단계 항목은 더 캐지 않았어. 이번 조각은 **어댑터 등록·세션 경로·Engine 배선 없이** 프로토콜 정의만 세우는 단계야.
- 먼저 확인한 사실: `C:\Users\서승우\.local\bin\claude.exe`(293,227,168 bytes, 2026-10-20 18:12)가 있고 `~/.claude`도 있어. `claude --version` → `2.1.304 (Claude Code)`. `claude --help`로 플래그를 확인했고 **추측하지 않았어**.
- 변경 파일:
  - `mvp-app/src-tauri/src/claude.rs`(신규) — `CONNECTION_KIND = "claude_code_cli"`, `command_args(existing, model)`, `parse_events(output, expected)`, `Answer{session_id,text}`, 회귀 테스트 7개. 모듈 상단에 `#![allow(dead_code)]`를 두고 "아직 등록·실행되지 않는 준비 단계"임을 주석으로 명시해 경고 없이 컴파일돼.
  - `mvp-app/src-tauri/fixtures/claude-cli-2.1.304-help.txt`(신규) — 이 PC의 설치본이 실제로 출력한 `--help` 전문(313줄, 22,319 bytes)을 그대로 보존. `mvp-app/evidence/`는 `.gitignore` 대상이라 **새 클론에서 컴파일이 깨지므로** 추적되는 `src-tauri/fixtures/`에 두고 `include_str!`로 컴파일 시점에 읽어.
  - `mvp-app/src-tauri/src/main.rs` — `mod claude;` 한 줄 추가.
- 계약 내용:
  - `command_args`는 `-p --output-format stream-json --verbose --permission-mode plan`을 기본으로 하고, 세션이 있으면 `--resume <id>`, 모델을 고르면 `--model <m>`을 덧붙여. 이 6개 플래그와 `stream-json`·`plan` 값은 모두 설치본 `--help`에 실제로 선언된 것만 사용해.
  - `parse_events`는 NDJSON 한 줄씩 읽어 `session_id`가 줄마다 같아야 하고(기대 세션과 다르면 오류), `assistant` 블록의 `tool_use`가 나오면 **답변하지 않고 실패**하며, 마지막 `result`가 정확히 한 번 있어야 해. 답변 텍스트는 `result.result`를 우선하고 비어 있으면 `assistant`의 `text` 블록을 이어 붙여. `is_error=true`면 오류. 알 수 없는 이벤트 종류와 CRLF는 무시해 최신 CLI가 새 이벤트를 보내도 턴이 깨지지 않아.
- 검증 (실제 출력):
  - `npm test` → `test result: ok. 70 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.72s` (기존 63 + 신규 7). 신규: `claude::tests::the_invocation_contract_only_uses_flags_the_installed_cli_declares`, `a_first_turn_resumes_nothing_and_uses_the_default_model`, `the_result_line_is_the_answer_and_the_session_must_not_change`, `an_assistant_text_block_is_only_the_fallback_for_the_answer`, `an_error_result_or_a_tool_call_fails_instead_of_answering`, `a_truncated_or_duplicated_stream_is_rejected`, `unknown_event_types_and_crlf_do_not_break_a_valid_turn` — 모두 `ok`.
  - `node scripts/verify_external_threads.mjs` → `{"passed":true,"recordedRows":27,"scratchRowsHidden":9,"normalConversationsPreserved":true}`
  - `npm run build` → `✓ built in 3.37s` (`dist/assets/index-B_6vxOOa.js` 433.29 kB, `index-YBSTmYxd.css` 52.24 kB, `guiProbe-djswdxyz.js` 45.64 kB — 프런트를 건드리지 않아 해시 동일)
  - Rust 경고는 기존 3건(`agents.rs`의 `Authenticated`·`Required`·`Error`, `Ready` 미사용 variant)과 cargo 증분 lock 삭제 경고 1건뿐이고 `claude.rs`에서 새 경고는 없어.
- 이 검증의 경계(중요):
  - `--help` 대조 테스트는 **실제 설치본이 선언한 인터페이스**에 대한 근거야(자기 검사 아님). 반면 **이벤트 형식은 공개 headless/SDK 문서 기준으로 작성했고 이 PC의 실제 `claude -p` 출력으로 확인하지 않았어.** 따라서 이 모듈을 "동작하는 연결"이나 "실제 CLI 연결 성공"으로 집계하지 않아. 어댑터가 등록되지 않았으므로 `probe_agents()`·`Agents` 화면·`active_role`·`support` 값은 전부 이전과 같고, 등록된 Claude 인스턴스는 여전히 `unsupported`/`unavailable`이야(실행 경로가 없는데 `supported`로 승격하지 않으려고 이번에 등록하지 않았어).
  - 실제 `claude -p` 1회 캡처는 프롬프트가 외부 API로 나가고 사용자 계정 사용량을 쓰므로 **승인 대기**로 남겨. 이 환경 샌드박스는 `claude --help` 실행 중에도 `reg.exe`를 차단했다고 보고했어(출력·종료코드는 정상). 실제 턴 검증이 이 샌드박스에서 온전히 재현될지도 함께 확인해야 해.
- 남은 문제:
  1. **승인 대기**: 실제 `claude -p` 1회로 stream-json 원문을 `evidence/`에 캡처해 파서를 실제 형식으로 확정하는 일(외부 데이터 전송 + 계정 사용량).
  2. 어댑터 미등록 — `ClaudeCodeAdapter`/`LeadAdapter`/`LeadSession` 구현, `register_agent_instance`의 `claude_code`→`claude_code_cli` 매핑, 세션 생성 경로(`create_session` provider), Engine `claude_turn` 배선이 모두 남아 있어. 이걸 하려면 `support`가 실제 실행 경로와 일치하도록 등록·세션·배선을 한 묶음으로 해야 해.
  3. 도구 정책 미확정 — `--permission-mode plan`은 설치본이 선언한 모드 중 보수적 선택일 뿐, 대화 전용 계약을 실제로 지키는지 검증되지 않았어. `--permission-prompts none`·`--disallowedTools` 조합은 실제 실행으로 확인한 뒤 정해야 해.
  4. 1단계 잔여 4건(GUI 실행 확인, `tauri-plugin-single-instance` 내부 경합, 원본 세션 전역 잠금, 종료 중 거절의 WebView 재현)은 그대로 차단 상태야.
- 다음 작업: (a) 승인을 받으면 `claude -p` 1회 캡처로 `parse_events`를 실제 형식에 맞추고, (b) 그 결과를 근거로 `ClaudeCodeAdapter` 등록 + `claude_code_cli` 매핑 + `claude_turn` 배선을 한 조각으로 진행. 승인 없이는 (b)만 먼저 하되 `support` 표기를 실행 경로와 함께 맞출 것.

## 단계 게이트 결정 — 1단계 반복 중단, 3단계로 이동 (2026-10-25)

- 배경: 1~14회차 자동 실행이 계속 1단계(기존 기능 안정화) 안에서 작은 정합성 결함을 찾아 고치는 데 머물렀어. 사용자가 이 반복을 지적했고, 앞으로 1단계 항목을 더 캐지 않고 3단계(추가 CLI 어댑터)로 넘어가.
- **1단계를 "완료"로 기록하지 않아.** 아래 4건은 닫히지 않은 채로 남아 있어.
- 1단계 현재 상태: 단위·Mock으로 닫을 수 있는 계약은 회귀 검사로 덮였어 — 종료 접수 차단(`ensure_accepting`, 등록·해제·모델 목록 포함), 무음 성공 제거(`tool_state`·`status`·`confirm_tool`·`bind_codex_session`·`bind_messages`→`deliver_requirements`), 리뷰 봉투의 단일 버전 읽기, 공개 답변 스트림·늦은 턴 이벤트 차단, OpenCode 턴 마감 순서, 등록 해제·고아 행 정리. 근거는 아래 각 항목의 `npm test` 결과야.
- 남은 4건과 닫히지 않은 이유:
  1. GUI 실행 확인 — 자동화 환경에서 WebView2가 초기화되지 않아(8회차: 창은 뜨지만 `#root` 자식 0, `gui-probe.json` 없음) 증거를 만들 수 없어. 사용자 화면 확인이 필요해.
  2. `tauri-plugin-single-instance` 내부 경합 — 플러그인 내부 타이밍 문제라 이 저장소에서 재현 수단이 없어.
  3. 원본 Codex 세션 전역 실행 잠금 — 실행 구조 변경이라 **승인 대기**야. 착수하지 않았어.
  4. 종료 중 거절의 실제 WebView 재현 — 1번과 같은 이유로 막혀.
- 2단계(Mock Agent 검증) 상태: Mock은 공개 답변, 늦은·중복 응답, 잘못된 요청 ID, 열린 도구 호출 1회 응답, 취소, 연결 재개를 스크립트로 재현하고 `npm test`에 포함돼 있어. 남은 "단일 worker 직렬성"은 `engine.rs`의 `worker()`가 **한 루프에서 한 job을 끝까지 `await`한 뒤 다음 `recv`를 하는 구조**라(engine.rs:428~454) 직렬성이 코드 형태로 고정돼 있고, `Engine`이 `AppHandle`을 들고 있어 테스트용으로 재작성하면 루프 모양을 검사하는 자기 검사가 돼. 별도 테스트를 만들지 않고 이 구조 확인으로 남겨. Mock 통과를 실제 CLI 연결 성공으로 집계하지 않는 원칙은 그대로야.
- 다음 작업: 3단계(추가 CLI 어댑터). 첫 조각은 Claude Code(`claude` CLI 설치 확인됨, `~/.claude` 존재) Lead 어댑터의 **비파괴 probe + 프로토콜 골격**이야. 실제 연결 성공은 실제 `claude -p` 응답과 로그로만 기록하고, Mock·단위 결과로 대체하지 않아. 인증·설정 파일은 읽거나 바꾸지 않아.
- 이 결정은 문서 기록이며 코드·DB·스키마·실행 파일 이름은 바꾸지 않았어.

## 리뷰 요구사항 전달의 결합·전달 단일 쓰기 (2026-10-25)

- 선택한 작업: 1단계(기존 기능 안정화) 잔여 항목 중 지난 회차가 남긴 "`bind_messages` 무음 성공". `engine.rs`의 리뷰 봉투 경로는 봉투를 Codex에 보낸 뒤 `bind_messages(ids, thread, turn)`로 대기 요구사항을 리뷰 턴에 묶고, **그다음** `delivery(ids, "delivered")`로 전달 완료를 표시했어. 두 문장이 서로 다른 트랜잭션이라 사이에서 실패하면(프로세스 종료·디스크 오류) 메시지는 특정 턴에 묶인 채 `delivery='queued'`로 남고, `pending_requirements`는 `queued`만 고르므로 **다음 리뷰에서 같은 요구사항을 Codex에 다시 전달**하게 돼. 게다가 `bind_messages`는 갱신 행 수를 보지 않아 없는 메시지 ID도 조용히 성공으로 처리하고 `meta.revision`만 올렸어. `tool_state`·`status`·`confirm_tool`·`bind_codex_session`에서 이미 제거한 무음 성공 유형의 남은 하나야.
- 변경 파일:
  - `mvp-app/src-tauri/src/store.rs` — `bind_messages`를 `deliver_requirements(ids, thread, turn)`로 대체. 빈 `thread`·`turn`을 거절하고, 전달할 요구사항이 없으면 아무것도 쓰지 않고 `Ok(())`(revision 불변). 한 트랜잭션 안에서 각 ID에 `thread_id`·`turn_id`·`delivery='delivered'`를 함께 쓰고, 갱신 행이 정확히 1이 아니면 `요구사항 메시지를 찾을 수 없습니다` 오류로 **배치 전체를 롤백**해. `meta.revision` 증가와 커밋도 같은 트랜잭션. 회귀 테스트 1개 추가.
  - `mvp-app/src-tauri/src/engine.rs` — 봉투 전달 후 두 호출을 `deliver_requirements` 한 번으로 교체. `rpc.turn_id` 없음 검사(`Codex 턴이 없습니다`)와 `reply_to_tool_call` → 저장 → `emit` 순서는 그대로.
  - `mvp-app/src-tauri/src/characterization_tests.rs` — 기존 3곳의 `bind_messages` + `delivery("delivered")` 조합을 `deliver_requirements` 한 번으로 교체(같은 최종 상태).
- 동작 보존: 봉투의 키 이름·구조(`kind`, `archReview`, `latestUserRequirements{conversationVersion,messages}`, `reviewIsStale`, `processingOrder`)와 `processingOrder` 순서, `@arch` 제외 규칙, 봉투 전송 후 저장·`emit` 순서는 그대로야. Blocking Delegation(같은 RPC 호출에 결과 반환), `request_arch_review`, `archReview`/`latestUserRequirements` 구조적 분리, `tool_deliveries` 상태 전이, 종료 순서와 접수 차단, Arch 타임아웃 120초(범위 30~600), 단일 worker, DB 스키마·`user_version=6`, 사용자 메시지·세션·컨텍스트는 변경하지 않았어. 정상 경로에서 쓰는 행·값(`thread_id`/`turn_id`/`delivered`)은 이전과 같아. 달라진 것은 (1) 두 쓰기가 한 트랜잭션, (2) 없는 ID가 오류, (3) 대기 요구사항이 0건일 때 revision을 올리지 않음(이전에는 두 번 올리고 빈 `emit`을 했음) 세 가지야.
- 검증 (실제 출력):
  - `npm test` → `test result: ok. 63 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.49s` (기존 62 + 신규 1, `store::tests::delivering_requirements_binds_and_marks_delivered_in_one_write ... ok`)
  - `node scripts/verify_external_threads.mjs` → `{"passed":true,"recordedRows":27,"scratchRowsHidden":9,"normalConversationsPreserved":true}`
  - `npm run build` → `✓ built in 2.58s` (`dist/assets/index-B_6vxOOa.js` 433.29 kB, `index-YBSTmYxd.css` 52.24 kB, `guiProbe-djswdxyz.js` 45.64 kB — 프런트를 건드리지 않아 해시 동일)
  - Rust 경고는 기존 2건(`agents.rs`의 `Authenticated`·`Required`·`Error`, `Ready` 미사용 variant)뿐이고 새 경고는 없어.
- 신규 테스트 내용: 요구사항 2건을 한 번에 전달하면 `revision`이 **1만** 증가하고 두 메시지가 모두 `delivered` + 같은 `thread_id`/`turn_id`가 되며 `pending_requirements`가 비는지, 세 번째 요구사항과 없는 ID를 함께 넘기면 오류가 나고 **세 번째 메시지가 `queued`·`thread_id`/`turn_id` 없음으로 그대로**이며 `revision`도 변하지 않고 여전히 대기 1건으로 남는지(부분 쓰기 없음), 빈 `turn`·빈 `thread`는 거절되고 빈 배치는 `Ok`이면서 `revision`을 올리지 않는지 확인해. 저장소 계층만 검사해.
- 이번 결과는 Rust 단위·분류 회귀 검사야. 실제 Codex App Server가 `request_arch_review`를 호출해 이 봉투를 받는 경로, 실제 WebView의 전달 상태 표시, 실제 프로세스가 두 쓰기 사이에서 죽는 상황은 확인하지 않았어. `python scripts/verify_*.py`와 데스크톱 빌드는 실행하지 않았어(변경이 Rust 백엔드이고 실행 중인 앱을 건드리지 않으려고).
- 남은 문제: (a) 1단계 잔여 4건(GUI 실행 확인, `tauri-plugin-single-instance` 내부 경합, 원본 세션 전역 잠금=**승인 대기**, 종료 중 거절의 WebView 재현)은 그대로 차단 상태야. (b) OpenCode 턴에서 CLI가 답을 만든 뒤 확정이 실패하면 그 답변 텍스트는 여전히 저장되지 않아(중단된 턴의 답변을 남길지 여부는 UX 결정이 필요해). (c) `delivery`는 여전히 갱신 행 수를 확인하지 않는 무음 성공 함수야 — 남은 호출부는 `engine.rs`의 `@arch` 직접 질문 경로 한 곳이고 `let _ =`로 결과를 버리는 의도적 best-effort야(전달 실패해도 리뷰는 진행). 이 경로를 엄격하게 바꾸려면 `@arch` 질문이 실패했을 때 사용자에게 어떻게 보여줄지 UX 결정이 먼저 필요해.
- 다음 작업: (b)의 답변 보존 여부 결정, 또는 `@arch` 직접 질문 경로의 `delivery` 실패 처리 방침 결정. 2단계 Mock 체크리스트는 "단일 worker 직렬성"만 `AppHandle` 의존이라 단위 검사로 닫을 수 없는 상태야.

## 외부 세션 연결의 무음 성공 제거 (2026-10-25)

- 선택한 작업: 1단계(기존 기능 안정화) 잔여 항목 중 지난 회차가 남긴 "`bind_codex_session` 무음 성공". 이 함수는 `UPDATE chat_sessions SET thread_id=?,codex_session_id=? WHERE id=?`의 갱신 행 수를 보지 않고 `meta.revision`을 올린 뒤 `Ok(())`를 돌려줬어. 세션 행이 없으면 아무것도 저장되지 않았는데 호출부는 성공으로 처리했고, 반대로 정상 저장 때도 `thread_id`가 snapshot에 포함되는데 revision을 같은 트랜잭션에서 올리지 않아 두 문장 사이에 실패하면 화면이 이전 thread를 계속 보여줄 수 있었어. `tool_state`·`status`·`confirm_tool`에서 이미 제거한 무음 성공 유형의 마지막 남은 하나야.
- 변경 파일:
  - `mvp-app/src-tauri/src/workspace.rs` — `bind_codex_session`이 빈 `id`·`thread`·`session`을 거절하고, 갱신을 `db.transaction()` 안에서 수행해 갱신 행이 정확히 1이 아니면 `세션을 찾을 수 없습니다` 오류를 반환해. `meta.revision` 증가는 같은 트랜잭션에서 갱신이 성립한 뒤에만 일어나고, 커밋도 그 안에서 끝나. 회귀 테스트 1개 추가.
- 동작 보존: `chat_sessions` 스키마·`user_version=6`, `thread_id`/`codex_session_id`에 쓰는 값, `tool_deliveries`, Blocking Delegation, `request_arch_review`, `archReview`/`latestUserRequirements` 구조적 분리, 종료 순서와 접수 차단, Arch 타임아웃 120초(범위 30~600), 단일 worker는 변경하지 않았어. 두 호출부(`engine.rs`의 Codex 연결 확립 직후, `commit_opencode_answer`의 OpenCode 마감)는 모두 직전에 `store.session(...)`으로 세션 존재를 확인한 뒤 호출하므로 정상 경로의 쓰기 값·순서는 그대로야. 바뀐 것은 "행이 없을 때 조용히 성공" 대신 오류를 돌려주고, 성공 시 `meta.revision`이 함께 오른다는 점뿐이야.
- 검증 (실제 출력):
  - `npm test` → `test result: ok. 62 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.73s` (기존 61 + 신규 1, `workspace::tests::binding_an_external_session_reports_misses_instead_of_storing_nothing ... ok`)
  - `node scripts/verify_external_threads.mjs` → `{"passed":true,"recordedRows":27,"scratchRowsHidden":9,"normalConversationsPreserved":true}`
  - `npm run build` → `✓ built in 3.53s` (`dist/assets/index-B_6vxOOa.js` 433.29 kB, `index-YBSTmYxd.css` 52.24 kB, `guiProbe-djswdxyz.js` 45.64 kB)
  - Rust 경고는 기존 3건(`agents.rs`의 미사용 variant)과 cargo의 증분 디렉터리 lock 삭제 경고 1건뿐이고 `workspace.rs` 관련 새 경고는 없어.
- 신규 테스트 내용: 세션을 만들고 바인딩하면 `thread_id`·`session_id`가 저장되고 snapshot `revision`이 증가하는지, 없는 세션 ID로 호출하면 오류가 나며 revision이 그대로이고 기존 바인딩이 유지되는지, 빈 thread·공백 session ID도 같은 방식으로 거절되는지, 재개방 후에도 바인딩이 남는지를 확인해. 저장소 계층만 검사해.
- 이번 결과는 Rust 단위·분류 회귀 검사야. 실제 Codex App Server 연결에서 `bind_codex_session`이 호출되는 경로, 실제 OpenCode 턴 마감, 실제 WebView의 thread 표시 갱신은 확인하지 않았어. `python scripts/verify_*.py`와 데스크톱 빌드는 실행하지 않았어(변경이 Rust 백엔드이고 실행 중인 앱을 건드리지 않으려고).
- 남은 문제: (a) 1단계 잔여 4건(GUI 실행 확인, `tauri-plugin-single-instance` 내부 경합, 원본 세션 전역 잠금=**승인 대기**, 종료 중 거절의 WebView 재현)은 그대로 차단 상태야. (b) OpenCode 턴에서 CLI가 답을 만든 뒤 확정이 실패하면 그 답변 텍스트는 여전히 저장되지 않아(중단된 턴의 답변을 남길지 여부는 UX 결정이 필요해). (c) `bind_messages`는 여전히 갱신 행 수를 확인하지 않아 같은 무음 성공 유형이 남아 있어(호출 경로상 방금 만든 메시지 ID만 넘어와 실제 실패 가능성은 낮아).
- 다음 작업: (b)의 답변 보존 여부를 결정하거나, `bind_messages`의 무음 성공을 같은 방식으로 정리. 2단계 Mock 체크리스트는 "단일 worker 직렬성"만 `AppHandle` 의존이라 단위 검사로 닫을 수 없는 상태야.

## OpenCode 턴 확정 순서 수정 — 취소된 턴이 외부 세션을 옮기지 않게 (2026-10-25)

- 선택한 작업: 1단계(기존 기능 안정화) 범위의 OpenCode Lead 턴 마감 순서. `engine.rs`의 `opencode_turn`은 CLI 답변을 받은 뒤 **먼저** `bind_codex_session`으로 `chat_sessions.thread_id`·`codex_session_id`를 새 외부 세션으로 옮기고, **그다음** `confirm_turn_start`로 사용자 메시지의 턴 결합을 확정했어. `confirm_turn_start`는 `delivery='queued'`인 행만 갱신하고 아니면 오류를 돌려주므로, CLI가 답하는 사이에 사용자가 중단해 메시지가 `cancelled`가 되면 (1) 외부 세션 포인터는 이미 새 세션으로 이동하고 (2) 답변은 저장되지 않아. 그 결과 다음 턴은 로컬 기록에 없는 답변을 포함한 외부 세션을 이어쓰게 돼 로컬 기록과 CLI 맥락이 어긋났어.
- 변경 파일:
  - `mvp-app/src-tauri/src/engine.rs` — 마감 3단계를 `commit_opencode_answer(store, session_id, user_id, answer) -> Result<()>`로 분리하고 순서를 `confirm_turn_start` → `bind_codex_session` → 답변 저장으로 바꿨어. `opencode_turn`은 이 함수를 호출하고 이어서 `state(..., "completed")`를 수행해. 성공 경로에서 쓰는 행과 값은 이전과 같아(같은 메시지 2건, `main`/`answer`/`delivered`, 같은 `thread_id`·`codex_session_id`). 회귀 테스트 2개 추가.
- 동작 보존: Blocking Delegation, `request_arch_review`, `archReview`/`latestUserRequirements` 구조적 분리, `tool_deliveries` 상태 전이, 종료 순서, Arch 타임아웃 120초(범위 30~600), 단일 worker, DB 스키마·`user_version=6`, 사용자 메시지·세션·컨텍스트는 변경하지 않았어. 바뀐 것은 취소로 확정이 실패할 때 외부 세션 포인터를 건드리지 않는다는 점뿐이야.
- 검증 (실제 출력):
  - `npm test` → `test result: ok. 61 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.46s` (기존 59 + 신규 2, `engine::opencode_commit_tests::a_finished_turn_binds_the_external_session_and_stores_the_answer ... ok`, `engine::opencode_commit_tests::a_cancelled_turn_neither_moves_the_external_session_nor_stores_an_answer ... ok`)
  - `node scripts/verify_external_threads.mjs` → `{"passed":true,"recordedRows":27,"scratchRowsHidden":9,"normalConversationsPreserved":true}`
  - `npm run build` → `✓ built in 2.38s` (`dist/assets/index-B_6vxOOa.js` 433.29 kB, `index-YBSTmYxd.css` 52.24 kB, `guiProbe-djswdxyz.js` 45.64 kB)
  - 경고는 기존 2건(`agents.rs`의 `Authentication`·`Availability` 미사용 variant)뿐이고 새 경고는 없어.
- 신규 테스트 내용: (1) 정상 턴에서 `chat_sessions.thread_id`·`codex_session_id`가 외부 세션 ID로 저장되고, 사용자 메시지가 `delivered`+턴 결합되며, 답변이 `opencode`/`main`/`delivered`로 1건 추가되는지 확인해. (2) `cancel_queued`로 메시지를 `cancelled`로 만든 뒤 마감하면 오류가 나고 `thread_id`는 `None`, `codex_session_id`는 빈 값 그대로이며 답변 메시지가 생기지 않는지 확인해. 저장소·순수 함수 계층만 검사해.
- 이번 결과는 Rust 단위 회귀 검사야. 실제 OpenCode CLI가 답하는 도중 중단을 걸어 이 경합을 재현한 것은 아니고, `opencode_turn`의 배선(어느 시점에 함수를 부르는지)은 컴파일과 코드 확인으로만 검증했어. 실제 WebView·`python scripts/verify_*.py`·데스크톱 빌드는 실행하지 않았어(변경이 Rust 백엔드이고 실행 중인 앱을 건드리지 않으려고).
- 지난 회차가 제안한 "Mock에 프로세스 종료 불명확 시나리오 추가"는 **하지 않았어**. 그 경로는 이미 `cli.rs`의 실제 stdio 상대 검사(`mock_stdio_keeps_one_tool_reply_on_the_original_rpc_id`: 쓰기 실패 후 `call-2` 행이 `unknown`, 재전송 없음)와 `characterization_tests::review_records_and_tool_delivery_survive_restart_without_replaying_open_call`(재시작 후 `unknown`)이 더 강한 형태로 덮고 있어. Mock에 같은 전이를 흉내 내면 Mock 자신의 동작을 검사하는 중복이 되고 실제 어댑터 증거로 오인될 수 있어서 추가하지 않았어.
- 남은 문제: (a) 1단계 잔여 4건(GUI 실행 확인, `tauri-plugin-single-instance` 내부 경합, 원본 세션 전역 잠금=**승인 대기**, 종료 중 거절의 WebView 재현)은 그대로 차단 상태야. (b) OpenCode 턴에서 CLI가 답을 만든 뒤 확정이 실패하면 그 답변 텍스트는 여전히 저장되지 않아(중단된 턴의 답변을 남길지 여부는 UX 결정이 필요해). (c) `bind_codex_session`은 세션 행이 없어도 조용히 성공하는 `UPDATE`야(호출 경로상 유효한 세션만 오지만 무음 성공 유형이 남아 있어).
- 다음 작업: (a)의 4건 중 사용자 환경 확인이 필요한 항목 정리, 또는 (b) 중단된 OpenCode 턴의 답변 보존 여부 결정. 2단계 Mock 체크리스트는 "단일 worker 직렬성"만 `AppHandle` 의존이라 단위 검사로 닫을 수 없는 상태야.

## 공개 답변 스트림의 Mock 검증과 늦은 턴 이벤트 차단 (2026-10-25)

- 선택한 작업: 1단계 회귀 표의 "Codex 사용자 턴" 중 **공개 답변 스트림 경로**, 그리고 2단계 Mock Agent 체크리스트의 "공개 답변"·"늦은 응답" 항목. `engine.rs`의 `codex_turn`은 `item/started`·`item/agentMessage/delta`·`item/completed`를 인라인으로 처리하면서 스레드/턴 일치를 분기마다 따로 비교했어. 이 판정이 `Engine`(AppHandle) 안에 있어 단위 테스트가 닿지 않았고, 그래서 "늦은 턴 이벤트가 현재 답변에 섞이지 않는다"는 계약에는 회귀 검사가 없었어. Mock Agent도 공개 답변 이벤트를 만들지 못해 그 경로를 스크립트로 재현할 수 없었어.
- 변경 파일:
  - `mvp-app/src-tauri/src/engine.rs` — `apply_stream_event(streams, store, event, thread, turn, session) -> Result<bool>` 추가. `threadId`/`turnId`가 현재 턴과 다르면 `Ok(false)`로 버리고, `agentMessage` 항목만 초안에 반영하며, 저장된 대화가 바뀌었는지(`changed`)를 돌려줘. `codex_turn`의 세 분기가 이 함수를 호출하도록 교체했고 `emit` 조건과 `dynamicToolCall` 확인 순서는 그대로야.
  - `mvp-app/src-tauri/src/mock_agent.rs` — `MockAgent::with_public_answer`(기본값 false) 추가. 실제 App Server가 보내는 순서(`item/started` → `item/agentMessage/delta` → 도구 호출 → `item/completed` → `turn/completed`)를 스크립트로 만들고, 회귀 테스트 1개 추가.
- 동작 보존: Blocking Delegation, `request_arch_review`, `archReview`/`latestUserRequirements` 구조적 분리, `tool_deliveries`, 종료 순서, Arch 타임아웃 120초(범위 30~600), `tool_state`·`status` 반환 계약, DB 스키마·`user_version=6`, 사용자 메시지·세션·컨텍스트는 변경하지 않았어. 이벤트 종류별 처리 결과(초안 시작/델타 누적/완료 확정, `agentMessage` 이외 항목 무시, 턴 불일치 시 무시)는 이전과 같고, 이전 코드가 `turn_id=None`에서 `json!(null)` 비교로 통과하던 경로는 이제 빈 턴으로 처리돼 불일치로 떨어져(패닉 가능 경로 제거) 더 안전한 쪽으로만 달라.
- 검증 (실제 출력):
  - `npm test` → `test result: ok. 59 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.44s` (기존 58 + 신규 1, `mock_agent::tests::scripted_public_answer_reaches_the_stream_and_a_late_turn_is_dropped ... ok`)
  - `node scripts/verify_external_threads.mjs` → `{"passed":true,"recordedRows":27,"scratchRowsHidden":9,"normalConversationsPreserved":true}`
  - `npm run build` → `✓ built in 2.46s` (`dist/assets/index-B_6vxOOa.js` 433.29 kB, `index-YBSTmYxd.css` 52.24 kB, `guiProbe-djswdxyz.js` 45.64 kB)
  - 경고는 기존 2건(`agents.rs`의 `Authentication`·`Availability` 미사용 variant)뿐이고 새 경고는 없어.
- 신규 테스트 내용: Mock Lead가 스크립트한 이벤트를 `apply_stream_event`로 순서대로 넣어 (1) `item/started`는 저장을 바꾸지 않고, (2) 델타가 `main` 채널 `streaming` 메시지를 만들고, (3) 도구 호출 이벤트는 공개 답변으로 취급되지 않아 메시지를 만들지 않고, (4) `item/completed`가 권위 텍스트로 확정해 `mock public answer`/`delivered` 한 건만 남는지 확인해. 이어서 2턴이 현재일 때 1턴의 늦은 델타·완료 이벤트를 넣어 저장이 늘지 않고 `late` 텍스트가 섞이지 않는지, 같은 2턴의 정상 델타는 두 번째 메시지(`mock public `, `streaming`)로 들어가는지 확인해.
- 이번 결과는 Rust 단위·Mock 회귀 검사야. 실제 Codex App Server가 이 이벤트를 보내는 경로, 실제 WebView의 답변 스트리밍 표시는 확인하지 않았어. `codex_turn`의 배선(어느 분기에서 함수를 부르고 언제 `emit`하는지)은 컴파일과 코드 확인으로만 검증했어. `python scripts/verify_*.py`와 데스크톱 빌드는 실행하지 않았어(변경이 Rust 백엔드이고 실행 중인 앱을 건드리지 않으려고).
- 1단계 잔여 4건(GUI 실행 확인, 플러그인 내부 경합, 원본 세션 전역 잠금=**승인 대기**, 종료 중 거절의 WebView 재현)은 지난 회차와 같이 이 환경에서 완결 불가라 그대로 차단 상태야. 이번 작업은 그 4건과 독립적인 회귀 보강이라 1단계 범위 안에 있어.
- 남은 문제: (a) 위 4건 그대로. (b) Mock은 공개 답변·늦은 이벤트만 재현하고, 인증 불명·프로세스 종료 불명확·`tool_deliveries` 상태 전이는 여전히 `cli.rs`/`store.rs` 테스트가 담당해 Mock 경로로는 검증되지 않아. (c) `Streams`의 40ms flush 스로틀 때문에 델타 반환값은 타이밍에 따라 달라져, 테스트는 반환값 대신 저장 결과로 판정해.
- 다음 작업: Mock에 "프로세스 종료 불명확(exit unknown)" 시나리오를 추가해 `tool_deliveries`가 `unknown`으로 남는 경로를 Mock 경로에서도 확인하거나, 남은 1단계 차단 항목 중 사용자 환경 확인이 필요한 것을 정리.

## 1단계 자동화 가능 항목 완료 확인과 잔여 항목 차단 상태 (2026-10-25)

- 선택한 작업: 로드맵 1단계(기존 기능 안정화)의 자동화 환경에서 완결 가능한 항목이 모두 끝났는지 현재 트리를 재확인. 1~9회차까지 종료 접수 차단 확장, 무음 성공 제거(`tool_state`/`status`/`confirm_tool`), 등록·해제 snapshot 반영, 고아 `agents` 행 정리, 배지·목록 불일치, 리뷰 응답 봉투의 요구사항 단일 읽기까지 단위 검증 가능한 결함을 모두 고쳤음. 이번 회차는 코드를 더 안 고치고 세 가지 검증 명령으로 기준선을 다시 돌림(이전 실행이 중단된 뒤 시간이 흘러 재확인 가치가 있음).
- 변경 파일: 없음(코드 변경 없음). `docs/PROGRESS.md` 항목만 추가.
- 검증 (실제 출력):
  - `npm test` → `test result: ok. 58 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.69s`
  - `node scripts/verify_external_threads.mjs` → `{"passed":true,"recordedRows":27,"scratchRowsHidden":9,"normalConversationsPreserved":true}`
  - `npm run build` → `✓ built in 3.15s` (`dist/assets/index-B_6vxOOa.js` 433.29 kB, `index-YBSTmYxd.css` 52.24 kB, `guiProbe-djswdxyz.js` 45.64 kB)
- 이번 결과는 1~9회차가 쌓은 단위·분류 회귀의 재통과야. 신규 코드 없이 기준선만 확인했으므로 새 동작 증거는 없어.
- 1단계 잔여 항목 4건은 모두 이 자동화 환경에서 완결 불가라 차단 상태로 둠:
  1. **GUI 실행 확인**(중복 실행 시 기존 창 전환·정상 종료 후 `msedgewebview2` 잔존 여부) — 이 환경의 WebView2 초기화 실패로 창이 뜨지 않아 증거 생성 불가. → 사용자 실제 환경에서 확인 필요.
  2. **플러그인 내부 경합**(`tauri-plugin-single-instance` 2.5.4의 `FindWindowW` 경합) — 플러그인 내부 한계. → 별도 IPC 보강은 요구사항(플러그인 사용)과 충돌해 미착수.
  3. **원본 세션 전역 잠금**(동시 사용 차단) — 구조 변경 + DB 스키마 변경 후보. → 안전 정책상 **승인 대기**, 임의 착수 안 함.
  4. **"종료 중 거절"의 실제 WebView 재현 경로** — WebView2 초기화 실패로 증거 생성 불가(직전 회차 시도 후 전부 revert). → 사용자 환경 확인 필요.
- 앞 단계(1단계)가 완전히 검증되지 않아 2단계(Mock Agent 검증)로 넘어가지 않음. 단 Mock Agent 자체는 이미 `#[cfg(test)]`로 8개 계약 테스트를 갖추고 `npm test`에 포함돼 있어 추가 검증 공백은 아님.
- 다음 작업: 위 4건 중 사용자가 (a) 실제 GUI 환경에서 1·4번을 확인하거나, (b) 전역 잠금 설계를 승인하면 해당 항목을 착수. 승인·환경 없이는 1단계에서 무리하게 진행할 코드 변경은 없음.

## 리뷰 응답 봉투의 요구사항 버전·목록 단일 읽기 (2026-10-25)

- 선택한 작업: 로드맵 1단계(기존 기능 안정화) 잔여 항목 중 Blocking Delegation 응답 봉투의 최신 요구사항 정합성. `engine.rs`는 `latestUserRequirements.messages`를 `store.snapshot()`에서 뽑고, `latestUserRequirements.conversationVersion`과 `reviewIsStale`을 각각 **다른** `session_version()` 호출로 읽었어. 사용자 입력은 Tauri command 스레드에서 별도로 커밋되므로 그 사이에 새 메시지가 들어오면 봉투의 두 절반이 서로 다른 대화 버전을 가리킬 수 있었어(버전 N+1인데 `messages`가 비어 있거나, 반대로 `messages`에 들어간 요구사항이 `reviewIsStale` 계산에 반영되지 않음). `archReview`와 `latestUserRequirements`를 구조적으로 분리하고 `reviewIsStale`을 보존한다는 계약을 코드 한 곳에서 보장하도록 고쳤어.
- 변경 파일: `mvp-app/src-tauri/src/store.rs`(`pending_requirements(session) -> Result<(i64, Vec<Message>)>` 추가 — 한 번의 DB 락 안에서 세션 버전과 `queued` 사용자 메시지를 함께 읽고 `substr(content,1,6)!='@arch '`로 `@arch` 직접 질문을 제외, 회귀 테스트 1개), `mvp-app/src-tauri/src/engine.rs`(봉투 조립을 순수 함수 `review_envelope(review, review_version, requirements_version, pending)`로 분리하고 `codex_turn`이 이를 사용, 회귀 테스트 1개).
- 동작 보존: 봉투의 키 이름·구조(`kind`, `archReview`, `latestUserRequirements{conversationVersion,messages}`, `reviewIsStale`, `processingOrder`)와 `processingOrder` 순서, `@arch` 제외 규칙, 봉투 전송 후 `bind_messages` + `delivery("delivered")` 순서는 그대로야. Blocking Delegation(같은 RPC 호출에 결과 반환), 열린 도구 호출 유지, `tool_deliveries` 상태 전이, 종료 순서, Arch 타임아웃 120초(범위 30~600), DB 스키마·`user_version=6`, 사용자 메시지·세션·컨텍스트는 변경하지 않았어. 바뀐 것은 두 값이 같은 읽기에서 나온다는 점뿐이고, 경합이 없을 때의 결과 값은 이전과 같아. `store.snapshot()` 전체 읽기가 사라져 메시지 전체를 매 리뷰마다 복사하던 비용도 없어졌어.
- 검증 (실제 출력):
  - `npm test` → `test result: ok. 58 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.51s` (기존 56 + 신규 2, `store::tests::pending_requirements_share_one_version_and_skip_arch_and_foreign_sessions ... ok`, `engine::envelope_tests::review_and_latest_requirements_stay_separate_with_an_explicit_staleness_flag ... ok`)
  - `node scripts/verify_external_threads.mjs` → `{"passed":true,"recordedRows":27,"scratchRowsHidden":9,"normalConversationsPreserved":true}`
  - `npm run build` → `✓ built in 2.31s` (`dist/assets/index-B_6vxOOa.js` 433.29 kB, `index-YBSTmYxd.css` 52.24 kB, `guiProbe-djswdxyz.js` 45.64 kB)
- 신규 테스트 내용: `pending_requirements`는 세션 버전과 `queued` 사용자 메시지만 돌려주고 `@arch` 직접 질문과 다른 세션의 대기 입력을 제외하며, `delivered`로 바꾸면 목록이 비고 세션 버전은 그대로인지(전달은 `chat_sessions.version`을 올리지 않음), 없는 세션은 오류인지를 확인해. 봉투 테스트는 버전이 같으면 `reviewIsStale=false`이고 `latestUserRequirements`가 비어 있어도 키가 존재하는지, 버전이 다르면 `reviewIsStale=true`이면서 늦은 요구사항이 `latestUserRequirements.messages`에 `queued` 상태로 들어가고 `archReview`에는 `messages`·`latestUserRequirements`가 섞이지 않는지, `processingOrder`가 `["latestUserRequirements","archReview"]`인지를 확인해. 저장소·순수 함수 계층만 검사해.
- 이번 결과는 Rust 단위 회귀 검사야. 실제 Codex App Server가 `request_arch_review`를 호출해 이 봉투를 받는 경로, 실제 WebView의 협업 패널 표시는 확인하지 않았어. `python scripts/verify_*.py`와 데스크톱 빌드는 실행하지 않았어(변경이 Rust 백엔드이고 실행 중인 앱을 건드리지 않기 위해). 두 번의 읽기 사이에 메시지가 끼어드는 경합 자체를 테스트로 재현한 것은 아니고, 값이 한 읽기에서 나온다는 구조와 봉투 조립 결과만 검사했어.
- 남은 문제: (a) 중복 실행 수정의 실행 확인(두 번째 실행이 기존 창을 앞으로 가져오는지, 정상 종료 후 `msedgewebview2` 잔존 여부)은 여전히 사용자 환경 확인이 필요해. (b) `tauri-plugin-single-instance` 2.5.4의 `FindWindowW` 경합은 플러그인 내부 한계로 남아 있어. (c) 원본 세션 전역 잠금 설계는 **승인 대기**. (d) "종료 중 거절"의 실제 WebView 재현 경로는 이 자동화 환경의 WebView2 초기화 실패로 만들지 못한 상태 그대로야.
- 다음 작업: `verify_live.py`로 실제 Arch 리뷰 봉투 수신을 한 번 확인하거나(실제 CLI 사용량·원본 저장소 부작용 있음), 남은 1단계 항목 정리 후 2단계(Mock Agent 검증) 체크리스트의 미검증 항목(`reviewIsStale`·`latestUserRequirements`는 이번에 저장소·봉투 계층까지 확보)을 실제 어댑터 경로로 좁혀 확인.

## 중복 실행 시 검은 창·렌더링 실패 대응 (2026-10-25)

- 선택한 작업: 사용자가 보고한 "여러 번 실행하면 창이 검게 나오거나 렌더링이 안 되고, 강제 종료를 반복하면 `%LOCALAPPDATA%\local.esh.conversation\EBWebView` 프로필이 오염된다" 문제. 코드로 확인한 결과 **단일 인스턴스 장치가 전혀 없었어**(`grep -rn "single_instance\|CreateMutex"` 결과 0건). 두 번째 프로세스가 같은 앱 식별자로 WebView2 user data 폴더를 동시에 잡고, 강제 종료로 남은 `msedgewebview2` 자식이 폴더를 쥔 채 새 인스턴스가 뜨는 경로가 그대로 열려 있었어.
- 원인 판정: (1) 중복 실행 = **맞음**. 두 프로세스가 동일 프로필을 동시에 연다. (2) `tauri.conf.json`의 `visible:false` = **재현 안 됨**. 현재 설정은 `{"label":"main","title":"ESH · AI 협업","width":1600,"height":900,"minWidth":900,"minHeight":600,"center":true,"zoomHotkeysEnabled":true}`로 `visible` 키 자체가 없어. 다만 설정 실수로 창이 숨는 경로를 코드로 막으라는 요구에 따라 `setup`에서 `show()`를 명시 호출. (3) 이전 프로세스의 WebView2 자식이 폴더를 잠금 = (1)의 결과이자 강제 종료의 잔재. 단일 인스턴스로 새 인스턴스가 프로필을 열기 전에 종료시키는 방식으로 함께 막았어.
- 변경 파일: `mvp-app/src-tauri/Cargo.toml`(`tauri-plugin-single-instance = "2"` 추가, 해석된 버전 2.5.4), `mvp-app/src-tauri/src/main.rs`(빌더의 **첫 플러그인**으로 단일 인스턴스 등록 — 두 번째 실행은 콜백에서 `unminimize`/`show`/`set_focus`만 수행하고 종료, `setup`에서 `window_state::restore` 뒤 `window.show()` 명시 호출), `mvp-app/src-tauri/Cargo.lock`.
- 동작 근거(코드 확인, 실행 검증 아님): Tauri 2.12.4 `app.rs`에서 `initialize_plugins()`가 `build()`의 마지막 단계(2607행)이고 설정 창 생성·사용자 `setup`은 `run()`의 `setup(self)`(1529행)에서 일어나. 즉 두 번째 인스턴스는 **창 생성과 `Engine::start`(SQLite `Store::open`) 이전에** 종료되므로 기존 창·세션·DB를 건드리지 않아. 플러그인 문서도 "The second instance never reaches `tauri::Builder::run`"이라고 명시해. 단일 인스턴스 잠금은 앱 식별자 `local.esh.conversation` 기반 named mutex이고, `RunEvent::Exit`에서 해제돼. 우리 코드에는 `std::process::exit` 호출이 없고 종료는 전부 `app.exit(0)`(main.rs 233·312행)이라 뮤텍스가 남지 않아.
- 종료 정리 확인: 기존 경로는 `CloseRequested` → `prevent_close` → `closing` 중복 방지 → `engine.shutdown()`(접수 차단 → worker 중지 → 최대 25초 대기) → 실패 시 `codex` 상태 `unknown` + `checkpoint` → `app.exit(0)`. 종료 확인 실패를 성공으로 바꾸는 코드는 없어. Codex·OpenCode·Arch 자식 프로세스는 모두 `kill_on_drop(true)`(cli.rs 128·545·630, opencode.rs 38·86)로 스폰돼 프로세스 종료 시 함께 정리되고, WebView2 자식은 `app.exit(0)`에서 Tauri가 웹뷰를 드롭하며 끝나. **이 정리 동작 자체를 실제 실행으로 관측하지는 않았어**(아래 검증 한계).
- 검증 (실제 출력):
  - `npm test` → `test result: ok. 56 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.68s`
  - `node scripts/verify_external_threads.mjs` → `{"passed":true,"recordedRows":27,"scratchRowsHidden":9,"normalConversationsPreserved":true}`
  - `npm run build:desktop` → 프런트 `✓ built`(`dist/assets/index-B_6vxOOa.js` 433.29 kB, `guiProbe-djswdxyz.js` 45.64 kB) + Rust `Finished \`dev\` profile ... in 18.65s`, 산출물 `mvp-app/src-tauri/target/debug/esh-chat-mvp4.exe` (16,614,400 bytes, 2026-10-25 15:56). 실행 파일 이름은 `esh-chat-mvp4` 하나만 갱신했고 새 이름의 exe는 만들지 않았어. 빌드 경고 2건(`agents.rs`의 `Availability::Ready`·`Authentication` 미사용 variant)은 이번 변경과 무관한 기존 경고야.
- 검증 한계(중요): 사용자 지시에 따라 GUI를 자동화 환경에서 띄우지 않았어. 따라서 "두 번째 실행이 새 창을 만들지 않고 기존 창을 앞으로 가져오는지"와 "종료 후 WebView2 자식이 남지 않는지"는 **실행으로 확인하지 못했어**. 참고로 이번 회차 초반에 GUI 프로브를 띄웠을 때 창은 `ESH · AI 협업` 제목으로 생성됐지만 devtools로 확인한 `document.getElementById('root').childElementCount`가 `0`, `document.body.innerText`가 빈 문자열이었고 `gui-probe.json`도 만들어지지 않았어(당시 실행 기록: `mvp-app/evidence/shutdown-20261025-153310/`). 이 증상은 앱 코드 결함이 아니라 자동화 환경의 WebView2 초기화 실패로 판단해.
- 이번 회차에 함께 시도했다가 되돌린 것: "종료 중 명령 거절"을 실제 WebView에서 재현하는 GUI 프로브(`ESH_GUI_PROBE=shutdown` + `begin_shutdown_probe` 명령 + `scripts/verify_shutdown.py`). 위 WebView2 초기화 문제로 증거를 만들 수 없어 커밋하지 않고 전부 되돌렸어. 그 검증 공백은 그대로 남아 있어.
- 이번 수정으로 새로 생기는 동작(코드 확인, 실행 검증 아님): 첫 인스턴스가 **종료 중**일 때(창을 닫은 뒤 `engine.shutdown()`이 끝날 때까지, 최악 25초) 다시 실행하면 두 번째 프로세스는 뮤텍스가 아직 잠겨 있어 플러그인 콜백만 수행하고 종료돼. 이 구간에서는 **새 창이 뜨지 않고, 곧 첫 인스턴스도 사라져** 사용자에게는 "실행했는데 아무 일도 안 일어남"으로 보여. 수정 전에는 이 구간에 두 번째 앱이 떴으니 새로 생긴 제약이야. 정상 종료(수 초)에서는 거의 겪지 않지만, 확인할 때 헷갈릴 수 있어 기록해 둬. 이 구간까지 없애려면 두 번째 실행이 첫 인스턴스 종료를 기다렸다가 인계받아야 하는데, 그건 뮤텍스 소유권을 플러그인 밖에서 직접 관리해야 해서 요구사항(플러그인 사용)과 충돌해 이번에는 하지 않았어.
- 남은 문제: (a) 위 두 가지 실행 확인은 사용자 환경에서 필요해. (b) `tauri-plugin-single-instance` 2.5.4의 Windows 구현은 뮤텍스를 잡은 뒤 첫 인스턴스의 메시지 창을 `FindWindowW`로 찾지 못하면(동시 실행 경합) 두 번째도 그대로 진행해. 이 경합은 플러그인 내부 한계로, 이번 변경에서 별도 IPC를 덧붙이지 않았어. (c) 원본 세션 전역 잠금 설계는 여전히 **승인 대기**.
- 다음 작업: 사용자가 실제 환경에서 2회 연속 실행·정상 종료 후 `msedgewebview2` 잔존 여부를 확인. 문제가 남으면 (b) 경합 보강을 별도 항목으로 설계.

## 에이전트 배지 개수와 목록 불일치 정리 (2026-10-25)

- 선택한 작업: 로드맵 1단계(기존 기능 안정화) 잔여 항목 중 앞선 회차가 남긴 "에이전트 등록·해제와 `agents` 행의 불일치"의 남은 절반. `main.tsx`는 `에이전트` 배지 개수를 `snapshot.agents.length`로 표시하고, `AgentsView`는 builtin 두 개와 `agent_instances`의 모든 사용자 등록을 목록에 그려. 그런데 `register_agent_instance`는 OpenCode Lead(`opencode_cli_json`)일 때만 `agents` 행을 만들었어. 그래서 Claude Code·Gemini CLI처럼 실행 연결이 없는 등록을 추가하면 목록은 3개인데 배지는 2로 남았어. 지난 회차가 "인스턴스가 없는 `agents` 행"을 지워 배지가 부풀던 문제를 고쳤으니, 이번에는 반대 방향(등록이 있는데 상태 행이 없는 경우)을 맞췄어.
- 변경 파일: `mvp-app/src-tauri/src/registry.rs`(`register_agent_instance`가 모든 등록에 `agents` 행을 한 트랜잭션 안에서 생성, OpenCode Lead는 기존처럼 `idle`/`대기 중`, 실행 연결이 없는 등록은 `unknown`/`실행 연결이 설정되지 않았습니다`, 회귀 테스트 1개), `mvp-app/src-tauri/src/store.rs`(`Store::open`에서 `agent_instances`에 있으나 `agents` 행이 없는 인스턴스를 `INSERT OR IGNORE`로 보충. 기존 상태는 덮어쓰지 않음).
- 동작 보존: `agents`·`agent_instances`·`chat_sessions` 스키마와 `user_version=6`, 사용자 메시지·세션·컨텍스트, `tool_deliveries`, Blocking Delegation, `request_arch_review`, archReview/latestUserRequirements 분리, 종료 순서와 접수 차단, Arch 타임아웃 120초(범위 30~600)는 변경하지 않았어. 새 DB 마이그레이션은 없어. 지난 회차의 "인스턴스 없는 `agents` 행 삭제"와 재시작 시 `unknown` 복구 갱신은 그대로야. 실행 연결이 없는 등록은 실행 상태를 주장하지 않도록 `idle`이 아니라 `unknown`으로 두고, 목록 라벨은 기존처럼 `지원` 값으로 `미지원`을 표시해.
- 검증 (실제 출력):
  - `npm test` → `test result: ok. 56 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.48s` (기존 55 + 신규 1, `registry::tests::every_registration_owns_one_status_row_so_badge_and_list_agree ... ok`)
  - `node scripts/verify_external_threads.mjs` → `{"passed":true,"recordedRows":27,"scratchRowsHidden":9,"normalConversationsPreserved":true}`
  - `npm run build` → `✓ built in 2.46s` (`dist/assets/index-B_6vxOOa.js` 433.29 kB, `index-YBSTmYxd.css` 52.24 kB, `guiProbe-djswdxyz.js` 45.64 kB)
- 신규 테스트 내용: 등록 전 `agents` 2개 → Claude Code(미지원) 등록 후 3개이고 해당 행이 정확히 1개이며 상태가 `unknown`인지, 그 행을 지워 이전 빌드 상태를 흉내 낸 뒤 재개방하면 `Store::open`이 `unknown`/`실행 연결이 설정되지 않았습니다`로 보충하고 builtin `codex`는 `idle`을 유지하는지, 한 번 더 재개방해도 3개로 같은지(멱등), 등록 해제 후 2개로 돌아가는지를 확인해. 저장소 계층만 검사해.
- 이번 결과는 Rust 단위·분류 회귀 검사야. 실제 Tauri WebView에서 배지 숫자가 3으로 바뀌고 목록 항목 수와 같은지는 확인하지 않았어. `python scripts/verify_*.py`도 실행하지 않았어. 이 worktree의 `mvp-app/src-tauri/target/debug/esh-chat-mvp4.exe`는 15:25에 사용자 요청으로 빌드된 이전 트리 산출물이라 이번 변경을 포함하지 않아. 이번 변경은 Rust 백엔드에 있어 화면 확인에는 재빌드가 필요하고, 방금 만들어진 실행 파일을 덮어쓰지 않으려고 데스크톱 빌드를 실행하지 않았어.
- 남은 문제: 원본 세션 동시 사용을 막는 전역 잠금은 여전히 없어(구조 변경이라 **승인 대기**). "종료 중 거절"을 실제 WebView에서 재현하는 검증 경로도 아직 없어. 실행 연결이 없는 등록의 `unknown` 상태는 `AgentsView` 상세에서 `상태 확인 필요`로 보이지만 이 화면 문구는 WebView에서 확인하지 않았어.
- 다음 작업: "종료 중 거절"을 실제 WebView에서 재현하는 검증 경로 마련, 또는 원본 세션 전역 잠금 설계 승인 요청.

## 시작 시 고아 `agents` 행 정리 (2026-10-25)

- 선택한 작업: 로드맵 1단계(기존 기능 안정화) 잔여 항목 중 앞선 회차가 남긴 "시작 시 고아 `agents` 행 정리". 등록 해제는 이제 인스턴스 행과 상태 행을 한 트랜잭션에서 지우지만, 그 수정 이전 빌드가 남긴 상태 행은 지울 경로가 없어 그대로 남았어. `agents`는 레지스트리에서 파생된 실행 상태이고 사용자 데이터가 아니며, 남은 행은 화면 목록에는 나타나지 않는데도 `main.tsx`의 배지 개수(`snapshot.agents.length`)만 부풀렸어.
- 변경 파일: `mvp-app/src-tauri/src/store.rs`(`Store::open`에서 `agent_instances`에 없는 `agents` 행 삭제, `codex`·`antigravity` 고정 행은 예외, 회귀 테스트 1개).
- 동작 보존: `agents`·`agent_instances`·`chat_sessions` 스키마와 사용자 메시지·세션·컨텍스트, `tool_deliveries`, Blocking Delegation, `request_arch_review`, archReview/latestUserRequirements 분리, 종료 순서와 접수 차단, Arch 타임아웃 120초(범위 30~600)는 변경하지 않았어. 새 DB 마이그레이션은 없고 `user_version`도 그대로야. 삭제 대상은 "인스턴스가 이미 없는 실행 상태 행"뿐이라 등록된 인스턴스와 builtin 두 행은 유지돼. 정리 후에도 기존 `unknown` 복구 갱신과 `revision` 증가는 그대로 수행돼.
- 검증 (실제 출력):
  - `npm test` → `test result: ok. 55 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.68s` (기존 54 + 신규 1, `store::tests::open_drops_status_rows_whose_instance_is_gone ... ok`)
  - `node scripts/verify_external_threads.mjs` → `{"passed":true,"recordedRows":27,"scratchRowsHidden":9,"normalConversationsPreserved":true}`
  - `npm run build` → `✓ built in 2.84s` (`dist/assets/index-B_6vxOOa.js` 433.29 kB, `index-YBSTmYxd.css` 52.24 kB)
- 신규 테스트 내용: 인스턴스가 있는 `inst-keep` 상태 행과 인스턴스가 없는 `inst-orphan` 상태 행을 함께 넣고 재개방하면 `agents`가 `antigravity`, `codex`, `inst-keep` 세 행만 남는지, 메시지가 새로 생기지 않는지, 한 번 더 재개방해도 결과가 같은지(멱등)를 확인해. 저장소 계층만 검사해.
- 이번 결과는 Rust 단위·분류 회귀 검사야. 실제 Tauri WebView에서 배지 개수가 줄어드는지, 이미 고아 행이 있는 사용자 DB에서 재시작 후 화면이 어떻게 보이는지는 확인하지 않았어. `python scripts/verify_*.py`는 실행하지 않았어.
- 데스크톱 빌드는 같은 날 사용자 요청으로 별도 실행했어 (`npm run build:desktop`) → `✓ built in 3.05s`(프런트) + `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 36.07s`, 산출물 `mvp-app/src-tauri/target/debug/esh-chat-mvp4.exe` (16,402,944 bytes, 2026-10-25 15:25). 실행 대상 이름은 `esh-chat-mvp4` 그대로야. 빌드 경고 2건(`agents.rs`의 `Authentication`·`Availability` 미사용 variant)은 이번 변경과 무관한 기존 경고야. 빌드는 컴파일 성공 증거이며 실제 WebView 동작 검증은 아니야.
- 남은 문제: 원본 세션 동시 사용을 막는 전역 잠금은 여전히 없어. 이 항목은 실행 잠금 정책·충돌 시 동작을 정해야 하는 구조 변경이라 **승인 대기**로 두고 임의로 착수하지 않았어. "종료 중 거절"을 실제 WebView에서 재현하는 검증 경로도 아직 없어.
- 다음 작업: "종료 중 거절"을 실제 WebView에서 재현하는 검증 경로 마련, 또는 원본 세션 전역 잠금 설계 승인 요청.

## 등록·해제 결과의 snapshot 반영 (2026-10-25)

- 선택한 작업: 로드맵 1단계(기존 기능 안정화) 잔여 항목 중 앞선 회차가 남긴 "등록 해제 시 `agents` 행 정리" 확인. 정리 자체는 `registry.rs`의 `unregister_agent_instance`가 이미 `DELETE FROM agents`로 수행하고 있었지만, 그 삭제가 `agent_instances` 삭제와 별개 문장이라 두 번째 삭제가 실패하거나 그 사이에 프로세스가 죽으면 인스턴스 없는 `agents` 행이 남았어. 게다가 두 store 함수 모두 `meta.revision`을 올리지 않고 Tauri 명령도 `emit()`을 호출하지 않아, 열려 있는 WebView는 삭제된 인스턴스의 상태와 `에이전트` 배지 개수를 다음 무관한 이벤트까지 계속 보여줬어(`main.tsx`의 `snapshot.agents.length`, `AgentsView.tsx`의 `snapshot.agents.find`).
- 변경 파일: `mvp-app/src-tauri/src/registry.rs`(`register_agent_instance`·`unregister_agent_instance`를 트랜잭션으로 묶고 `meta.revision` 증가, 회귀 테스트 1개), `mvp-app/src-tauri/src/main.rs`(두 명령이 성공 후 `engine.emit()` 호출).
- 동작 보존: `agent_instances`·`agents` 스키마, 기존 두 builtin 행, `chat_sessions` 참조 검사(사용 중 세션이 있으면 해제 거부), `tool_deliveries`, Blocking Delegation, `request_arch_review`, archReview/latestUserRequirements 분리, 종료 순서와 접수 차단, Arch 타임아웃 120초(범위 30~600)는 변경하지 않았어. 새 DB 마이그레이션은 없고 사용자 메시지·세션·컨텍스트는 건드리지 않아. `emit()`은 실패해도 등록·해제 자체를 되돌리지 않도록 반환값만 전달해.
- 검증 (실제 출력):
  - `npm test` → `test result: ok. 54 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.70s` (기존 53 + 신규 1, `registry::tests::registration_and_removal_are_visible_to_snapshot_consumers ... ok`)
  - `node scripts/verify_external_threads.mjs` → `{"passed":true,"recordedRows":27,"scratchRowsHidden":9,"normalConversationsPreserved":true}`
  - `npm run build` → `✓ built in 2.76s` (`dist/assets/index-B_6vxOOa.js` 433.29 kB, `index-YBSTmYxd.css` 52.24 kB)
- 신규 테스트 내용: OpenCode Lead 등록 직후 `revision`이 증가하고 `agents`에 `idle` 행이 생겨 개수가 2→3이 되는지, `status(..., "running", ...)`가 `true`인지, 해제 후 `revision`이 다시 증가하고 해당 `agents` 행이 사라져 개수가 2로 돌아가며 `agent_instances`에도 남지 않고 실행 파일은 그대로인지를 확인해. 저장소 계층만 검사하고 Tauri 명령 배선과 `emit()` 전달은 컴파일과 코드 확인으로만 검증했어.
- 이번 결과는 Rust 단위·분류 회귀 검사야. 실제 Tauri WebView에서 등록·해제 직후 배지와 상세 패널이 즉시 갱신되는지, 삭제된 인스턴스가 선택된 상태에서 패널이 닫히는지는 확인하지 않았어. `python scripts/verify_*.py`와 데스크톱 빌드는 실행하지 않았어.
- 남은 문제: 이 수정 이전에 이미 생긴 고아 `agents` 행은 시작 시 정리 경로가 없어 그대로 남아(배지 개수에만 영향) 있어. 원본 세션 동시 사용을 막는 전역 잠금도 여전히 없어.
- 다음 작업: 시작 시 `agent_instances`에 없는 `agents` 행 정리 여부 결정, 또는 "종료 중 거절"을 실제 WebView에서 재현하는 검증 경로 마련.

## 에이전트 상태의 무음 성공 제거 (2026-10-25)

- 선택한 작업: 로드맵 1단계(기존 기능 안정화) 잔여 항목 중 앞선 회차가 남긴 "`Store::status`의 무음 성공". `status`는 `UPDATE agents ... WHERE id=?`의 갱신 행 수를 보지 않고 `meta.revision`을 올린 뒤 `Ok(())`를 돌려줬어. 세션의 `agent_instance_id`(등록 인스턴스 ID, 예: OpenCode 등록 UUID)로 호출되면 `agents` 표에 해당 행이 없어 아무것도 저장되지 않았는데도 호출부는 저장된 것으로 처리했어. `cancel`·`error`·`state`가 모두 이 경로를 쓰고, `AgentsView.tsx`는 이미 `snapshot.agents`에서 인스턴스 ID로 상태를 찾고 있어 화면에는 값이 없어 `idle`로 대체되고 있었어.
- 변경 파일: `mvp-app/src-tauri/src/store.rs`(`status`가 `Result<bool>` 반환, 빈 ID 거부, `agents`·`agent_instances`에 있는 ID만 허용, 등록 인스턴스는 `INSERT OR IGNORE`로 `idle` 기준 행을 만든 뒤 갱신, 갱신 행 0개면 `false`·revision 미변경, 회귀 테스트 1개), `mvp-app/src-tauri/src/engine.rs`(`state`·`cancel`이 `false`를 받으면 `agent_status_unmatched` audit 기록, 중단·턴 진행은 그대로 유지).
- 동작 보존: `agents` 스키마·기존 두 행, `tool_deliveries`, Blocking Delegation, `request_arch_review`, archReview/latestUserRequirements 분리, 종료 순서, Arch 타임아웃 120초(범위 30~600), 기존 세션·메시지·컨텍스트는 변경하지 않았어. 새 DB 마이그레이션은 없고 `agents`에 등록 인스턴스 행이 추가로 생길 수만 있어(기존 행·데이터 삭제 없음). `cli.rs`의 `store.status("antigravity", ...)?`와 정리 경로의 `let _ =` 호출은 반환형 변경만으로 컴파일되며 동작이 같아.
- 검증 (실제 출력):
  - `npm test` → `test result: ok. 53 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.68s` (기존 52 + 신규 1, `store::tests::status_reports_whether_it_was_stored ... ok`)
  - `node scripts/verify_external_threads.mjs` → `{"passed":true,"recordedRows":27,"scratchRowsHidden":9,"normalConversationsPreserved":true}`
  - `npm run build` → `✓ built in 2.75s` (`dist/assets/index-B_6vxOOa.js` 433.29 kB)
- 회귀 검사 내용: builtin `codex` 상태는 `true`로 저장되고 snapshot에 반영. `agent_instances`에만 있는 `inst-open-1`은 행이 생성되고 상태가 저장됨. 없는 `ghost-agent`와 공백 ID는 `false`를 반환하고 행을 만들지 않으며 `revision`도 그대로. 이 테스트는 저장소 계층만 검사해요.
- 이번 결과는 Rust 단위·분류 회귀 검사야. 실제 Tauri WebView에서 OpenCode 세션의 실행 상태가 Agents 화면에 표시되는지, 실제 CLI 턴에서 `agent_status_unmatched`가 남는지는 확인하지 않았어. `python scripts/verify_*.py`와 데스크톱 빌드는 실행하지 않았어.
- 남은 문제: 원본 세션 동시 사용을 막는 전역 잠금은 여전히 없어. 등록 인스턴스 행이 한 번 생기면 등록 해제 후에도 `agents`에 남아(상태 `unknown`으로 복구될 뿐) 정리 경로가 없어. 이번 변경으로 화면에 인스턴스별 실행 상태가 나타날 수 있지만 WebView 검증은 없어.
- 다음 작업: "종료 중 생성·가져오기·등록 거절"을 실제 WebView에서 재현하는 검증 경로 마련, 또는 등록 해제 시 `agents` 행 정리 여부 결정.

## 도구 응답 확인의 무음 성공 제거 (2026-10-25)

- 선택한 작업: 로드맵 1단계(기존 기능 안정화) 잔여 항목 중 앞선 회차가 남긴 "`Store::tool_state` 무음 성공, 특히 `confirm_tool`의 `turn_id=None` → `null` 키". `confirm_tool`은 `self.turn_id`가 없을 때 `null`을 포함한 키로 갱신을 시도했고, `tool_state`는 갱신 행이 0개여도 `Ok(())`를 돌려줘 실제로 저장되지 않은 상태를 `confirmed`로, `review_confirmed` audit까지 남길 수 있었어.
- 변경 파일: `mvp-app/src-tauri/src/store.rs`(`tool_state`가 갱신 행 수 일치 여부를 `bool`로 반환), `mvp-app/src-tauri/src/cli.rs`(`confirm_tool`이 `Result<bool>` 반환, turn ID 없음·행 불일치 시 `review_confirm_unmatched` audit 후 `false`), `mvp-app/src-tauri/src/engine.rs`(`dynamicToolCall` 완료 시 확인이 실제로 성립한 경우에만 `검토 결과를 반영하는 중` 상태로 전환).
- 동작 보존: `prepared → sent → confirmed/unknown` 상태 값과 `tool_deliveries` 스키마, 재시작 시 `prepared`·`sent` → `unknown` 처리, Blocking Delegation, `request_arch_review`, archReview/latestUserRequirements 분리, 종료 순서, Arch 타임아웃 120초(범위 30~600)는 변경하지 않았어. `resolve_once`의 `sent`·`unknown` 기록은 `reserve_tool` 직후라 행이 보장되므로 호출부를 바꾸지 않았어.
- 검증 (실제 출력):
  - `npm test` → `test result: ok. 52 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.69s` (신규 테스트 파일 없이 기존 `cli::tests::mock_stdio_keeps_one_tool_reply_on_the_original_rpc_id`에 회귀 4개 항목 추가)
  - `node scripts/verify_external_threads.mjs` → `{"passed":true,"recordedRows":27,"scratchRowsHidden":9,"normalConversationsPreserved":true}`
  - `npm run build` → `✓ built in 3.13s` (`dist/assets/index-B_6vxOOa.js` 433.29 kB)
- 회귀 검사 내용: 정상 확인은 `true`와 `confirmed` 유지. `turn_id=None`에서 확인 시 `false`·`review_confirmed` audit 수 불변·`review_confirm_unmatched` 1건·`call_key LIKE '%:null:%'` 행 0건. 없는 키에 대한 `tool_state`는 `false`. 기존 `codex_tx` audit 1회·중복 응답 차단·전송 실패 `unknown` 검사도 그대로 통과했어.
- 이번 결과는 Rust 단위·분류 회귀 검사야. 실제 Codex App Server가 `dynamicToolCall` 완료를 보내는 경로를 실제 CLI로 재현한 검사가 아니고, `python scripts/verify_*.py`와 데스크톱 빌드는 실행하지 않았어. `confirm_tool`이 `false`를 반환하는 상황(턴 ID 소실·키 불일치)은 실제 CLI에서 강제로 만들지 않았어.
- 남은 문제: 원본 세션 동시 사용을 막는 전역 잠금은 여전히 없어. `Store::status`도 갱신 행 0개를 성공으로 처리해(에이전트 ID가 없으면 조용히 넘어감) 같은 무음 성공 유형이 남아 있어.
- 다음 작업: `Store::status`의 무음 성공 처리 여부 결정, 또는 "종료 중 생성·가져오기·등록 거절"을 실제 WebView에서 재현하는 검증 경로 마련.

## 종료 접수 차단을 에이전트 등록·해제와 OpenCode 모델 조회까지 확장 (2026-10-25)

- 선택한 작업: 로드맵 1단계(기존 기능 안정화) 잔여 항목 "종료·큐·원본 동시 사용 실패 재현" 중 앞선 회차가 남긴 접수 차단 공백. `register_agent_instance`/`unregister_agent_instance`는 종료 중에도 `agent_instances`·`agents` 행을 쓰거나 지울 수 있었고, `list_opencode_models`는 종료 중에도 단기 OpenCode CLI 프로세스를 새로 띄울 수 있었어.
- 변경 파일: `mvp-app/src-tauri/src/registry.rs`(명령 진입용 `register_instance`/`unregister_instance`와 회귀 1개), `mvp-app/src-tauri/src/main.rs`(두 등록 명령이 새 함수를 쓰도록 교체, `list_opencode_models` 진입부 검사), `mvp-app/src-tauri/src/engine.rs`(`admission()` 접근자만 추가).
- 차단 검사를 Tauri 래퍼가 아니라 저장소 호출 바로 옆에 둔 이유는 판정 순서(검사 → 쓰기 없음)를 앱 없이 단위 테스트로 확인하기 위해서야. 기존 `submit`·`request_compact`·`set_model_settings`·`set_opencode_model`의 검사와 오류 문구, shutdown 순서, Blocking Delegation, `tool_deliveries`, Arch 타임아웃 120초(범위 30~600), DB 스키마·사용자 데이터는 변경하지 않았어.
- 검증 (실제 출력):
  - `npm test` → `test result: ok. 52 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.62s` (기존 51 + 신규 1)
  - `node scripts/verify_external_threads.mjs` → `{"passed":true,"recordedRows":27,"scratchRowsHidden":9,"normalConversationsPreserved":true}`
  - `npm run build` → `✓ built in 2.79s` (`dist/assets/index-B_6vxOOa.js` 433.29 kB)
- 신규 테스트 `registration_writes_are_refused_while_closing`는 실제 SQLite에 등록 행을 만든 뒤 접수를 닫고, 등록·해제가 `종료 중` 오류로 거절되며 행 수가 변하지 않고 기존 행이 남는지, 이후 호출도 계속 거절되는지를 확인해. 이는 명령 본문이 호출하는 함수 경로를 직접 검사한 것이고, Tauri 명령 배선 자체는 컴파일과 코드 확인으로만 검증했어.
- 이번 결과는 Rust 단위·분류 회귀 검사야. 실제 Tauri WebView에서 종료 중 명령 경합을 재현한 검사가 아니고, `python scripts/verify_*.py`와 데스크톱 빌드는 실행하지 않았어. OpenCode CLI를 실제로 띄워 종료 중 거절을 확인한 것도 아니야.
- 남은 문제: 원본 세션 동시 사용을 막는 전역 잠금은 여전히 없어. `Store::tool_state`는 없는 전달 키를 갱신해도 성공을 돌려주고, `confirm_tool`은 `turn_id`가 없을 때 `null` 키로 조회해 조용히 아무 행도 갱신하지 않은 채 `review_confirmed` audit을 남길 수 있어.
- 다음 작업: "종료 중 생성·가져오기·등록 거절"을 실제 WebView에서 재현하는 검증 경로를 마련하거나, `tool_state`의 무음 성공을 제거할지 결정.

## 종료 접수 차단을 생성·가져오기 명령까지 확장 (2026-10-25)

- 선택한 작업: 로드맵 1단계(기존 기능 안정화) 잔여 항목인 "종료·큐·원본 동시 사용 실패 재현" 중 종료 접수 차단 공백. `submit`만 `accepting`을 검사해서 룸·세션 생성, 외부 세션 목록·가져오기, 모델 목록 command는 종료 중에도 worker job·SQLite 쓰기·단기 Codex 프로세스를 시작할 수 있었어.
- 변경 파일: `mvp-app/src-tauri/src/engine.rs`(공용 `ensure_accepting` 함수와 `Engine::ensure_accepting`), `mvp-app/src-tauri/src/main.rs`(`create_room`, `create_session`, `list_codex_sessions`, `import_codex_session`, `list_codex_models` 진입부), `mvp-app/src-tauri/src/characterization_tests.rs`(회귀 1개).
- 기존 `submit`·`request_compact`·`set_model_settings`·`set_opencode_model`의 검사와 오류 문구는 그대로 두었어. shutdown 순서(접수 차단 → worker 중지 → Arch 정리 → Codex 종료 → SQLite 확정), Blocking Delegation, `tool_deliveries`, Arch 타임아웃 120초(범위 30~600)는 변경하지 않았어. 새로 거절되는 경로는 종료 중에만 동작하므로 정상 실행 동작은 그대로야.
- 검증 (실제 출력):
  - `npm test` → `test result: ok. 51 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.64s` (기존 50 + 신규 1)
  - `node scripts/verify_external_threads.mjs` → `{"passed":true,"recordedRows":27,"scratchRowsHidden":9,"normalConversationsPreserved":true}`
- 이번 결과는 Rust 단위·분류 회귀 검사야. 실제 Tauri WebView에서 종료 중 command 경합을 재현한 검사가 아니고, `python scripts/verify_*.py`와 데스크톱 빌드는 실행하지 않았어. 신규 테스트는 접수 차단 판정 자체만 검사하며 command 배선은 컴파일과 코드 확인으로만 검증했어.
- 남은 문제: `register_agent_instance`/`unregister_agent_instance`는 여전히 접수 차단 검사가 없어 종료 확정 단계의 SQLite 쓰기와 겹칠 수 있어. 원본 세션 동시 사용을 막는 전역 잠금도 여전히 없어.
- 다음 작업: 등록·해제 command에도 같은 차단을 적용할지 결정하고, "종료 중 생성·가져오기 거절"을 실제 WebView에서 재현하는 검증 경로를 마련.

## 기준선 확인과 검증 환경 복구 (2026-10-25)

- 로드맵 1단계(기존 기능 안정화) 기준선을 현재 트리에서 다시 확인했어. 코드는 변경하지 않았어.
- 검증 결과 (실제 출력):
  - `npm test` → `test result: ok. 50 passed; 0 failed; 0 ignored; finished in 0.66s`
  - `node scripts/verify_external_threads.mjs` → `{"passed":true,"recordedRows":27,"scratchRowsHidden":9,"normalConversationsPreserved":true}`
  - `npm run build` → `✓ built in 1.99s` (`dist/index.html` 0.36 kB, `index-B_6vxOOa.js` 433.29 kB)
- 발견한 문제: worktree에서 `verify_external_threads.mjs`가 `mvp-app/evidence/workspace-20261018-041248/audit.json`을 열지 못해 `ENOENT`로 실패했어. `mvp-app/evidence/`가 gitignore 대상이라 worktree에도 새 클론에도 존재하지 않기 때문이야. `mvp-app/evidence`를 원본 저장소에 junction으로 연결해 통과시켰고, 새 worktree를 만들 때도 같은 연결이 필요해.
- 이번 결과는 로컬 단위·분류 회귀 검사야. 실제 Codex·Antigravity CLI, Tauri WebView, OpenCode 실행은 하지 않았고 기존 실제 연결 성공 기록을 이번 결과로 재집계하지 않아.
- 다음 작업: 로드맵 1단계 잔여 항목인 "종료·큐·원본 동시 사용 실패 재현" 중 단위 테스트로 재현 가능한 부분부터 착수.

## 자동화 인계 문서 작성 (2026-10-25)

- `docs/AUTOMATION_HANDOFF.md`를 추가했어. 특정 도구나 프로모션에 종속되지 않도록 "한 회차를 어떻게 수행하는가" 기준으로 정리했고, 붙여넣기용 실행 프롬프트, 검증 명령과 판정 기준, 안전 정책, 도구별 복구 방법, 실측 함정 표를 담았어.
- 실측 근거: MausBot 루틴은 `access`가 `read-and-draft` 하나로 고정돼 있고 실행 시 파일 쓰기가 차단돼. 테스트 실행에서 루틴 출력이 `쓰기 불가`였고 `D:\프로젝트\ESH-codex\.routine-write-test.txt`는 생성되지 않았어. 그래서 코드 개발 자동화는 WorkBuddy(1시간 간격)가 맡고, MausBot 루틴은 읽기 전용 상태 점검(6시간 간격)으로 용도를 바꿨어.
- 이번 작업은 문서 추가뿐이라 코드·테스트를 변경하지 않았어. `npm test`를 다시 실행하지 않았고, 직전 회차의 50 passed 기록을 이번 결과로 재집계하지 않아.
- 다음 작업: 로드맵 1단계 잔여 항목인 "종료·큐·원본 동시 사용 실패 재현" 정리부터 착수.

## 브랜치 동기화와 병합 충돌 해결 (2026-10-25)

- `bot/codex-impl`이 main보다 뒤처져 있어 `git merge main`을 실행했어. `docs/PROGRESS.md`, `mvp-app/src-tauri/src/mock_agent.rs` 두 파일에서 충돌이 났어.
- 두 충돌 모두 main 쪽을 채택했어. PROGRESS는 OpenCode 항목 3개까지 포함한 최신 기록이었고, `mock_agent.rs`는 취소 채널 송신자를 유지하는 main의 수정이 타임아웃 테스트를 실제로 검증해. 브랜치 커밋 `6578414`의 테스트 분리 내용은 main에 동등한 리팩터가 이미 들어가 중복이었어.
- 검증: `mvp-app/`에서 `npm test` → **50개 통과, 0개 실패** (`test result: ok. 50 passed; 0 failed`).
- 이번 실행은 단위 테스트만 돌렸어. 실제 Codex·Antigravity CLI, Tauri WebView, OpenCode 실행은 호출하지 않았고 기존 실제 연결 성공 기록을 이번 결과로 재집계하지 않아.
- 다음 작업: 로드맵 1단계(기존 기능 안정화) 잔여 항목 중 "종료·큐·원본 동시 사용 실패 재현" 정리부터 착수.

## 테스트 전용 Mock Agent 재검증 (2026-10-25)

- 현재 `mvp-app/src-tauri/src/mock_agent.rs`는 `main.rs`의 `#[cfg(test)]`에서만 컴파일돼. 운영 `registered()`에는 Codex, Antigravity, OpenCode 세 어댑터가 있고 Mock은 포함되지 않아. 격리 테스트 DB의 Mock 등록 행도 활성 역할이 없는 것으로 확인했어.
- Reviewer 정상 응답의 질문·버전·ID, 실패, 잘못된 ID, 호출자 제한 시간 초과, 실행 중·사전 취소를 독립적으로 검사해. 기존 타임아웃 테스트는 취소 송신자가 먼저 해제되면 제한 시간을 검증하지 못하므로 송신자를 유지하도록 수정했어. 기존 Lead 세션 열기·턴·취소·재연결 후 재개와 원래 열린 도구 호출에 대한 1회 응답 검사도 유지했어.
- 현재 `main`에서 `npm test` **50개 통과, 0개 실패**. `node scripts/verify_external_threads.mjs`도 통과했어. 이번 실행은 in-process Mock·단위/분류 회귀 검사이며 실제 Codex·Antigravity CLI, API, Tauri WebView는 호출하지 않았어. 운영 `engine.rs`는 여전히 기존 Codex·Antigravity 실행 경로를 직접 사용하므로 범용 Registry 기반 실행 디스패치까지 검증한 것은 아니야.

## OpenCode 세션 모델 선택 (2026-10-25)

- `mvp-app/src-tauri/src/opencode.rs`에서 등록된 CLI의 `models --pure` 출력을 제한 시간과 크기 검사를 두고 읽어 `provider/model` 및 `nvidia/deepseek-ai/deepseek-r1` 같은 멀티 슬래시 ID를 표시해. 빈 세그먼트·공백·제어 문자가 있는 ID는 거부해. `mvp-app/src/main.tsx`는 목록 조회 실패를 표시하고 다시 조회할 수 있으며, 저장 모델의 목록 누락은 해당 세션의 조회가 완료된 뒤에만 보여줘.
- `mvp-app/src-tauri/src/engine.rs`는 선택 시 CLI 모델 목록과 대조한 뒤 기존 `chat_sessions.model`에 세션별로 저장해. 실행할 때는 목록을 다시 조회하지 않고 선택한 모델에만 `run --model`을 전달해. CLI가 모델을 거부하거나 제공자 연결에 실패하면 실행 오류로 처리해. 미선택 세션은 `--model`을 생략해 OpenCode 기본 동작을 따르며, Codex 모델·추론 강도 경로는 유지해. 새 DB 마이그레이션은 없어.
- 이번 수정 검증: `mvp-app/src-tauri/src/opencode.rs`의 실제 출력 사례 기반 파싱·부정 입력 테스트를 포함해 `npm test` 46개 통과, `npm run build`와 수정 후 `npm run build:desktop` 통과. 격리된 ESH WebView에서 `openrouter/cohere/north-mini-code:free`를 선택해 세션별 DB 저장을 확인했고, 선택 모델로 보낸 요청에 실제 `OK` 답변을 받았어.

## OpenCode Lead 연결 구현 (2026-10-25)

- 등록된 npm OpenCode CLI를 Lead로 활성화하고 새 세션에서 선택할 수 있게 연결했어. JSON 답변의 세션 ID를 SQLite에 저장해 다음 턴에서 이어 쓰며, 중단·시간 초과·오류를 기존 단일 worker 경로로 처리해. 기존 Codex 세션은 Codex로 유지해.
- SQLite 스키마를 v6으로 올렸어. 기존 세션은 Codex로 보존하고, 기존 OpenCode npm CLI 등록은 경로가 일치할 때 실행 연결로 마이그레이션해. 데이터 보존을 포함한 Rust 테스트 44개, 프런트 빌드, 같은 이름의 데스크톱 EXE 빌드가 통과했어.
- 실제 ESH WebView에서 격리 DB를 사용해 npm OpenCode CLI를 등록하고, 새 Lead 세션에서 첫 요청과 이어쓰기 요청에 각각 `OK` 답변을 받았어. SQLite에 동일한 외부 세션 ID와 네 메시지의 `delivered` 상태가 저장됐고, 앱을 닫았다 다시 열어 두 답변이 화면에 복원되는 것도 확인했어. 테스트용 DB는 사용자 기본 DB와 분리했어. 테스트 창을 여는 도구 환경에서는 `USERPROFILE`이 없어 Codex 모델 목록 조회 오류가 따로 표시됐지만 OpenCode 두 턴은 성공했어.

## 등록 역할과 활성 역할 구분 (2026-10-24)

- 기존 SQLite `agent_instances.role`은 희망 역할로 그대로 읽고 저장해. `list_agent_instances`의 응답에 계산된 `activeRole`을 추가했어. builtin 인스턴스의 ID·연결 방식·역할·capability가 실제 등록 Adapter와 모두 맞을 때만 `lead` 또는 `reviewer`를 반환하고, 실행 어댑터가 없는 사용자 등록은 `null`이야. DB 스키마·사용자 기록·worker·Blocking Delegation은 변경하지 않았어.
- Agents 추가 폼과 등록 상세에는 `희망 역할`, 상세에는 별도 `활성 역할`을 표시해. 새 CLI 후보가 Lead를 희망해도 활성 역할은 `없음`으로 보여. Codex/Antigravity의 활성 역할은 각각 Lead/Reviewer로 표시해. 기존 등록 행과 경로는 그대로 유지해.
- `npm test` 41개 통과(기존 40개 + 역할 판정 회귀 1개). 첫 `npm run build:desktop`은 프런트 빌드 통과 후 실행 중인 ESH 앱(PID 15960)의 EXE 잠금으로 실패했어. 사용자가 앱을 닫은 뒤 같은 이름의 최종 EXE 빌드가 통과했어. 실제 WebView 등록 검사 `mvp-app/evidence/registry-20261024-223117/result.json`에서 builtin Codex 활성 Lead, 미지원 신규 인스턴스의 희망 Lead/활성 없음, 등록·해제, 대화 snapshot·보호 설정 보존을 확인했어. 이 UI 검사는 실제 AI 턴을 실행하지 않았어.

## 테스트 전용 Mock Agent 확장 검증 (2026-10-24)

- `mvp-app/src-tauri/src/mock_agent.rs`를 `#[cfg(test)]`로만 연결했어. 고유 ID `mock_agent`, 테스트 전용 in-process 연결 방식, Lead/Reviewer 역할과 기능 계약을 갖고 실제 CLI·API를 호출하지 않아. 격리 테스트 DB에만 `mock-reviewer`/`mock-lead` 인스턴스를 넣어 Registry 연결을 확인해. 운영 `registered()`와 기본 Agents 목록은 Codex/Antigravity 두 개 그대로야. 단일 worker 및 Blocking Delegation 실행 경로는 변경하지 않았어.
- Mock Reviewer 성공·실패·시간 초과·대기 중 취소·잘못된 review ID를 검사했어. 잘못된 ID는 기존 Arch 구조화 결과 검증 함수를 재사용해 거부해. Mock Lead는 세션 열기, 턴 시작, 같은 열린 호출에 1회 응답, 잘못된/중복 호출 거부, 취소, 연결을 다시 만든 뒤 세션 재개를 검사했어. Mock Lead→Reviewer→원래 열린 호출의 위임 흐름에서는 다른 턴의 병렬 시작을 거부하고 원 호출 완료를 확인했어. 이는 어댑터 계약 테스트이며 실제 Codex↔Antigravity CLI 통합 성공으로 집계하지 않아.
- 최종 `npm test`: 기존 36개와 신규 4개, 합계 40개 통과. 기본 sandbox의 첫 테스트 빌드는 Tauri `OUT_DIR` 환경 오류였고 권한 있는 재실행에서 통과했어. 프런트 빌드는 통과했지만 실행 중인 ESH 앱(PID 10840)이 처음 EXE 교체를 막았어. 사용자가 앱을 닫은 뒤 같은 이름의 `npm run build:desktop`이 통과했고, 마지막 테스트 전용 위임 검사 보강 후 재실행한 EXE 빌드도 통과했어.
- 실제 등록 WebView 검사 `mvp-app/evidence/registry-20261024-203911/result.json`이 통과했어. 실제 CLI/WebView 통합 검사 `mvp-app/evidence/panels-20261024-203930/result.json`도 통과했어. Codex의 같은 턴에서 Antigravity 리뷰 2회가 성공하고 열린 호출별 응답 1회·도구 완료 1회, `tool_deliveries=confirmed`, 턴 재개와 재시작 기록 복원을 확인했어. 두 실제 검사는 마지막 테스트 전용 Mock 위임 검사 추가 직전의 빌드에서 실행했고, 추가 후 최종 빌드가 통과했어. 이 실제 CLI 결과는 Mock 40개 테스트와 별도로 집계해. 새로운 Mock Agent 자체는 실제 API 호출을 하지 않았어.
- 등록 UI는 실행 어댑터가 없는 후보도 `Lead`/`Reviewer`를 선택해 저장해. `connection_kind=not_configured`, `support=unsupported`라 실행은 차단되지만 선택한 역할이 활성화된 듯 보일 수 있어. 최소 변경안은 등록 시 역할을 **희망 역할**로 명시하고, 활성 역할은 어댑터가 연결돼 capability·연결 방식·검증 상태를 충족할 때만 별도 표시·선택 가능하게 하는 거야. 기존 SQLite `role` 값은 희망 역할로 해석해 보존하고, 먼저 UI 문구/표시와 백엔드 활성화 판정을 분리하면 마이그레이션 없이 시작할 수 있어. 이번 작업에서는 등록 UI·DB를 바꾸지 않았어.

## 기본 경로·OpenCode 재검사 (2026-10-24)

- 현재 EXE의 실제 Tauri WebView 등록 검사는 `mvp-app/evidence/registry-20261024-202531/result.json`에서 통과했어. 설치된 Claude/OpenCode의 CLI 경로와 미설치 Gemini의 제안 경로, 등록·해제, 대화 snapshot 및 보호 설정 보존을 확인했어. 첫 격리 실행은 OS 접근 거부로 probe 제한 시간을 넘겼고, 권한 있는 재실행은 통과했어. 이 UI 검사는 실제 AI 턴을 실행하지 않아.
- `npm test`는 36개 모두 통과했어. 사용자가 현재 등록한 OpenCode 인스턴스는 읽기 전용 DB 확인 시 npm CLI 실행 파일 경로와 Lead 역할을 갖고 있었어. 다만 연결 방식은 여전히 `not_configured`라 ESH의 OpenCode 실행 어댑터는 없어.
- 해당 OpenCode CLI로 별도 최소 모델 턴을 재시도했어. 모델 선택까지 진행한 뒤 `Cannot connect to API: Unable to connect`와 종료 코드 1로 끝나 응답을 받지 못했어. 보호 대상 인증·설정 파일 20개 항목의 해시는 전후 같아. 근거: `mvp-app/evidence/opencode-retest-20261024/result.json`. 기존 사용자 세션은 이어서 실행하지 않았어.
- 이번 재검사에서 코드는 수정하지 않았고 EXE도 재빌드하지 않았어. OpenCode API 접속 원인과 ESH 내 OpenCode AI 턴은 아직 확인되지 않았어.

확인일: 2026-10-18. 현재 실행 파일 대상은 `esh-chat-mvp4`, 패키지 버전은 `0.1.0`이야. 아래 초기 검증 표는 인계 문서 작성 당시 결과이며, 후속 수정은 별도로 기록해.

## 에이전트 추가 기본 경로 제안 (2026-10-24)

- 후보 선택 시 설치 파일을 찾으면 그 절대 경로를 입력칸에 넣고, 찾지 못해도 Claude `~/.local/bin/claude.exe`, Gemini `%APPDATA%/npm/gemini.cmd`, OpenCode `%APPDATA%/npm/node_modules/opencode-ai/bin/opencode.exe`를 제안해. 제안 경로의 파일이 없으면 설치 상태는 계속 `미설치`야. OpenCode 데스크톱 `OpenCode.exe`는 CLI 기본 경로로 사용하지 않아.
- Gemini의 알려진 npm `gemini.cmd`는 경로 등록만 허용하고 실행 연결은 추가하지 않았어. 후보 탐지에서는 어떤 CLI나 셸 스크립트도 실행하지 않아. 기존 사용자 등록 행·설정·SQLite 스키마·worker·Blocking Delegation은 변경하지 않았어.
- `npm test` 36개 통과(기존 35개 + 기본 경로·Gemini npm shim 검사 1개). 동일 이름의 `npm run build:desktop` 통과. 첫 EXE 교체는 실행 중인 앱의 잠금으로 실패했고 사용자가 닫은 뒤 성공했어. 최종 EXE의 실제 WebView `mvp-app/evidence/registry-20261024-202133/result.json`에서 Claude·OpenCode 설치 경로, 미설치 Gemini 기본 경로의 입력칸 반영, 기존 등록·중복 거부·해제와 대화 snapshot/설정 보존을 확인했어. 실제 AI 턴은 이번 UI 검사에서 실행하지 않았어.

## OpenCode 연결 확인 (2026-10-24)

- 사용자 DB를 읽기 전용으로 확인했어. `opencode`/Reviewer 인스턴스가 등록돼 있고 경로는 OpenCode 데스크톱 `OpenCode.exe` 1.18.21.0이야. `connection_kind=not_configured`이며 현재 ESH에 OpenCode 실행 어댑터는 없어. 따라서 등록 상태는 ESH의 실제 AI 연결 성공을 뜻하지 않아.
- 별도 npm OpenCode CLI 1.18.34의 `--version`/`--help`는 성공했어. 실제 `run --pure` 최소 응답 요청은 기본 모델 선택 후 `Cannot connect to API: Unable to connect`로 종료 코드 1이었어. 첫 기본 샌드박스 시도는 CLI 로그 경로 접근 차단으로 모델 호출 전에 실패했고, 일반 실행 권한에서 다시 시도한 결과가 API 연결 실패야. 인증·설정 파일 해시는 전후 동일했어. 근거: `mvp-app/evidence/opencode-smoke-20261024/result.json`.
- 미확인: OpenCode 데스크톱 앱 자체의 모델 응답과 ESH 내 OpenCode 턴 실행. 현재 구현 범위에는 두 실행 경로가 없어 이번 검사에서 성공으로 집계하지 않아.

## Agents 안전 탐지·등록 (2026-10-24)

- `registry.rs`에 Claude Code(`claude.exe`), Gemini CLI(`gemini.exe`), OpenCode(`opencode.exe`)의 PATH 파일 탐지와 절대 경로 수동 등록을 추가했어. 파일명·파일 존재를 검사할 뿐 탐지한 CLI를 실행하거나 인증 파일을 읽지 않아. 후보는 이 세 종류로 제한했고 `.cmd` 래퍼나 PATH 밖의 설치는 자동 탐지하지 않아. 수동 경로로 등록할 수 있어. 파일명 검사는 제조사 서명·진위를 증명하지 않으므로 등록된 신규 인스턴스는 `미지원`/`설정 필요`로 표시하고 실행 경로에 연결하지 않아.
- SQLite v4→v5에 `agent_instances` 테이블을 추가해 provider·고유 instance ID·connection·role·경로를 분리했어. Codex·Antigravity builtin 행을 만들되 기존 `agents` 실행 상태, 세션·메시지·`tool_deliveries`를 옮기거나 삭제하지 않아. 중복 provider/정규화 경로는 거부하고 등록 해제는 사용자 행만 지워. 실행 파일과 기존 설정은 건드리지 않아. `AgentsView.tsx`에 추가·수동 경로·등록 해제와 설치/인증/실행 가능/실행 상태 분리 표시를 추가했어. Engine의 단일 worker·Blocking Delegation 코드는 변경하지 않았어.
- `npm test` **35개 통과**(기존 32개 + 탐지/등록/마이그레이션 3개). v4 기록을 가진 DB의 v5 추가, 잘못된 경로·미설치·중복·허용되지 않은 provider·등록 해제·재시작 복원과 기존 세션 전환/취소/압축/위임 회귀를 검사했어. 같은 이름의 `npm run build:desktop` 통과. 첫 빌드는 실행 중인 EXE 잠금으로 실패했고 사용자가 앱을 닫은 뒤 성공했어.
- 최종 EXE 실제 WebView 등록 검사: `mvp-app/evidence/registry-20261024-200610/result.json`. 설치/미설치 후보 목록, 수동 경로 등록, 신규 인스턴스 미지원·ID 표시, 중복 거부, 등록 해제, fixture 파일·대화 snapshot 보존, schema v5를 확인했어. 앞선 `registry-20261024-195717` 실패는 검증 드라이버가 Agents 화면에 남아 공통 오른쪽 패널 크기 검사를 수행한 탓이었고 UI 동작 실패는 아니야. 드라이버 동선을 고친 후 통과했어.
- 실제 WebView/CLI 회귀: 최종 실행 상태 문구 정리 직전의 동일 Rust 코드 EXE에서 `mvp-app/evidence/panels-20261024-200403/result.json`은 같은 턴의 Arch 리뷰 2회와 호출별 도구 응답 1회·`confirmed`를 통과했어. `mvp-app/evidence/panels-20261024-195755/result.json`은 기존 Agents 점검 화면, `mvp-app/evidence/workspace-20261024-195951/result.json`은 A→B→A·세션 재개·기록 복원, `mvp-app/evidence/cancel-20261024-200112/result.json`은 턴/위임 취소, `mvp-app/evidence/compact-20261024-200131/result.json`은 수동 압축 완료를 통과했어. 마지막 변경 후 최종 EXE의 등록 WebView 검사는 다시 통과했어. 이 결과는 실제 CLI 응답이며 신규 CLI 실행 검사는 아니야.
- 확인 한계: 후보 설치 판정은 실행 파일 존재만 뜻하고 인증·실제 실행 가능 여부는 확인하지 않아 `unknown` 또는 `미지원`으로 표시해. 비정상 종료를 포함한 도구 응답 exactly-once는 보장·검증하지 않았어.

## Agents 중앙 관리 화면 배치 (2026-10-23)

- 사이드바의 `대화`·`에이전트` 메뉴와 최근 세션 탐색은 유지하고, 기존 사이드바 내부 상태 카드를 중앙 `AgentsView.tsx`의 에이전트 목록·선택 상세 화면으로 옮겼어. 상세에는 기존 설치·인증·실행 가능·실행 상태, 연결 방식, 점검 시각·실패 사유와 활성 세션·실행 ID를 그대로 사용해. 새 에이전트 연결·프로필·상단 탭은 추가하지 않았어.
- `main.tsx`의 중앙 컨테이너를 고정하고 대화 DOM을 전환 중에도 유지해 세션·스크롤·컴포저 상태를 보존해. Agents를 표시할 때 오른쪽 협업 패널과 리사이저를 화면에서만 숨기고 그 너비를 중앙에 줘. `v0.css`/`themes.css`는 기존 테마 색과 컴포넌트 톤으로 목록·상세를 배치했어. Rust·SQLite·단일 worker·Blocking Delegation 코드는 변경하지 않았어.
- `npm run build`와 동일 이름의 `npm run build:desktop` 통과. 첫 두 EXE 빌드는 사용 중인 실행 파일 잠금으로 실패했고 사용자가 앱을 닫은 뒤 성공했어. 최종 EXE 실제 WebView `mvp-app/evidence/panels-20261023-233134/result.json`에서 중앙 목록·상세, 오른쪽 패널 숨김, 남은 너비 사용, 대화 복귀 후 스크롤 위치, 설치/미설치·수동 새로고침·snapshot 보존을 확인했어.
- 같은 EXE의 실제 CLI/WebView `mvp-app/evidence/cancel-20261023-233153/result.json`은 실행 중 Agents 전환·새로고침 후 같은 세션·실행 ID와 연결/thread 복구를 확인했어. `mvp-app/evidence/panels-20261023-233220/result.json`은 Codex→Antigravity→Codex의 같은 턴 리뷰 2회, 호출당 응답 1회와 `tool_deliveries confirmed`를 통과했어. 두 검사는 Mock AI 응답이 아니야.

## Agents 상태 점검·관리 화면 3단계 (2026-10-23)

- `agents.rs`의 두 어댑터가 기존 실행 파일 탐색 경로를 읽기 전용으로 확인해 설치됨·미설치·점검 실패를 구분해. CLI 실행, 모델 턴, 인증 파일 읽기·수정, 기존 RPC 연결 조작은 하지 않아. 인증은 안전한 비대화식 확인 수단이 없어 `unknown`, 설치돼도 실행 가능은 `unknown`이야. 미설치면 실행 불가, 파일 확인 실패면 점검 실패로 표시해.
- `probe_agents`는 ID·연결 방식, 설치·인증·실행 가능 상태, 점검 시각·실패 사유를 별도 응답으로 반환해. Agents 화면은 처음 열 때와 수동 새로고침 때 조회하며 `점검 전`·`점검 중`도 표시해. 기존 snapshot의 실행 상태는 별도 필드로 유지하고, 활성 상태에는 `activeRun`의 세션·실행 ID를 연결해. probe 결과는 화면 메모리에만 있고 SQLite v4와 기존 `agents` 행, 세션·메시지·`tool_deliveries`는 변경하지 않았어.
- `npm test`: 기존 **29개 + 신규 3개 = 32개 통과**. 설치됨/미설치/파일 검사 오류, 인증·실행 가능 상태의 독립성, 점검 전후 SQLite 기록 보존을 검사했어. `npm run build:desktop`은 프런트와 동일 이름의 `esh-chat-mvp4.exe` 빌드에 통과했어. 처음 두 빌드는 실행 중 EXE 파일 잠금으로 실패했고 사용자가 앱을 닫은 뒤 다시 빌드했어.
- 최종 EXE 실제 WebView `mvp-app/evidence/panels-20261023-231826/result.json`: Codex·Antigravity 설치됨/인증 불명/실행 가능 불명, Antigravity 미설치 override의 실행 불가·실패 사유, `점검 중` 표시와 수동 새로고침, 점검 전후 snapshot 및 SQLite schema v4 보존을 확인했어. 실제 모델 응답은 이 검사에서 사용하지 않았어.
- 최종 화면 검사 직전의 동일 실행 코드 EXE에서 실제 CLI/WebView `mvp-app/evidence/cancel-20261023-231441/result.json`은 실행 중 새로고침 후 세션·실행 ID 유지와 턴/위임 중 취소·복구를 통과했어. `mvp-app/evidence/panels-20261023-231505/result.json`은 같은 턴의 Arch 2회와 호출당 wire 응답 1회·`confirmed`, `mvp-app/evidence/workspace-20261023-231639/result.json`은 A→B→A 및 원본 thread 재개를 통과했어. 이후 최종 빌드에서 바뀐 코드는 debug WebView의 `점검 중` 검사뿐이야.
- 미확인: 설치된 CLI의 인증 여부와 실행 가능 여부는 모델 턴·인증 정보 접근 없이 판정할 수 없어 `unknown`이야. 실제 권한 거부 파일의 운영체제 실패는 단위 검사에서 잘못된 경로 오류로 대체했고, 실제 CLI의 Arch 시간 초과·비정상 종료 exactly-once는 이번 범위에서 새로 검증하지 않았어.

## AgentAdapter 2단계 도입 (2026-10-23)

- `mvp-app/src-tauri/src/agents.rs`에 공통 `AgentAdapter`와 역할별 `LeadAdapter`/`LeadSession`/`ReviewAdapter`, `AgentCapabilities`를 추가했어. 기존 `codex`/`antigravity` ID와 연결 방식(App Server stdio JSON-RPC / agy CLI stream-json), Lead/Reviewer 역할을 각각 별도 값으로 둬. Codex의 `leadConversation`과 `toolCallBridge`도 별도 capability야. 설치·인증 점검은 아직 하지 않아 `probe=NotChecked`이며, 실제 연결 성공으로 표시하지 않아.
- Codex 어댑터는 기존 `Rpc`를 감싸고 Antigravity 어댑터는 기존 `cli::arch`를 그대로 호출해. `Engine`의 단일 worker와 Blocking Delegation 이벤트 루프, 원래 `request_arch_review` RPC ID로 돌려주는 응답, `tool_deliveries`, 세션 전환·취소·수동 압축·SQLite 경로를 유지했어. 별도 조회용 Codex 연결도 같은 어댑터의 `connect`를 사용해. UI·스키마·기존 설정은 변경하지 않았어.
- `npm test`: 기존 26개와 어댑터 메타데이터, Mock Lead 열린 호출 ID/중복 응답, Mock Reviewer 버전·취소 계약 3개를 합쳐 **29개 통과**. `npm run build:desktop` 통과, 같은 `esh-chat-mvp4.exe`를 갱신했어.
- 현재 EXE의 실제 CLI/WebView 검사: `mvp-app/evidence/panels-20261023-212943/result.json`에서 같은 Codex 턴의 Arch 검토 2회, 호출당 응답·완료 event 1회, `confirmed`와 복원을 확인했어. `mvp-app/evidence/workspace-20261023-213113/result.json`에서 A→B→A·원본 재개·복사 및 기록 보존을 확인했어. `mvp-app/evidence/cancel-20261023-213221/result.json`에서 턴·위임 중 취소, `mvp-app/evidence/compact-20261023-213244/result.json`에서 수동 압축 완료와 메시지 보존을 확인했어. 모두 실제 CLI 응답이고 Mock AI 답변이 아니야.
- 남은 범위: 설치·인증·실행 가능 상태 점검과 Agents 관리 화면은 3단계야. 이번 검사로 비정상 종료까지 포함하는 exactly-once를 주장하지 않아.

## 세션 전환 시 입력 누락 수정 (2026-10-23)

- `engine.rs` worker가 A의 RPC를 닫은 뒤 `continue`해 이미 꺼낸 B 입력을 잃던 경로를 고쳤어. 현재 입력의 activeRun을 연결 전환 전에 만들고, 이전 연결을 닫은 뒤 같은 입력으로 턴을 진행해. 전환 중 취소되면 새 턴을 시작하지 않아. 종료 확인 실패는 `unknown`으로 기록하고 해당 worker의 후속 입력을 `interrupted`로 남겨 새 연결을 열지 않아. 재연결 실패로 소모된 입력도 `interrupted`가 되며 자동 재실행하지 않아.
- `store.rs`는 `turn/start` 응답에 turn ID가 있을 때 해당 사용자 메시지의 thread/turn과 `delivered`를 원자적으로 기록해. 이미 전달·취소·중단된 메시지에는 두 번째 턴 결합을 거부해. 실패한 입력의 `queued → interrupted` 역시 조건부 업데이트로 처리해, 동시에 들어온 취소의 `cancelled` 상태를 덮어쓰지 않아. 기존 스키마·세션·메시지·`tool_deliveries`를 마이그레이션하거나 초기화하지 않았어.
- Rust `npm test` **26개 통과**. A→B→A 연결 전환, B의 다중 대기 메시지 순서, 같은 메시지 중복 턴 차단, 전환 중 취소, 연결 종료·Mock 재연결 실패의 복구 상태를 추가로 검사했어. `npm run build:desktop` 통과, 실행 대상은 같은 `esh-chat-mvp4.exe`야. 첫 빌드는 실행 중인 앱의 파일 잠금으로 실패했고 사용자가 닫은 뒤 다시 빌드했어.
- 이전 실패는 `mvp-app/evidence/workspace-20261023-205406/result.json`으로 보존했어. 수정 후 실제 CLI/WebView `mvp-app/evidence/workspace-20261023-211114/result.json`과 최종 EXE의 `mvp-app/evidence/workspace-20261023-211601/result.json`은 A→B→A, 같은 원본 thread 재개, 격리된 세션 맥락, 복사 세션 Arch 도구 응답 1회, 재시작 후 기록 보존을 통과했어. `mvp-app/evidence/panels-20261023-211221/result.json`은 같은 Codex 턴의 Arch 2회 검토와 호출당 응답 1회·`confirmed`를, `mvp-app/evidence/cancel-20261023-211417/result.json`은 기존 위임 중 취소를 통과했어. 후자의 두 검사는 최종 실패 경로 보강 직전 EXE에서 실행했고, 최종 EXE는 세션 전환 성공 경로를 재검사했어.
- 실제 연결 종료 실패·재연결 실패와 전환 중 취소는 Mock/단위 회귀로 검사했어. 해당 경합을 실제 CLI 프로세스 장애로 강제 재현한 결과는 아니야. 실행 결과 요약은 `mvp-app/evidence/session-switch-20261023/result.json`에 있어.

## 범용 에이전트 계획 1단계: 기존 동작 특성 테스트 (2026-10-23)

- [AGENT_MANAGEMENT_PLAN.md](AGENT_MANAGEMENT_PLAN.md)에 요청한 7개 계약별 단위·Mock/실제 CLI 검사와 검증 공백을 나눠 적었어. 새 에이전트 연결, DB 마이그레이션, UI 동작 변경은 하지 않았어.
- `mvp-app/src-tauri/src/characterization_tests.rs`의 격리 SQLite 검사 3개를 추가했어. 로컬 세션별 턴·버전·thread 결합과 재시작 시 queued 입력 처리, 협업 요청·답변과 `tool_deliveries` 보존/unknown 복구, 위임 중 같은 세션의 새 요구사항과 다른 세션 입력의 분리를 확인해.
- `mvp-app/src-tauri/src/cli.rs`에 가짜 stdio 상대를 쓰는 검사 1개를 추가했어. 원래 JSON-RPC ID에 응답하고 같은 호출의 중복 응답을 막으며, `sent → confirmed`와 전송 실패 시 `unknown`을 확인해. 이는 Mock 프로토콜 검사이며 실제 Codex/Antigravity 접속 성공이나 비정상 종료까지 포함하는 exactly-once 보장은 아니야.
- `mvp-app/`에서 `npm test`는 Rust 21개 통과(기존 17개 + 신규 4개). 처음 기본 샌드박스의 Tauri 리소스 빌드는 Windows 액세스 거부였고, 권한 경계 밖 재실행에서 통과했어. 첫 `npm run build:desktop`은 프런트 빌드 뒤 실행 중인 PID 49604의 `esh-chat-mvp4.exe` 잠금으로 실패했어. 사용자가 앱을 닫은 뒤 같은 이름으로 재실행한 데스크톱 빌드는 통과했어. 별도 exe를 만들지 않았어. 결과 요약: `mvp-app/evidence/agent-baseline-20261023/result.json`.
- 현재 빌드의 실제 CLI/WebView 검사: `mvp-app/evidence/panels-20261023-205145/result.json`은 Codex → Arch 2회 검토 → 같은 thread/turn 재개, 호출당 wire 응답 1회와 완료 event, `confirmed`, 재시작 기록 복원, CLI 부재 오류를 통과했어. `mvp-app/evidence/cancel-20261023-205346/result.json`은 위임 중 취소와 Arch 프로세스 종료, 열린 호출 취소 응답 1회, 같은 연결 복구를 통과했어. `mvp-app/evidence/compact-20261023-210026/result.json`은 실제 수동 압축 요청 1회, 일치하는 item ID, 완료와 메시지 보존을 통과했어. `mvp-app/evidence/panels-final-20261023-210046/result.json`은 기존 실제 CLI 대화의 격리 복원과 구버전 기록 보존을 통과했어.
- **실패:** `mvp-app/evidence/workspace-20261023-205406/result.json`은 새 세션 B의 입력을 보낸 뒤 GUI probe 제한 시간을 넘겼어. 격리 DB에는 B 입력이 `queued`로 남고 B의 `turn/start`는 없어. `engine.rs` worker가 A의 RPC를 닫은 후 `continue`하여 이미 `rx.recv()`로 꺼낸 B 입력을 다시 처리하지 않는 경로와 일치해. 따라서 실제 세션 전환·재개 회귀는 통과로 집계하지 않아. 기존 동작을 바꾸려면 이전 RPC를 정상 종료한 다음 **같은 Input**으로 다음 실행을 이어가도록 하고, 세션 A/B/A 순서·큐·중단·열린 도구 정리까지 회귀 검증해야 해. 이번 테스트 단계에서는 실행 코드를 고치지 않았어.
- 실제 Arch 제한 시간 강제 경과와 실제 CLI의 수동 압축 실패·시간 초과는 검증 공백이야. 이전 증거를 이번 성공으로 집계하지 않아.
- 발견한 변경 후보: `Store::tool_state`는 없는 전달 키를 갱신해도 성공을 돌려줘. 향후 provider/adapter가 추가되면 잘못된 호출 확인을 놓칠 수 있어. 또 `codex_tx` audit은 실제 stdio 쓰기 전에 남아서, 그 개수만으로 wire 전달 횟수를 증명할 수 없어. 이번 단계에서는 동작을 바꾸지 않았고, 수정 시 기존 `unknown` 복구 및 `confirmed` 경로와의 호환성을 먼저 검사해야 해.

## 중앙 대화 머리말·시간·구분선 정리 (2026-10-23)

- 중앙 Codex 답변 위의 `Codex · Lead` 머리말을 제거하고 사용자·Codex 메시지의 시간을 각각 본문 아래로 옮겼어. 사용자 메시지 바로 다음에 Codex 답변이 나올 때만 `<hr>` 구분선을 표시해. 사용자 말풍선과 오른쪽 협업 기록의 머리말·카드는 유지해.
- `npm run build`와 같은 `esh-chat-mvp4.exe`의 `npm run build:desktop` 통과. 실제 Tauri WebView 합성 마크다운 검사 `mvp-app/evidence/markdown-20261023-203238/result.json`에서 중앙 머리말 제거·본문 아래 시간·사용자 다음 답변의 구분선·말풍선 구별·협업 카드 유지와 원문 보존을 확인했어. 격리 복원 검사 `mvp-app/evidence/panels-final-20261023-203247/result.json`에서 기존 실제 CLI 대화 6개와 구버전 기록 4개, 패널 읽기 전용·크기 조절을 확인했어. 새 AI 요청은 보내지 않았어.

## Codex 답변 문서형 표시 (2026-10-23)

- 중앙 Codex 답변 본문의 배경·테두리·모서리·안쪽 여백을 없애 문서처럼 표시해. 마크다운 글자 크기와 코드 블록 구분은 유지하고, 사용자 메시지 말풍선과 오른쪽 협업 기록 카드에는 적용하지 않았어. Rust·SQLite·에이전트 연결은 변경하지 않았어.
- `npm run build`와 동일한 `esh-chat-mvp4.exe`의 `npm run build:desktop` 통과. 실제 Tauri WebView 합성 마크다운 검사 `mvp-app/evidence/markdown-20261023-202825/result.json`에서 중앙 본문의 투명 배경·테두리 0·안쪽 여백 0, 협업 카드 유지, 마크다운 내용·원문 보존을 확인했어. 저장된 실제 CLI 대화의 격리 복원 검사 `mvp-app/evidence/panels-final-20261023-202834/result.json`에서 메시지 6개·구버전 기록 4개 보존과 협업 패널 읽기 전용·크기 조절을 확인했어. 새 AI 요청은 보내지 않았어.

## 대화·협업 패널 가독성 정리 (2026-10-23)

- 협업 패널은 검토 기록이 없는 세션에서 처음에는 접혀 있고 기록이 생기면 열려. 사용자가 직접 접고 펼친 뒤에는 자동 전환하지 않아. 목록 기본 폭은 360px, 상세는 560px로 따로 저장해. 기존에 저장된 600px 이하의 넓은 목록 폭은 한 번 360px로 옮기고 이후 조절 값은 유지해. 중앙 헤더의 검토 버튼으로 언제든 패널을 열 수 있어.
- 중앙 대화·입력 글자는 14px, 협업 상세는 13px로 조정하고 중앙 대화/입력 폭을 760px로 넓혔어. 협업 상세 표의 강제 620px 최소 폭을 없애고 셀 내용을 줄바꿈해. 큰 검토 카드와 중복 영어 섹션 제목을 줄였고, 작동하지 않는 파일 첨부 버튼을 화면에서 제거했어. 모델·추론 선택, 컨텍스트 팝업·압축, 전송, 협업 스레드 목록·상세 기능은 유지해.
- `npm run build:desktop` 통과. 최종 실제 Tauri WebView 복원 검사 `mvp-app/evidence/panels-final-20261023-202442/result.json`에서 저장된 실제 CLI 대화 6개·구버전 기록 4개 보존, 목록·상세 전환, 읽기 전용, 패널 크기 조절을 확인했어. `mvp-app/evidence/panels-20261023-202424/result.json`에서 빈 세션의 협업 패널 초기 접힘과 세 테마를 확인했어. 합성 마크다운 검사 `mvp-app/evidence/markdown-20261023-202432/result.json`에서 중앙 14px·협업 13px, 협업 표 줄바꿈·코드 스크롤·원문 보존을 확인했어. 이번 변경에서는 새 AI 요청을 보내지 않았어.

## 기본 UI 팔레트와 테마 선택 (2026-10-23)

- 왼쪽 하단 워크스페이스 메뉴에 기본·라일락·클래식 테마를 추가했어. 기본 테마는 사용자가 지정한 `#202020` 배경·`#292929` 서피스·`#3B3B3B` 경계선·`#E5E5E5` 텍스트·`#A1A1A1` 보조 텍스트·`#8EADEB` 강조색이야. 라일락과 클래식의 지정 색도 `src/themes.css`에 그대로 반영했어. 사용자가 지정하지 않은 보조 텍스트·경계선·호버색은 각 팔레트에서 `color-mix`로 파생해.
- 테마 선택은 `esh.ui-theme` localStorage에 저장하고 다음 실행에서 렌더링 전에 적용해. 색상은 탐색기·중앙 대화·협업 패널·입력창·팝업·마크다운에 적용해. Rust, SQLite 스키마, CLI/협업 경로는 변경하지 않았어.
- `npm run build`와 같은 `esh-chat-mvp4.exe`의 `npm run build:desktop` 통과. 실제 Tauri WebView 검사 `mvp-app/evidence/panels-20261023-194332/result.json`에서 세 테마의 지정 CSS 팔레트 값, 중앙 배경·입력창 서피스·전송 강조색의 계산 결과, 선택 표시와 localStorage 저장을 확인했어. 검사는 격리된 DB를 사용하고 AI 요청을 보내지 않았어. 테마 초기 검사의 렌더링 대기·전환 측정 오류는 `panels-20261023-194158`과 `panels-20261023-194223`에 실패 기록으로 보존했어.

## 임시 앱 아이콘 제거·프로그램 존댓말 (2026-10-23)

- `mvp-app/src-tauri/icons/icon.ico`의 임시 ESH 그림을 투명한 ICO로 교체했어. Tauri의 Windows 리소스 빌드에 ICO 파일이 필요해서 빌드용 파일은 유지했고, `src-tauri/build.rs`가 아이콘 변경 시 리소스를 다시 생성하게 했어. 최종 `esh-chat-mvp4.exe`에서 추출한 32×32 아이콘의 불투명 픽셀은 0개였어. 기능을 구분하는 화면 내 버튼 아이콘은 유지했어.
- UI 안내·툴팁·오류·상태 문구를 존댓말로 바꾸고, 새 Codex 세션과 기존 세션 재개의 developerInstructions 및 Arch 프롬프트도 사용자에게 존댓말로 답하도록 변경했어. 저장된 과거 대화 원문은 수정하지 않았어. 외부 Codex 기록 제목 정리는 과거 반말 지침과 현재 지침을 모두 인식해.
- `npm test` Rust 17개, `node scripts/verify_external_threads.mjs`, `npm run build:desktop` 통과. 첫 제목 정리 검사는 과거 지침 문구를 인식하지 못해 실패했고, 두 형식을 처리하도록 수정한 뒤 통과했어. 최종 실행 파일의 실제 Tauri WebView 복원 검사 `mvp-app/evidence/panels-final-20261023-002944/result.json`에서 기존 메시지 6개, 구버전 기록 4개 보존과 패널 동작을 확인했어. 새 AI 응답의 실제 존댓말 여부는 이번 검사에서 모델 호출을 하지 않아 확인하지 않았어.

## 수동 압축 요청·완료 일치성 점검 (2026-10-22)

- 압축 요청을 worker 큐에 넣기 전에 SQLite 상태를 `compacting`으로 예약해 연속 클릭이 중복 RPC로 이어지지 않게 했어. Codex 실행 중·대기 메시지가 있을 때 압축을 거절하고, 수동 압축 중 새 메시지도 거절해 단일 worker 순서를 유지해.
- 수동 압축은 받은 이벤트의 스레드 ID, 새 압축 턴 ID, 시작된 `contextCompaction` 항목 ID가 모두 일치할 때만 완료 후보로 받아. 해당 턴이 정상 종료하면 완료 상태를 기록해. 명시적 실패는 `failed`, 연결 종료·완료 미확인은 `unknown`으로 정리하고 끊긴 RPC 연결을 닫아 다음 작업에서 새로 연결해. 항목 완료는 확인했지만 턴 종료 확인이 안 되면 완료로 기록하되 연결을 닫고 감사 로그에 종료 미확인을 남겨.
- `npm test`의 Rust 테스트 17개, `npm run build`, 같은 `esh-chat-mvp4.exe`의 `npm run build:desktop` 통과. 이벤트 ID 불일치 거부, 중복 예약, 실패·unknown 뒤 재요청을 단위 검사했어. 실제 WebView·Codex App Server 검사 `mvp-app/evidence/compact-20261022-234021/result.json`에서 수동 압축 RPC 1회, 압축 중→완료, 시작·완료 항목 ID 일치, 같은 스레드와 기존 메시지 2개 보존을 확인했어. Mock 응답은 없고 격리 DB를 사용했어. 연결 종료·명시적 실패를 실제 CLI에서 강제로 재현한 검사는 아니야.

## 클릭 후 포커스 테두리 정리 (2026-10-22)

- WebView가 클릭한 요소에 `:focus-visible`을 남겨도 테두리가 보이지 않도록 입력 방식을 구분해. 포인터 클릭 후에는 외곽선을 숨기고 Tab·방향키로 이동하면 중립색 포커스 선을 보여. 입력창 바깥 테두리도 클릭만으로 밝아지지 않게 했어. 버튼 선택 상태와 기능은 유지해.
- `npm run build`와 같은 `esh-chat-mvp4.exe`의 `npm run build:desktop` 통과. 실제 Tauri WebView 복원 검사 `mvp-app/evidence/panels-final-20261022-230417/result.json`의 `final-restored-gui.json`에서 클릭 후 `pointerOutline=none`, 키보드 이동 후 `keyboardOutline=solid`를 확인했어. 기존 실제 CLI 대화 6개, 구버전 기록 4개, 협업 패널 읽기 전용·크기 조절도 통과했어. 격리 DB 복사본을 썼고 새 AI 요청은 보내지 않았어.

## AI 협업 스레드 목록·상세 전환 (2026-10-19)

- 오른쪽 패널을 스레드 전체 목록과 선택한 상세 대화의 두 화면으로 분리했어. 상세 상단의 뒤로 가기를 누르면 목록으로 돌아가고, 중앙 협업 카드에서는 최신 스레드를 바로 열어. 세션을 바꾸면 목록으로 돌아가. 실제 collaborationId로 묶인 메시지만 표시하며 오른쪽은 계속 읽기 전용이야.
- `npm run build`, 동일 `esh-chat-mvp4.exe`의 `npm run build:desktop` 통과. 실행 중인 앱의 파일 잠금 때문에 사용자가 앱을 닫은 뒤 빌드했어. 최종 실제 WebView 복원 검사 `mvp-app/evidence/panels-final-20261019-234831/result.json`에서 스레드 목록→상세→뒤로 가기, 중앙 협업 카드→상세, 스레드별 기존 실제 CLI 메시지 6개·구버전 기록 4개 보존, 오른쪽 읽기 전용·패널 너비 조절을 확인했어. 격리 DB를 썼고 새 AI 호출은 없었어.

## 마크다운 제목·볼드 크기 균형 (2026-10-19)

- 일반 본문을 키운 뒤에도 중앙 H1/H2/H3가 21/18/16px, 협업 H1/H2가 18/16px로 남아 제목이 과하게 커 보였어. 중앙은 18/16/14px, 협업은 16/14/13px(H1/H2/H3)로 낮추고 제목 위아래 간격과 볼드 굵기를 줄였어. 볼드는 본문과 같은 글자 크기야.
- 동일 `esh-chat-mvp4.exe`의 `npm run build:desktop` 통과. 실제 WebView의 합성 마크다운 검증 `mvp-app/evidence/markdown-20261019-233946/result.json`에서 중앙 본문/볼드 13px·H1 18px, 협업 본문 12px·H2 14px, 표·코드·링크·HTML 차단과 원문 보존을 확인했어. 실제 AI 호출은 없었어.
- 앞선 `markdown-20261019-233832`, `233907`은 검증기가 답변 안의 볼드 대신 카드 상단 발화자 이름을 측정해 실패했어. 원인을 고쳐 재검사했고 실패 기록은 보존했어.

## 글자 크기 균형·창 상태 복원 (2026-10-19)

- 협업 상세 본문은 14px에서 12px로 낮추고 제목·표·코드도 함께 조정했어. 중앙 대화·입력창은 11px에서 13px, 왼쪽 탐색기 주요 항목은 11px에서 13px로 키웠어. 보조 정보도 8~9px 대신 대체로 10~11px로 올렸어.
- 시작 시 강제 최대화를 제거하고 정상 종료 때 창의 내용 영역 크기·바깥 위치·최대화 여부를 `window-state.json`에 저장해. 다음 실행은 이 상태를 복원해. 첫 실행은 1600×900 가운데 창을 보여. 화면 밖 위치는 가운데로 옮겨. 검증 시 `ESH_DATA_DIR`로 창 상태와 DB를 함께 격리해.
- `npm run build:desktop` 통과(동일 `esh-chat-mvp4.exe`). 실제 WebView 복원 검사 `mvp-app/evidence/panels-final-20261019-233342/result.json`에서 협업 본문 12px, 중앙 본문 13px 이상, 기존 실제 대화 6개·구버전 기록 4개 보존과 읽기 전용·패널 크기 조절을 확인했어.
- 실제 창 종료·재실행 검사 `mvp-app/evidence/window-restore-20261019-233329/result.json`에서 일반 창이 100,80 위치와 1120×720 바깥 크기로 정확히 복원되고 최대화 상태도 재실행 후 유지됐어. 첫 검증 `window-restore-20261019-232859`는 초기 생성 중인 작은 창을 선택해 시간 초과했고, 다음 `233105`는 같은 문제, `233228`은 바깥 크기를 내용 영역 크기로 설정해 테두리만큼 커졌어. 실패 증거를 보존했고 창 선택 및 저장 기준을 고쳐 최종 검사를 통과했어. 강제 종료 시 마지막 창 상태 저장은 보장하지 않아.

## 협업 패널 가독성 조정 (2026-10-19)

- 오른쪽 검토 목록 제목을 2줄까지 읽을 수 있게 하고 본문 글자를 10px에서 14px로 키웠어. 메시지 카드는 패널 너비를 쓰며, 긴 표는 카드 내부에서 가로 스크롤해. 목록 높이를 제한하고 상세 상단의 중복된 긴 요청 제목은 짧은 `검토 대화` 헤더로 바꿨어. 기본 협업 패널 너비는 340px에서 520px로 늘렸어. 기존 저장 너비가 이보다 좁으면 한 번만 520px 이상으로 보정하고 이후 사용자 조절 값은 유지해.
- `npm run build`, 동일 `esh-chat-mvp4.exe`의 `npm run build:desktop` 통과. 처음 데스크톱 빌드는 실행 중인 PID 51532의 파일 잠금으로 멈췄고, 사용자가 앱을 닫은 뒤 같은 파일명으로 다시 빌드했어.
- 실제 Tauri WebView 복원 검사 `mvp-app/evidence/panels-final-20261019-232056/result.json` 통과. 저장된 실제 CLI 대화 6개와 구버전 기록 4개를 격리 DB에서 복원했고, 협업 본문 계산 글자 크기 14px 이상·카드 폭·중복 제목 제거, 읽기 전용·패널 크기 조절을 확인했어. 검증 창의 협업 패널 초기 폭은 550px이었어. 수동 픽셀 비교는 하지 않았어.

## 창 크기·포커스·헤더·사이드바 정리 (2026-10-19)

- 새 창은 작업 영역에 최대화해 열어. 클릭 뒤 남는 파란 포커스 테두리를 중립색의 얇은 키보드 포커스 표시로 바꾸고 마우스 포커스의 불필요한 외곽선을 없앴어.
- 중앙 헤더의 Codex Lead 제목을 선택 세션 이름으로 바꾸고 모델을 아래에 표시해. 헤더 오른쪽 상태 표시와 협업 패널 하단 읽기 전용 안내를 제거했어. 실제 에이전트 상태는 왼쪽 Agents 화면의 Connections 카드에서 계속 보여.
- 입력창은 3줄/최소 84px에서 2줄/최소 58px로 낮췄어. 수동 크기 조절과 모델·전송·압축 기능은 유지해.
- `npm run build`, `npm run build:desktop` 통과. 같은 `esh-chat-mvp4.exe`의 실제 WebView 복원 검사(`mvp-app/evidence/panels-final-20261019-231201/result.json`)에서 기존 메시지 6개·구버전 기록 4개 보존, 선택 세션 이름 표시, 제거한 표시의 부재, 입력창 높이, Agents 화면의 연결 상태, 협업 패널 읽기 전용·크기 조절을 확인했어. 격리된 DB 복사본을 사용했어.
- 새 창 최대화 상태는 실행 파일을 격리된 DB로 띄운 뒤 Win32 `IsZoomed`로 확인했어(`mvp-app/evidence/window-default-20261019-231223/result.json`). 클릭 시 포커스 색상의 픽셀 비교는 하지 않았고 CSS와 프런트 빌드로 확인했어.

## 창 가장자리 여백 제거 (2026-10-19)

- 사용자가 표시한 창 안쪽 테두리 여백은 `v0-shell`에 넣어둔 `margin: 12px`, 줄어든 `width/height`, 둥근 테두리·그림자 때문에 생겼어. WebView 영역 전체를 채우도록 이 프레임 스타일을 제거했어. Windows 제목 표시줄은 그대로야.
- `npm run build`와 동일 `esh-chat-mvp4.exe`의 `npm run build:desktop` 통과. `mvp-app/evidence/panels-final-20261019-225948/result.json`의 실제 WebView 복원 검사에서 기존 메시지 6개·구버전 기록 4개 보존, 패널 읽기 전용·크기 조절이 통과했어. 여백 값은 CSS에서 0으로 확인했고 픽셀 캡처 비교는 하지 않았어.

## RECENT 프로젝트·세션 트리 스타일 (2026-10-19)

- 사용자 이미지에 맞춰 프로젝트 행에 폴더 아이콘과 세션 수를 두고, 세션 아이콘 대신 세로 연결선과 텍스트 행을 적용했어. 선택 행은 어두운 배경과 둥근 오른쪽 모서리로 표시해. 프로젝트/세션 선택·검색·생성·Codex 기록 추가는 기존 React 상태와 Tauri command를 사용해. 프로젝트별 추가 버튼은 마우스를 올리거나 키보드 포커스가 있을 때 오른쪽에 아이콘으로 나타나며 세션 행을 밀지 않아.
- `npm run build`와 동일 `esh-chat-mvp4.exe`의 `npm run build:desktop` 통과. 최종 `mvp-app/evidence/panels-final-20261019-225602/result.json`의 실제 WebView 복원 검사에서 기존 메시지 6개·구버전 기록 4개 보존, 패널 읽기 전용·크기 조절이 통과했어. DB는 검증용 복사본을 썼어. 화면 캡처는 다른 창이 가려 실제 ESH 픽셀 비교 근거로 사용하지 않았어.

## 화면 확대·축소 단축키 (2026-10-19)

- 메인 Tauri 창에서 WebView2 기본 `zoomHotkeysEnabled`를 켰어. Ctrl+`+`/`-`로 화면 전체를 확대·축소해. 화면 배율은 대화·세션 데이터와 분리돼 있어.
- 현재 창 설정의 값과 설치된 Tauri CLI 스키마의 지원 필드를 확인했어. 동일 `esh-chat-mvp4.exe`의 `npm run build:desktop`이 통과했어. 실제 키 입력에 따른 배율 변화는 자동화로 관측하지 않았어.

## 컨텍스트 원형 표시·상세 팝업 (2026-10-19)

- 기존 컴포저 위 인라인 컨텍스트 카드를 전송 버튼 옆 원형 사용률 표시로 옮겼어. 원을 누르면 기존 실제 토큰 사용량·압축 상태·수동 압축 버튼을 팝업에서 보여. 바깥 클릭과 Esc로 닫고 세션 전환 시 팝업 상태를 초기화해.
- 비율은 기존 `snapshot.contexts`의 `last.totalTokens / modelContextWindow`를 그대로 사용해. 한도가 없거나 값이 오래되면 `—`로 표시하며 가짜 수치를 만들지 않아. 압축 호출과 비활성 조건, Rust·SQLite·CLI 경로는 변경하지 않았어.
- `npm run build`, `npm test`(Rust 15개), 동일 `esh-chat-mvp4.exe`의 `npm run build:desktop`이 통과했어. `mvp-app/evidence/compact-20261019-224151/result.json`에서 실제 WebView 팝업 열기, 실제 Codex `thread/compact/start` 1회, 압축 중→완료, item ID 일치, 같은 thread와 메시지 2개 보존을 확인했어. Mock AI 응답은 없고 보호 설정·인증·`technical-spike/` 해시도 유지됐어.
- 앞선 `compact-20261019-223710`, `223822`, `223933`, `224039`는 GUI 검증 스크립트가 새 팝업을 생성 다이얼로그로 오인하거나 입력 대상을 잘못 잡아 중단된 실패 기록이야. 실패 증거를 보존했고, 생성 다이얼로그 선택자를 `.workspace-dialog`로 한정한 뒤 최종 검사가 통과했어. 이 검사는 실제 압축 경로를 확인한 것이며, 팝업의 모든 화면 크기나 모든 키보드 경로를 포괄하진 않아.

## v0 ESH UI 통합 (2026-10-19)

- 입력 자료: 사용자가 제공한 `collaborative-agent-ui-design.zip`과 배포 화면 `https://esh-alpha.vercel.app/`. ZIP은 단순한 초기 3패널 목업이고 배포 화면은 Connections 카드와 리뷰 목록 등 표시가 더 풍부해. 배포 화면의 시각 구성을 기준으로, ZIP의 컴포넌트 경계·색상·간격을 참고했어. ZIP의 하드코딩된 세션·에이전트·리뷰 데이터와 동작 시뮬레이션은 가져오지 않았어.
- 이전 화면: 프로젝트 트리, 세션 목록, 대화 카드, 협업 메시지가 독립된 자체 CSS 패널에 모두 펼쳐졌어. 이후 화면: v0의 ESH Studio 탐색기, Codex Lead 대화와 컴포저, 리뷰 스레드 목록/선택 상세를 `main.tsx`의 기존 snapshot·command에 연결했어. 시각 스타일은 `src/v0.css`, SVG 아이콘은 `src/Icons.tsx`에 분리했어.
- 실제 연결: 방/세션 생성·선택·검색, Codex 기록 copy/resume 추가, Codex/Arch 상태, main/collaboration 메시지, 리뷰 스레드 선택, 메시지 전송, 모델·추론 강도 저장, 실행 중단, 컨텍스트 사용량·수동 압축을 기존 경로로 사용해. 탐색기/협업 패널 접기와 너비 저장도 유지해. 오른쪽은 계속 읽기 전용이야.
- v0의 Claude Code/Gemini CLI/OpenCode·가짜 세션·가짜 승인 기록은 제거했어. 파일 첨부는 현 MVP에 백엔드 경로가 없어서 비활성화하고 화면에 미지원이라고 표시해. v0의 Agents 메뉴는 현재 연결된 Codex/Antigravity 상태만 보여.
- Rust 백엔드, SQLite schema 4, CLI 통신·Blocking Delegation·전달 기록은 변경하지 않았어. 실행 파일 이름과 앱 식별자도 유지했어.
- 최종 `npm run build:desktop` 통과(같은 `esh-chat-mvp4.exe`). `npm test` 15개와 `node scripts/verify_external_threads.mjs` 통과. 첫 데스크톱 빌드는 사용자가 실행 중이던 exe 잠금으로 멈췄고, 사용자가 앱을 닫은 뒤 같은 이름으로 빌드했어.
- 실제 Tauri/CLI 검증: `mvp-app/evidence/panels-20261019-222415/result.json`에서 Mock 없이 사용자 입력, Arch 첫·후속 검토, 같은 Codex 턴 재개, 호출별 wire 응답 1회, 대화/협업 분리, 재시작 복원이 통과했어. `mvp-app/evidence/import-ui-20261019-222352/result.json`에서 실제 Codex 목록과 추가 화면이 통과했어. 최종 문구·미응답 상태 표시까지 반영한 exe는 `mvp-app/evidence/panels-final-20261019-222856/result.json`에서 기존 메시지 6개·구버전 기록 4개 보존, 읽기 전용 패널과 크기 조절이 통과했어. 보호 설정과 `technical-spike/` 해시도 유지됐어.
- 파일 첨부, 추가 에이전트 연결, 실제 검토 결과의 의미 반영 판정은 현재 MVP 범위 밖이야. 이번 UI 변경 후 수동 압축 완료 경로를 새로 실행하진 않았고, 기존 기능의 버튼 연결·Rust 단위 테스트·최종 빌드만 확인했어.

## 수동 컨텍스트 압축 버튼 (2026-10-19)

- 입력창 위 컨텍스트 상태에 **컨텍스트 압축** 버튼을 추가했어. 선택 세션에 threadId가 있고 실행 중이 아니며 전달 대기 메시지가 없을 때만 눌러.
- worker 큐에 압축 job을 넣고, worker가 그 세션의 Codex 연결을 확보한 뒤 `thread/compact/start`를 호출해. 별도 프로세스로 같은 thread를 동시에 잡지 않아.
- 완료 판정은 `contextCompaction` item의 started ID와 completed ID가 일치할 때만 해. 이어서 그 턴의 `turn/completed`를 최대 30초 기다려 다음 턴과 겹치지 않게 하고, 확인 못 하면 `compact_turn_close_unconfirmed`를 audit에 남겨. 완료 미확인은 `unknown`, 요청·실행 오류는 `failed`이며 자동 재시도는 없어.
- Rust 테스트 15개 통과(`context::tests::manual_compact_requires_bound_thread_and_clears_previous_item` 추가). TypeScript/Vite 빌드 통과.
- 실제 프로토콜 확인: `evidence/compact-probe-20261019/probe.json`. 설치본 Codex에서 새 검증 스레드를 만들고(모델 턴 없음) `thread/compact/start`를 호출했어. 응답 `{}`, 이어서 `turn/started` → `item/started(contextCompaction)` → `thread/tokenUsage/updated` → `item/completed(같은 item ID)` 순서를 실제로 받았어.
- 데스크톱 빌드는 앱 종료 후 통과했어(`esh-chat-mvp4.exe`, 2026-10-19 18:54, 진단 코드 제거 상태).
- 당시 자동화 환경에서는 WebView2가 초기화되지 않아 실제 GUI 완료 검증을 남겨뒀어. 이후 동일 mvp4 실행 파일의 `python scripts/verify_compact.py`가 통과했고, 새 팝업에서 실제 수동 압축 요청·item ID 일치·GUI 상태 전이·메시지 보존을 확인했어. 근거는 상단 `compact-20261019-224151/result.json`이야.


## 컨텍스트 사용량·압축 상태 (동일 mvp4)

- 입력창 위에 최근 보고 기준 컨텍스트 추정 비율/토큰과 압축 상태를 추가했어. 상세 정보에서 최근 입력·출력·캐시, 누적 토큰, 보고/압축 시각을 구분해. last.totalTokens를 분자로 사용하고, 한도 미제공·모델 변경·압축 후 갱신 전에는 비율을 만들지 않아.
- 실제 thread/tokenUsage/updated와 contextCompaction 시작·완료를 처리해. 관측 전/압축 중/완료/실패/중단/확인 필요 상태와 완료 횟수를 저장해. 비정상 종료 시 미완료 압축은 unknown이며 자동 재개하지 않아. 사용자 메시지는 삭제하거나 요약으로 대체하지 않아.
- SQLite schema 4의 session_context를 추가했어. 로컬 세션/thread 일치로 구분하고 재시작에 복원해. 구버전 DB 마이그레이션과 잘못된 수치 거부, 중복 완료·세션 분리·취소/재시작 불확실성 처리를 포함한 Rust 테스트 14개가 통과했어. TypeScript/Vite와 동일 mvp4 데스크톱 빌드도 통과했어.
- `mvp-app/evidence/context-20261018-170755/result.json`, `stream`이 아닌 `contextUsage`를 담은 `context-gui.json`, `audit.json`, `restored-gui.json`에서 실제 사용량 이벤트 5개/자동 압축 완료 2회, 화면의 압축 중→완료, 최근 수치 일치, 미사용 세션 분리, 실제 Arch 응답 후 같은 thread/turn 재개와 도구 응답 wire write 1회, 재시작 기록/상태 복원이 통과했어. Mock 응답은 없어. 보호 설정·인증·technical-spike 해시도 유지됐어.
- 실제 자동 압축 검증은 debug probe 전용 프로세스 override `model_auto_compact_token_limit=3000`을 사용했어. 일반 실행이나 사용자 설정 파일은 바꾸지 않았어. 기본 임계값까지 장문 대화를 늘린 스트레스 검증은 아니야. 수동 압축 버튼·임계값 설정·계정 한도 UI는 구현하지 않았어.
- 최초 `context-20261018-170535`는 검증기가 숨겨진 안내의 “압축 완료 이벤트를 아직 관측하지 않았어”를 완료 상태로 오인해 실패했어. 실제 상태 속성으로 검사를 고쳤으며 최초 실패 증거는 보존했어.
- 최종 빌드의 별도 복원 검사 `mvp-app/evidence/panels-final-20261018-170909/result.json`에서 기존 실제 대화 6개와 구버전 항목 4개, 읽기 전용 협업 패널/크기 조절을 확인했어. 원본 DB 대신 복사본을 마이그레이션했고 새 AI 턴을 실행하지 않았어.

## Codex 실시간 답변 — 동일 mvp4 갱신 및 실제 검증 완료

- `stream.rs`에서 공개 agentMessage delta를 같은 메시지 ID에 누적하고 약 40ms 간격으로 DB/UI를 갱신해. 완료 원문으로 확정하며 중복 완료 행을 방지해.
- 작성 중/중단된 답변 표시, 부분 마크다운 렌더링, 같은 메시지 갱신 시 스크롤, 중단·종료 시 부분 답변 보존과 재시작 interrupted 처리를 구현했어. Arch는 기존 검증된 최종 리뷰 전달을 유지해.
- 현재 코드의 Rust 단위 테스트 12개와 TypeScript/Vite 빌드가 통과했어. `scripts/verify_stream.py`에 실제 WebView의 완료 전 부분 응답 관찰, 중단 보존, 같은 연결/턴 Arch 후 재개, 도구 응답 1회, 재시작 복원 검사를 추가했어.
- 최초 데스크톱 빌드는 실행 중인 exe 잠금(os error 5)으로 실패했고, 사용자 앱 종료 후 같은 `target/debug/esh-chat-mvp4.exe`에 빌드가 통과했어. 별도 버전 exe는 만들지 않았어.
- `mvp-app/evidence/stream-20261018-165615/result.json`, `stream-gui.json`, `audit.json`, `restored-gui.json`에서 실제 CLI/WebView 검증이 통과했어. 실제 delta 934개와 최종 원문 일치, 완료 전 동일 메시지의 화면 갱신 143회, 최종 행 중복 없음, 중단된 부분 답변 보존, 같은 연결 유지, 실제 Arch 응답 후 같은 thread/turn 재개, 도구 응답 1회, 재시작 메시지 복원을 확인했어. Mock AI 응답은 없으며 보호 설정·인증 및 technical-spike 해시가 유지됐어.
- 강제 종료 직전 저장되지 않은 최대 약 40ms의 버퍼는 유실될 수 있어. 정상 중단에서는 남은 버퍼를 저장해.

## 실행 중단 버튼

- 현재 선택한 세션의 실행이 있을 때 입력창에 중단 버튼을 표시해. 실행 ID를 대조해 오래된 버튼 요청을 거부하고 중단 중 중복 클릭/같은 세션 전송을 막아.
- Codex는 turn/interrupt와 실제 interrupted 종료 event로 중단을 확인해. Arch 실행 중에는 프로세스 종료 → 열린 도구에 취소 결과 1회 전달 → Codex 중단 순서를 사용해. 정상 중단 후 앱·연결·thread와 기록을 유지해.
- 같은 세션의 기존 queued 입력은 cancelled로 보존하고 다음 공유 맥락에서 제외해. 다른 세션 입력과 중단 완료 후 새 입력은 유지해.
- TypeScript/Vite와 Rust 테스트 10개가 통과했어. `mvp-app/evidence/cancel-20261018-163622/result.json`에서 실제 GUI 버튼으로 Codex 생성 중단, Arch 실행 중단과 PID 종료 확인, 열린 도구 취소 응답 1회, 같은 Codex 연결/thread에서 새 요청 2회 복구, queued 입력 취소/미실행, 오래된 runId 거부, 재시작 기록 복원이 통과했어. Mock 응답은 사용하지 않았고 보호 설정·인증·spike 해시도 유지됐어.
- 첫 실제 검사 `cancel-20261018-163431`은 turn/start 응답 뒤 실제 turn/started 전 interrupt가 거절되는 경합으로 unknown 처리됐어. 실패는 보존하고 실제 시작 이벤트 수신 후 interrupt 재요청으로 보완했어.
- 정상 2회 Arch 리뷰/동일 턴 재개는 `panels-20261018-163711`의 실제 CLI/GUI 증거에서 확인됐어. 해당 전체 스크립트는 마지막 legacy migration 화면이 공용 WebView의 빈 세션 선택을 따라가 실패했으므로 전체 통과로 집계하지 않아. 동일하게 restore-history를 사용하도록 스크립트를 보완했어. 정상 리뷰 부분만 재검사한 범위 제한 결과는 `cancel-normal-20261018-163930/result.json`에 있어(새 AI 호출 없음).
- 최종 빌드의 별도 복원 회귀 `panels-final-20261018-163830/result.json`에서 기존 대화 6개/구버전 항목 4개 보존과 세션 전환·패널 동작이 통과했어. 결과 경로는 모두 mvp-app/evidence 기준이야. 중단 확인 실패 분기와 실행 중 강제 종료의 모든 경합을 실제로 검증했다는 뜻은 아니야.

## AI 메시지 마크다운 렌더링

- react-markdown + remark-gfm을 추가해 중앙 Codex 답변과 오른쪽 AI 협업 메시지에 제목·강조·목록·인용·표·취소선·코드 블록·읽기 전용 체크리스트를 표시해. 사용자 메시지와 DB 원문은 유지해.
- HTML 실행은 허용하지 않아. http/https 링크만 활성화하고 외부 이미지는 alt 텍스트로 대체해. 긴 코드·표에는 가로 스크롤을 적용해. 문법 색상 강조·수식·Mermaid는 이번 범위가 아니야.
- TypeScript/Vite 빌드와 SSR 구조·안전성 검사가 통과했어. 동일 mvp4 데스크톱 빌드와 실제 WebView 검사도 통과했어. `mvp-app/evidence/markdown-20261018-161152/result.json`에서 렌더링 구조, 읽기 전용 체크리스트, 사용자 원문, HTML·위험 URL·외부 이미지 차단, 코드 가로 스크롤과 카드 넘침 방지를 확인했어. 이 검사는 합성 렌더링 fixture이며 실제 AI 응답 성공으로 집계하지 않아. `panels-final-20261018-161130/result.json`에는 기존 실제 대화 6개 복원과 구버전 항목 4개 보존·패널 동작 통과 기록이 있어. 최초 fixture 검사(161127)는 사용자 본문이 아닌 발화자 이름의 strong까지 검사해 실패했으며 본문으로 범위를 바로잡았고 실패 증거는 보존했어.

## 모델·추론 강도 선택 추가 (동일 mvp4 실행 파일 갱신)

- 입력창 안에 Codex 모델과 추론 강도 선택을 추가했어. 실제 `model/list`가 반환하는 모델·지원 강도만 표시하고, 모델 변경 시 해당 모델의 기본 강도를 저장해.
- 세션별 SQLite 설정 저장(schema 3), 세션 전환/재시작 복원, 다음 턴 적용, 지원하지 않는 조합 거부, 목록 실패/재조회 UI를 구현했어. 대기 중인 입력에는 전송 당시 설정을 고정해.
- 실행 중인 Arch 검토와 열린 Codex 호출은 그대로 유지해. Antigravity 모델 설정은 이번 범위가 아니야. 기존 미설정 세션은 기존 모델·낮음 추론을 사용하는 호환 동작을 유지해.
- 새 debug GUI 검증 thread에 threadSource=esh-verification을 부여해 실제 사용자 목록에서 검증 기록이 다시 섞이는 것을 줄였어.
- 빌드/타입 검사와 Rust 단위 테스트 9개가 통과했어. `scripts/verify_models.py`의 실제 CLI/WebView 검증 근거는 `mvp-app/evidence/models-20261018-160229/result.json`, `audit.json`, `models-gui.json`, `restored-gui.json`이야.
- 실제 Codex 0.163.0-alpha.10.2, agy 1.4.0에서 GPT-6.1 Sol/low → GPT-6 Luna/medium으로 같은 thread를 유지하며 변경했고, 두 번째 턴의 실제 Arch 검토 뒤 같은 turn 재개/도구 응답 1회를 확인했어. GUI 세션별 복구, 잘못된 설정 차단, 없는 Codex exe로 목록 오류 표시도 통과했어. 설정·인증·technical-spike 보호 파일 해시가 유지됐어.

추가 회귀 확인: `evidence/panels-final-20261018-160453/result.json`에서 최종 빌드의 구버전 대화 6개 복원, 기존 항목 4개 보존, 세션 전환·패널 읽기 전용/크기 조절이 통과했어. 앞선 `panels-final-20261018-160403`의 실패는 공용 WebView가 다른 검증 DB의 빈 기본 세션 선택을 기억해서 검사 대상이 달라진 경우였어. restore-history 검증 모드에서 기록이 있는 세션을 명시적으로 선택하도록 검사 경로를 보완했고, 일반 사용자의 선택 복원 동작은 바꾸지 않았어. 실패 증거도 보존했어. 목록 분류 회귀 검사도 통과했어.

## 실행 파일 정리와 후속 빌드 규칙

사용자의 최신 요청에 따라 `target/debug`의 `esh-chat.exe`, `esh-chat-mvp1.exe`, `esh-chat-mvp2.exe`, `esh-chat-mvp3.exe`를 삭제했고 `esh-chat-mvp4.exe` 하나만 남겼어. 삭제 전후 mvp4 SHA-256이 동일한지 확인했어. 소스·DB·검증 증거는 삭제하지 않았어.

앞으로는 이름을 올리지 않고 `esh-chat-mvp4.exe`를 같은 경로에 갱신해. 빌드 대상과 스크립트는 이미 이 이름을 사용하므로 코드 변경이나 재빌드는 하지 않았어. 실행 중 잠기면 사용자에게 앱을 닫도록 안내하고 별도 이름의 실행 파일을 만들지 않아.

## 빌드 전 .d / .pdb 정리

`scripts/build.ps1`은 데스크톱 빌드 전에 `target/debug` 바로 아래의 `esh-chat*.d`, `esh_chat*.pdb`를 제거해. 의존성 캐시 하위 디렉터리, exe, 소스, DB, 검증 증거는 건드리지 않아. 빌드 후 새로 생성된 파일은 다음 빌드 때 정리해.

PowerShell 구문 검사와 실제 스크립트의 정리 구간 실행을 확인했고, 기존 `.d`/`.pdb` 6개를 삭제했어. mvp4 exe SHA-256은 동일해. 이 변경은 빌드 전처리라 앱 재빌드나 CLI 검증은 실행하지 않았어.

## mvp4 — 중복 프로젝트·룸 선택 행 제거

- 프로젝트 제목 밑에 반복 표시되던 룸 버튼을 제거했어. 제목에 룸 선택, 세션 개수, 펼치기·접기 표시를 합쳤어.
- 같은 프로젝트를 접고 펼칠 때 현재 세션을 유지해. 다른 프로젝트 제목을 누르면 그 룸의 첫 세션(없으면 빈 룸 화면)을 선택해.
- 새 세션/Codex 추가 버튼과 개별 세션 선택, CLI·SQLite 처리는 유지했어. 실행 파일은 `esh-chat-mvp4`야. TypeScript/Vite·데스크톱 빌드 통과. 실제 WebView 복원/세션 전환/패널 검사(`evidence/panels-final-20261018-154917/result.json`)와 실제 Codex 목록·추가 화면 검사(`evidence/import-ui-20261018-154920/result.json`)가 통과했어. 두 증거 경로는 `mvp-app/` 기준이고 새 모델 턴은 실행하지 않았어.

## mvp3 — ESH 콘솔 창 숨김

- `main.rs`의 Windows GUI 서브시스템 설정을 release 전용에서 Windows의 모든 빌드로 변경했어. 배포 중인 debug exe 자체가 콘솔 창을 만들던 원인이야.
- Codex·Arch 자식 프로세스는 기존 CREATE_NO_WINDOW 설정을 유지해. stdio 통신, 프로세스 종료 관리, SQLite 경로는 변경하지 않았어.
- 실행 파일과 빌드·검증 명령을 `esh-chat-mvp3`로 맞췄어. TypeScript/Vite·데스크톱 빌드 통과. 실제 PE 헤더에서 mvp2는 console(3), mvp3는 Windows GUI(2)임을 확인했어. `mvp-app/evidence/panels-final-20261018-154608/result.json`에서 실제 창의 대화 복원·기록 보존·패널 동작·정상 종료 검사가 통과했어. 새 AI 호출은 실행하지 않았어.

## mvp2 — 탐색기 가독성 임시 개선

- 프로젝트 제목 14px, 세션 제목 13px, 세션 부가 정보 11px로 키웠어.
- 탐색기 보조 글자 대비, 프로젝트 사이 구분선과 간격, 세션 선택 배경·청록색 표시를 강화했어.
- 세션 추가/가져오기 버튼을 키우고 좁은 패널에서 줄바꿈하도록 했어. 긴 세션 제목은 두 줄로 제한해.
- 메시지·에이전트·SQLite·CLI 처리 코드는 변경하지 않았어. 모델·추론 설정은 여전히 미구현이야.
- 실행 대상·빌드·검증 스크립트 이름을 `esh-chat-mvp2`로 맞췄어. TypeScript/Vite와 데스크톱 빌드가 통과했어. `mvp-app/evidence/panels-final-20261018-045121/result.json`에서 실제 WebView의 기존 대화 6개 복원, 구버전 항목 4개 보존, 세션 전환, 패널 읽기 전용/크기 조절이 통과했어. 새 AI 턴은 실행하지 않았어. 보조 글자와 탐색기 배경의 계산상 대비는 약 8.99:1이야.

## 확인 방법과 근거 범위

`mvp-app/src/`, `src-tauri/src/`, Cargo/npm/Tauri 설정, 빌드·검증 스크립트와 실제 저장된 result/audit/WebView 증거를 확인했어. 아래의 “통과”는 해당 기록 시점과 실행 범위에 한정돼. 오래된 결과를 최신 전체 앱의 재검증으로 간주하지 마.

이번 문서 작업에서 다시 실행한 검사:

| 검사 | 결과 | 범위 |
|---|---|---|
| `npm test` | 7 passed, 0 failed | 현재 Rust 단위 테스트. 실제 AI 모델 호출 없음 |
| `node scripts/verify_external_threads.mjs` | passed | 실제 저장 응답 27개, 테스트 경로 기록 9개 분류 및 일반 대화 보존·제목 처리. 경계 사례용 합성 입력도 포함 |

Rust 7개 검사는 timeout 설정 범위, 오래되거나 빈 Arch 구조화 결과 거부, 채널·call 연결, 재시작 상태 처리와 중복 예약 차단, legacy migration, 세션별 맥락·버전 분리, copy/resume 저장 의미를 검사해. CLI 통신·OS 강제 종료·모델의 의미적 요구사항 반영을 이 단위 테스트가 증명하진 않아.

이번 작업에서는 프런트/데스크톱 빌드나 실제 AI 협업을 새로 실행하지 않았어. 아래는 기존 증거를 읽어 확인한 결과야. 경로 기준은 `mvp-app/`이야.

| 증거 | 실제 확인된 내용 |
|---|---|
| `../technical-spike/VERIFIED.json` | Codex Dynamic Tool 약 12초 대기, 호출 응답 wire write 1회·완료 event 1회, 같은 thread 후속 대화. 별도 agy 2턴 세션 재개 성공. 둘을 결합한 GUI 검증은 아님 |
| `evidence/gui-20261018-032922/result.json` | 초기 실제 GUI/CLI 연결, 같은 Codex thread/turn 복귀, Arch 결과 1회 전달, 메시지/상태 재시작 복구, 연결 오류 표시 |
| `evidence/panels-20261018-035628/result.json` | 실제 Arch 검토 2회(후속 포함), 같은 Codex thread/turn, 호출별 응답 1회, 중앙 2개/협업 4개 메시지 분리, 오른쪽 읽기 전용·패널 크기 조절, 구버전 기록 보존, 재시작·연결 오류 |
| `evidence/workspace-20261018-041248/result.json` | 실제 룸·세션 생성, A/B 맥락 분리, A 재개, 원본 목록 조회·copy·resume, copy의 실제 Arch 검토, 도구 응답 1회, 룸·세션·메시지 복원 |
| `evidence/panels-final-20261018-041522/result.json` | 당시 최종 빌드에서 저장된 실제 대화/구버전 DB 복사본 복원. AI를 새로 실행한 검사는 아님 |
| `evidence/import-ui-20261018-042551/result.json` | mvp1 이름 변경 뒤 실제 CLI 목록 + WebView 검사. 해당 페이지의 자동 실행/검증 항목 13개 기본 숨김·체크 시 표시, 검색 버튼 56px, 선택 후 이름·추가 버튼 정상. 새 모델 턴 실행 없음 |

검증 폴더의 `*-gui.json`과 `audit.json`/DB에 상세 근거가 있어. 각 result에는 확인 대상 설정·인증 및 spike 원본 변경 없음도 기록돼 있어. 이 해시 검사는 지정한 파일 목록에 대한 것이며 컴퓨터 전체 파일 불변 검증은 아니야.

`evidence/gui-20261018-032444/`, `evidence/workspace-20261018-041153/`에는 `passed=false`가 있어. 실패 기록은 그대로 보존하며 통과 건수에 포함하지 않아. 후자는 구버전 실행 파일 사용으로 probe 모드가 맞지 않았던 이력이야(`WORKSPACE_VERIFICATION.md`와 해당 GUI 실패 기록 참고).

## 완료된 구현

### 대화·협업

- Tauri 2 + React 화면에서 실제 Codex/Antigravity CLI 결과를 표시해. 운영용 Mock 응답 경로는 없어.
- Codex Lead가 Dynamic Tool로 Arch 검토를 요청하고 응답을 기다려. Arch 종료 뒤 같은 도구 호출에 결과를 반환해 Codex가 같은 턴에서 이어 답해.
- 리뷰 요청·후속 요청·Arch 응답은 callId로 연결된 오른쪽 읽기 전용 타임라인에 표시해. 중앙은 사용자와 Codex 답변이야.
- 사용자 메시지는 SQLite에 즉시 저장해. 검토 중 메시지도 받으며 같은 로컬 세션의 대기 요구사항을 리뷰와 분리해 전달하는 코드가 있어. **이 경합 시나리오의 실제 통합 검증은 아래 미완료 항목이야.**
- 오류·CLI 시작 실패는 GUI 오류로 표시해. Arch 실패는 성공 리뷰로 대체하지 않고 `success=false`로 Codex에 반환해.
- 기본 120초/설정 30~600초, Arch 자동 재시도 없음, 리뷰 ID/버전/결과 검사, 턴당 Arch 실행 최대 3회가 구현돼.

### 탐색기·저장

- 프로젝트·룸 생성, 룸 안의 세션 생성·선택, 세션별 맥락/버전 분리, 앱 전체 직렬 실행 큐가 있어.
- 기존 Codex 세션의 로컬 목록·검색·추가 페이지 조회, 공개 텍스트 copy, 원본 thread resume을 제공해.
- 이름 없는 ESH 세션의 내부 전달 문구를 목록 제목에서 정리해. 자동 실행·검증 기록은 기본 숨김/체크 시 표시해.
- 세션 선택 시 이름 자동 입력, 검색 결과 초기화·오래된 조회 응답 무시, 검색 버튼 줄바꿈 수정이 들어 있어.
- SQLite schema 4와 legacy migration, 메시지 순서·생성 시간·발화자·채널·식별자·상태 저장 및 복구가 있어.
- 창 종료 정리와 재시작 시 미완료 실행 unknown/queued 입력 interrupted 처리가 있어. 열린 RPC를 자동 재개하지 않아.
- 3패널 드래그·키보드 크기 조절, 선택 세션·패널 너비 복구를 제공해.

## 미완성·미구현

| 항목 | 현재 정확한 상태 |
|---|---|
| Arch 모델 선택 | Codex 모델·추론 선택은 구현 완료. Arch는 `gemini-3.8-flash-high` 고정이며 별도 선택 UI 없음 |
| 컨텍스트 확장 | 사용량·압축 상태 표시/SQLite 저장과 수동 압축 버튼 구현. 임계값 설정 UI·압축 실패 복구 UX는 없음. GUI 검증 범위는 상단 후속 기록 참조 |
| 중단 범위 확장 | 현재 실행 중단 버튼은 구현. 개별 queued 항목 취소·작업별 영속 상태 이력은 없음 |
| 개발 작업 | 작업 폴더 연결, 파일 탐색·수정, 터미널 실행, 승인 UI, diff, Git/worktree 관리 없음 |
| Arch 지속 세션 | spike에서는 resume 검증, 실제 앱에서는 리뷰마다 새 agy 프로세스/대화 실행 |
| 추가 에이전트 | QA 에이전트·역할 편집·임의 provider 추가 없음 |
| 사용자 메시지 의미 반영 확인 | queued/delivered/interrupted만 사용. 별도 applied 상태나 의미적 확인 없음 |
| 다중 작업 실행 | 전체 worker 하나. 세션별 병렬 실행·job 상태 이력·영속 실행 큐 없음 |
| 외부 세션 동기화 | 가져온 시점의 공개 텍스트만 복사. 외부 앱의 새 발언 실시간 동기화 없음 |
| 룸·세션 관리 확장 | 생성·조회·선택·가져오기만 있음. 이름 변경·삭제·이동·내보내기 없음 |
| 출력 UI | Codex 공개 답변 스트리밍·부분 답변 보존 구현. Arch는 검증된 완료 결과 표시. 첨부 업로드 없음. AI 메시지 Markdown 렌더링 구현 |
| 배포 | debug exe 빌드와 로컬 실행. bundle 비활성화, 설치 프로그램·자동 업데이트 없음 |
| 상태 관리 라이브러리·에디터 | Zustand, shadcn/ui, xterm.js, Monaco 미도입 |

## 알려진 제한·코드에서 확인한 개선 필요 사항

재현된 과거 UI 문제와 현재 남은 위험을 구분했어. 아래 코드상 위험은 모두 실제 장애를 재현했다는 뜻이 아니야.

1. **검증 기록 분류는 아직 한정적이야.** 이전 3개만 숨겨 다른 테스트 기록이 보이던 문제는 실행 방식·경로 분류로 보완했고 mvp1 GUI 검사 통과했어. 하지만 현재도 특정 기존 ID 3개와 특정 테스트 경로를 포함한 규칙이야. 향후 모든 테스트를 식별하는 provenance 체계는 없어. 페이지 전체가 숨김 대상이면 목록이 비어 보여도 “더 불러오기”로 다음 페이지를 읽어야 해.
2. **같은 원본을 동시에 쓰는 문제를 막는 전역 잠금이 없어.** 가져오기 시 active 확인만 해. 다른 프로세스가 그 뒤 원본을 쓰거나 앱 두 개가 같은 ESH DB를 열면 상태 복구/쓰기 경합이 생길 수 있어. 실행 파일 이름이 달라도 앱 identifier와 DB는 같아.
3. **컨텍스트가 중복될 수 있어.** `engine.rs`는 매 turn 현재 사용자 요청과 최근 60개 메시지를 다시 보내. 최근 60개는 “원하는 모든 과거 대화를 의미적으로 보존하는 요약”이 아니야. Codex 원래 이력과 중복되고 긴 대화 비용·압축 결과에 영향을 줄 수 있어.
4. **전달과 요구사항 반영의 경계가 남아 있어.** 최신 사용자 메시지 우선은 구조와 프롬프트로 지시하며 실제 준수를 프로그램이 검증하지 않아. pending 수집과 도구 응답 전송 사이에 들어온 메시지는 다음 턴 큐에 남을 수 있어. 검토 중에는 모두 같은 턴에 반영된다고 단정하지 마.
5. **종료 가드는 메시지 전송에 집중돼 있어.** `submit`은 accepting을 검사하지만 룸/세션 생성·외부 목록/가져오기 command에는 같은 검사가 없어. 별도 단기 RPC는 worker가 소유하지 않아 종료 경합 검증이 필요해. 원본 가져오기는 단기 RPC close 오류를 무시하는 코드도 있어.
6. **오류 정리와 상태 표시의 경합이 있어.** 이전 세션 RPC close 실패는 새 입력의 execution_session 설정 전 발생할 수 있어 오류가 이전 세션에 기록될 위험이 있어. 글로벌 agents 상태라 다른 세션을 선택해도 실행 중인 세션의 상태가 표시돼. Arch 종료확인 실패 후 최종 상태가 적절히 unknown인지도 경로별 재검증이 필요해.
7. **도구 전달은 장애 후 exactly-once가 아니야.** 예약 뒤 전송 중 죽으면 실제 수신 여부를 알 수 없어. unknown으로 남기고 자동 재전송하지 않는 현재 한계를 유지해.
8. **Arch 도구 사용 차단은 완전한 샌드박스가 아니야.** plan 모드·금지 프롬프트가 있으며 CLI 도구 자체가 비활성화됐다고 보장하지 않아. 개발 기능 확대 전에 실제 권한 경계를 검토해야 해.
9. **로그 보존·크기 정책이 없어.** `audit`에 전체 프로토콜/내용/stderr가 쌓이고 snapshot도 전체 메시지를 전달해. 장시간 사용의 DB 크기·성능·민감 정보 로그 취급 정책이 필요해.
10. **수동 압축은 중단·재시도 경로가 없어.** 압축 job은 실행 중인 턴이 없을 때만 시작하고, 진행 중에는 전용 취소 버튼이 없어. 압축 턴 ID를 `Rpc.turn_id`로 추적하지 않으므로 앱 종료·강제 종료 시 진행 중 압축은 `unknown`으로 남고 자동 재개하지 않아. 완료 이벤트를 180초 안에 확인하지 못해도 `unknown`이야.

## 추가 검증이 필요한 조건

다음은 코드 경로가 있거나 설계 요구사항이지만, 확인한 result만으로 실제 앱 검증 완료라고 할 수 없어.

- 실제 Arch 제한 시간 초과와 무한 대기 방지, Codex 실패 응답 후 재개.
- 검토 도중 새 사용자 요구사항 주입 → 오래된 리뷰 표시 → 같은 세션 최신 요구 우선 답변의 통합 시나리오.
- Arch 실행 중 창 닫기, 프로세스 종료 확인 실패, 도구 응답 전송 도중 강제 종료와 재시작.
- 실제 인증 만료·권한 실패의 GUI 표시. 기존 오류 GUI 검사는 존재하지 않는 agy 경로를 사용했어.
- 기본 임계값에서 장문 대화 자동 압축 스트레스 검증. 낮은 프로세스 전용 임계값에서 실제 자동 압축 상태 관측은 상단 검증 기록 참조. 모델·추론 선택 및 세션별 복원/같은 thread 적용도 검증 기록 참조.
- 원본 세션에 Arch 도구가 없는 경우의 실제 안내, 외부 앱 동시 사용/연속 가져오기 경합.

## 다음 작업 제안 — 아직 구현하지 않음

1. **대화 안정성 보강:** 중단 버튼은 구현했어. 남은 통합 검증 공백을 메우고, 종료/큐/원본 동시 사용의 실패 재현과 상태 처리를 정리해.
2. **다음 기능 후보(미구현):** 압축 실패·unknown 복구 UX와 재시도 안내. 수동 압축 버튼, 사용량·압축 상태 표시, 모델 목록·추론 선택과 세션별 저장은 구현했어. 새로운 API 연결은 당시 설치 프로토콜로 다시 확인해.
3. **기능에 맞춘 UI 정리:** 기존 탐색기·중앙 대화·오른쪽 협업 구분을 유지하며 설정 위치, 실행 대상 세션·오류·중단 표시를 정돈해.
4. **개발 도구 확장:** 실제 작업 폴더, 권한·승인, 프로세스 관리가 정해진 뒤 파일 수정·터미널·diff·Git worktree를 단계적으로 추가해.
5. **범용 에이전트 관리:** [단계별 설계 계획](AGENT_MANAGEMENT_PLAN.md)에 현재 코드 근거, `AgentAdapter`/`AgentCapabilities`, 설치·인증·실행 가능·현재 실행의 분리, Agents 화면 흐름, Blocking Delegation·SQLite 호환성, Mock Agent 검증을 정리했어. 계획만 기록했고 구현·마이그레이션·실행 검증은 아직 하지 않았어.

Codex Desktop과의 기능 동등성이나 출시 일정은 확정하지 않았어. 현재 검증된 대화 MVP에서 확장하는 순서이며, 공개 프로토콜 지원과 추가 구현이 필요한 범위를 구분해야 해.

## 다음 개발자가 실행할 명령

`mvp-app/`에서 실행해.

```powershell
npm test
node scripts/verify_external_threads.mjs
npm run build:desktop
python scripts/verify_import_ui.py
python scripts/verify_models.py
python scripts/verify_cancel.py
python scripts/verify_stream.py
python scripts/verify_context.py
python scripts/verify_live.py
python scripts/verify_workspace.py
python scripts/verify_restore.py
```

이 명령들을 매번 전부 실행할 필요는 없어. 첫 두 개는 로컬 검사, 실제 CLI를 사용하는 GUI 검사는 사용량과 저장소 부작용을 고려해 변경 범위에 맞춰 골라. `verify_restore.py`는 기존 실제 대화 DB 복사본을 사용해 새 모델 턴 없이 복원을 검사해. GUI probe는 debug 빌드 + `ESH_GUI_PROBE`가 있을 때만 동작해.

과거 `README.md`, `VERIFICATION.md`, `PANEL_VERIFICATION.md`, `WORKSPACE_VERIFICATION.md`, `IMPORT_UI_FIX.md`에는 당시 빌드명과 검증 단계가 남아 있어. 현재 실행 대상은 Cargo/빌드 스크립트의 `esh-chat-mvp4`를 기준으로 하고, 과거 문서의 next/fixed/ui exe를 최신본으로 안내하지 마.
