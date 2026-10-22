# ESH 대화 MVP

최신 기능과 실측 검증은 [프로젝트·세션 생성 및 Codex 추가](WORKSPACE_VERIFICATION.md)에 정리했어. 이전 UI 검증은 [3패널 검증 기록](PANEL_VERIFICATION.md)에 있어.

사용자·Codex Lead·Antigravity Arch가 협업하는 Tauri 2 앱이야. 왼쪽 프로젝트·룸·실제 Codex 세션 탐색기, 중앙 사용자↔Codex 대화, 오른쪽 읽기 전용 AI↔AI 타임라인으로 나뉘어. 두 경계를 드래그하거나 방향키로 조절할 수 있고 너비는 로컬에 저장돼. 파일 수정·셸 실행·Git worktree·diff·QA 에이전트는 구현하지 않았어.

## 실행

```powershell
cd mvp-app
npm install
npm run desktop
```

빌드된 앱은 `src-tauri/target/debug/esh-chat-mvp4.exe`를 실행하면 돼. 기존 앱을 닫고 새 실행 파일을 열어줘. React 파일도 포함되어 있어 Vite 서버 없이 실행돼. 새로 빌드하려면 `npm run build:desktop`을 실행해.

Node, Windows C++ 빌드 도구, WebView2가 필요해. 현재 PC에서는 프로젝트 상위 `.tools/`의 Rust를 사용해. 다른 PC에서는 표준 Rust MSVC 설치를 사용할 수 있어. 이 스크립트는 자식 프로세스의 환경만 설정하고 시스템 PATH나 CLI 설정을 편집하지 않아.

단순 `npm run dev`는 브라우저 화면 개발용이야. Tauri IPC가 없으면 실제 CLI 연결이 불가능하다고 표시하고 성공을 흉내 내지 않아.

## 대화 방법

- 중앙 입력창의 대상은 Codex Lead야. 예: `Arch에게 이 설계를 검토받고 네 의견도 알려줘.`
- 오른쪽에는 실제 `request_arch_review` 질문과 Arch 응답, 추가 도구 호출로 보낸 후속 질문만 보여. 사용자 입력창이나 전송 버튼이 없어.
- 중앙에는 사용자 요청과 Codex의 사용자 대상 답변만 보여. Codex의 공개 진행 안내는 왼쪽의 접을 수 있는 기록에 보관하고, 오류는 별도 배너로 표시해.
- 탐색기에서 새 프로젝트·룸과 그 안의 세션을 만들 수 있어. 세션을 선택해 계속 대화하거나, Codex 추가 버튼으로 기존 기록 가져오기·원본 이어쓰기를 선택할 수 있어.
- 실행 중에도 메시지를 입력할 수 있어. 즉시 SQLite에 저장하고 전달 전에는 `전달 대기`로 보여줘.
- Arch 검토 중 도착한 Codex 대상 메시지는 리뷰 본문과 별도 필드로 전달해. 검토 버전이 다르면 stale로 표시하고 최신 사용자 요구사항을 우선하도록 지시해.
- 일반 Codex 실행 중 도착한 메시지는 현재 턴이 끝난 뒤 다음 턴에서 처리해. 실시간 steering은 이 MVP에 넣지 않았어.
- 전달 상태는 실제 모델 반영을 증명하지 않으므로 자동으로 `applied`를 표시하지 않아.

## 실행 중단

입력창의 ‘■ 중단’을 누르면 현재 선택한 세션의 실행을 멈춰. Arch가 검토 중이면 Arch 실행과 열린 검토 호출도 함께 정리해. 같은 세션에 이미 쌓여 있던 대기 메시지는 취소됨으로 남기고 다른 세션의 대기 작업은 유지해. 정리가 끝나면 새 메시지를 보내 이어갈 수 있어. 상태가 확인되지 않으면 중단 성공으로 처리하지 않고 상태 확인 필요로 표시해.

## 마크다운 표시

Codex·Antigravity 메시지 카드에 제목, 목록, 강조, 표, 인용, 코드 블록과 체크리스트를 표시해. 긴 코드와 표는 안쪽에서 가로 스크롤할 수 있어. 사용자가 입력한 글과 저장된 원문은 유지해. HTML은 실행하지 않고 이미지는 설명 텍스트로 표시해.

## 모델·추론 강도 선택

챗 입력창 하단의 모델과 추론 강도 목록에서 선택하면 돼. Codex App Server의 실제 모델 목록과 각 모델이 지원하는 강도만 보여줘. 모델을 바꾸면 해당 모델의 기본 강도가 선택돼. 선택값은 세션별 SQLite에 저장되고, 다른 세션을 선택하거나 앱을 다시 켜도 복구돼.

다음 새 턴부터 적용되며 이미 실행 중인 Codex 턴/Arch 검토를 다시 시작하지 않아. 이미 큐에 저장한 메시지는 전송 당시 설정을 사용해. 미설정 세션은 기존 모델과 낮음 강도의 호환 설정을 유지하므로 원하는 모델을 명시적으로 선택해줘. Antigravity 모델 선택 기능은 이번에 추가하지 않았어.

실제 검증은 `scripts/verify_models.py`, 결과는 `evidence/models-20261018-160229/result.json`에 있어. 모델 목록 조회에 실패하면 오류와 재조회 버튼을 표시해. 설정·인증 파일은 수정하지 않아.

## React → Rust → CLI → React

1. React가 `send_message`를 invoke해. Rust는 입력을 검사하고 SQLite에 사용자 메시지를 저장한 뒤 작업 큐에 넣어.
2. Rust worker가 독립 Codex App Server를 stdio로 시작해. 검증된 experimental Dynamic Tools 등록 형식으로 `request_arch_review`를 제공해.
3. Codex가 도구를 요청하면 `waiting_for_arch`를 저장하고 요청의 RPC ID·callId·threadId·turnId를 보존해. 기존 도구 호출은 계속 열린 상태야.
4. Rust가 실제 `agy`를 실행하고 구조화된 최종 결과의 reviewId·conversationVersion·answer, 프로세스 종료 코드와 `SUCCESS`를 검사해. 각 리뷰는 독립 대화를 사용해 이전 structured_output 잔류를 피하고 결과 식별자도 대조해.
5. Arch 답변을 SQLite에 저장하고 최신 사용자 요구사항과 리뷰를 분리한 envelope를 **기존 RPC 요청 ID에 한 번만** 반환해. 추가 `turn/start`로 재개를 흉내 내지 않아.
6. 같은 Codex 턴의 도구 완료와 후속 발언을 처리해. 새 메시지·상태를 SQLite에 먼저 기록하고 `chat-updated` 이벤트를 보내.
7. React는 revision이 이전 화면보다 오래되지 않은 snapshot만 적용하고 SQLite sequence 순서대로 실제 발언을 렌더링해. 초기 로딩도 SQLite snapshot에서 시작해.

메시지에 `channel`, `kind`, `roomId`, `threadId`, `turnId`, `collaborationId`를 추가했어. 중앙은 `main`, 오른쪽은 `collaboration` 중 검토 요청·후속 요청·리뷰 응답만 표시해. 협업 ID는 실제 callId를 사용하고 문장의 단어나 모델 추론으로 대상을 추측하지 않아. 후속 요청은 같은 턴의 두 번째 이후 `request_arch_review` 호출이야. Codex 사용자 답변을 AI 간 메시지로 복제하지 않아.

기존 테이블을 재생성하지 않는 SQLite 트랜잭션 마이그레이션을 사용해. 원래 메시지 ID·내용·sequence·생성 시각을 보존하고 기존 audit에서 확인되는 라우팅을 보완해. 예전 UI에 없던 검토 질문은 실제 audit의 원문과 원래 시각으로 복구해. 이 복구 메시지는 새 sequence를 받으므로 오른쪽은 생성 시각과 sequence 순서로 정렬해. 직접 Arch에게 보냈던 기록은 `legacy`, 진행 안내는 `activity`로 남기고 왼쪽 기록에서 읽을 수 있어.

서버 요청 ID `0`도 정상 ID야. callId와 RPC 응답 ID는 별개로 관리해. 정상 연결에서는 SQLite 유일 키로 중복 전송을 차단하고 `prepared → sent → confirmed`로 기록해. 전송 이후 프로세스가 비정상 종료되면 exactly-once 복구를 가정하지 않고 `unknown`으로 남겨.

## 주요 파일

- `src/main.tsx`: 최소 대화 GUI, 초기 기록 복원, Tauri 이벤트 수신과 메시지 전송.
- `src/panels.ts`: 두 패널 경계의 포인터 드래그·키보드 조작·최소 너비 제한·너비 저장.
- `src-tauri/src/main.rs`: Tauri 명령 등록, 앱 상태 초기화, 창 종료 처리.
- `src-tauri/src/engine.rs`: 단일 작업 큐, Blocking Delegation, 최신 메시지 우선 전달, 상태 전이.
- `src-tauri/src/cli.rs`: spike에서 확인한 프로토콜을 Rust로 이식한 Codex·Antigravity 어댑터, 타임아웃·종료 관리.
- `src-tauri/src/store.rs`: 메시지 순서·시각·상태, 에이전트 상태, 도구 전달 상태, 실행 증거를 SQLite에 저장.
- `src/guiProbe.ts`, `scripts/verify_live.py`: debug opt-in 검증에서 실제 WebView 입력·렌더링과 CLI audit를 대조해. AI 응답을 생성하거나 Mock으로 바꾸지 않아.

기존 `../technical-spike/`는 읽기 전용으로 참고했어. Python 검증기를 실행하거나 원본 코드를 수정하지 않았고, 새 앱에 필요한 연결 형식과 계약만 이식했어. 원본 파일 해시는 `evidence/technical-spike-before.json`과 최종 검증 결과로 비교해.

## 저장과 오류 처리

기본 DB 경로는 `%LOCALAPPDATA%\local.esh.conversation\chat.sqlite`야. 메시지에는 sequence, 발화자, 내용, 생성 시간, 버전, 전달 상태와 채널·스레드·검토 ID를 저장해. 에이전트별 현재 상태와 갱신 시각도 저장해. 기존 세션 생성·재개 방식과 CLI 프로세스 수명은 바꾸지 않았어.

앱 재시작 시 메시지는 그대로 복원해. 진행 중이던 실행은 `unknown`, 전송되지 않은 메시지는 `interrupted`로 표시하고 자동 재실행하지 않아. 기존 CLI 로그인은 사용하지만 로그인·설정 변경 기능은 없어. CLI 미설치·인증·구조화 결과 오류는 시스템 오류 발언과 실패 상태로 표시해.

Arch 타임아웃은 기본 120초, 정수 30~600초이며 자동 재시도하지 않아. Codex의 네트워크 재접속 정책은 Codex 자체 동작이야. Arch 실패도 같은 Codex 도구 호출에 `success:false`로 돌려줘. 오류를 정상 리뷰로 바꾸지 않아.

종료 시 신규 작업을 막고 Arch 종료를 요청·확인한 뒤 열린 Codex 도구 호출을 취소 결과로 정리하고, Codex 종료 후 SQLite 상태를 확정해. 확인할 수 없는 종료는 `unknown`으로 기록해. 닫기 후 정리를 위해 잠시 기다릴 수 있어.

설정이 필요한 경우 앱 실행 프로세스에만 환경 변수를 줄 수 있어. GUI 설정 화면은 추가하지 않았어.

| 환경 변수 | 용도 |
|---|---|
| `ESH_CODEX_BIN` | Codex 실행 파일 경로 override |
| `ESH_AGY_BIN` | Antigravity 실행 파일 경로 override |
| `ESH_ARCH_TIMEOUT_SECONDS` | 30~600초, 기본 120 |
| `ESH_DATA_DIR` | 별도 DB 저장 폴더 |

Codex 기본 모델은 `gpt-6.1-sol`, Arch는 `gemini-3.8-flash-high`야. 전역 설정과 Strata/OpenCode 인증 정보를 편집하지 않아. 대화 전용으로 다른 Codex 도구 요청은 거절해. Antigravity `plan` 모드를 엄격한 OS 보안 경계로 주장하지 않으며, 별도 파일 작업 권한 UI는 이번 범위에 포함하지 않아.

## 검증

```powershell
npm test
npm run build:desktop
$env:PYTHONUTF8='1'
python scripts/verify_live.py
```

실제 CLI 검증은 기존 로그인을 이용하고 실제 사용량을 소비해. 검증 중에는 실제 Tauri 창이 열려. `ESH_GUI_PROBE`는 debug 빌드에서만 활성화되며 일반 실행에서는 동작하지 않아.

검증기는 독립 DB에서 실제 GUI 입력 → 첫 리뷰 → 실제 후속 리뷰 → 같은 Codex 턴의 최종 답변을 확인해. 요청·응답이 오른쪽에만, 사용자 요청·최종 답변이 중앙에만 나오는지 원본 CLI 이벤트와 대조해. 오른쪽 편집 요소 부재, 두 경계의 포인터 이벤트와 너비 변화, 세션 기록 필터, 앱 재시작 복원도 검사해. 이전 DB의 복사본을 열어 원본 메시지와 audit 기반 분류가 보존되는지도 확인해. 별도 실행 파일 경로 오류도 화면에서 확인해. 실제 인증 실패를 만들기 위해 기존 로그인을 해제하지 않아.

초기 검증의 DOM 증거·SQLite·CLI 원본 이벤트와 판정은 `evidence/gui-날짜-시각/`, 3패널 변경 검증은 `evidence/panels-날짜-시각/`에 보관해. 이전 로그는 수정하지 않아.

공식 기준: [Codex App Server](https://learn.chatgpt.com/docs/app-server), [Antigravity Headless](https://antigravity.google/docs/cli/headless/), [Tauri 명령·IPC](https://v2.tauri.app/develop/calling-rust/).

Codex 답변은 실제 App Server delta를 받아 실시간으로 표시해. 작성 중인 마크다운도 갱신하고 중단 시 받은 부분을 보존해. Arch는 검증된 완료 리뷰만 표시해. 실제 CLI·WebView 스트리밍/중단 후 재개/재시작 복원 검증은 `evidence/stream-20261018-165615/result.json`을 참고해.

입력창 위에 Codex 컨텍스트 사용량(최근 보고 기준 추정 비율/토큰)과 압축 상태를 표시해. 자세히 펼치면 최근 입력·출력·캐시·누적 토큰과 보고/압축 시각을 볼 수 있어. 세션별 SQLite 저장과 재시작 복원을 지원하며 압축 진행/완료는 실제 contextCompaction 이벤트로만 표시해. 수동 압축 버튼과 임계값 변경 UI는 없어.
