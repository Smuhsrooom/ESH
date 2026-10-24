# ESH 대화 MVP

Windows용 Tauri 2·React 앱입니다. 왼쪽에서 프로젝트·룸·세션과 에이전트를 관리하고, 중앙에서 Codex 또는 OpenCode Lead와 대화합니다. Codex가 요청한 Antigravity Arch 검토는 오른쪽 읽기 전용 패널에 표시합니다.

## 준비와 실행

Windows의 WebView2, Node.js/npm, Rust MSVC 도구 모음과 C++ 빌드 도구가 필요합니다. 이 저장소 상위 `.tools/`에 Rust가 있으면 스크립트가 그 도구를 사용하고, 없으면 설치된 표준 Rust를 사용합니다. 사용하려는 CLI는 따로 설치하고 해당 CLI에서 로그인해야 합니다.

```powershell
cd mvp-app
npm install
npm run desktop
```

`npm run desktop`은 Tauri 개발 창을 엽니다. 실행 파일을 만들려면 `npm run build:desktop`을 실행하고 `src-tauri/target/debug/esh-chat-mvp4.exe`를 엽니다. 앱이 실행 중이라 파일이 잠겨 있으면 앱을 정상 종료한 뒤 빌드해야 합니다. `npm run build`는 TypeScript·Vite 프런트엔드 빌드이고, `npm run dev`의 브라우저 화면에는 Tauri IPC가 없어 CLI 대화는 실행되지 않습니다. 설치 패키지와 자동 업데이트는 아직 제공하지 않습니다.

## 대화하기

1. 왼쪽에서 프로젝트·룸을 만들거나 선택하고 `새 세션`을 누릅니다. 기본 대화 에이전트는 Codex Lead입니다. OpenCode를 사용하려면 에이전트 화면에서 **npm OpenCode CLI 실행 파일**을 Lead로 등록한 뒤 새 세션에서 그 에이전트를 선택합니다.
2. 기존 Codex 기록은 `Codex 추가`에서 찾아 새 로컬 세션으로 복사하거나 원본 이어쓰기를 선택할 수 있습니다. 원본 이어쓰기는 실제 원본 세션에 영향을 줄 수 있으므로 선택을 확인하세요.
3. 입력창 아래에서 모델을 선택하고 메시지를 보냅니다. Codex는 실제 App Server 모델 목록과 지원 추론 강도를 보여 주고, OpenCode는 등록된 CLI의 모델 목록을 보여 줍니다. 선택값은 세션별 SQLite에 저장됩니다. OpenCode에서 `기본 모델`을 선택하면 실행 시 `--model`을 생략합니다.
4. Codex에게 Arch 검토를 요청하면 `request_arch_review` 호출의 질문·답변을 오른쪽에서 볼 수 있습니다. 이 패널에는 직접 입력할 수 없습니다. OpenCode 세션에는 Arch 검토 연결이 없습니다.

실행 중단 버튼은 현재 실행을 멈추고, CLI 오류와 모델 목록 조회 실패는 화면에 표시합니다. 입력한 메시지는 SQLite에 저장되며 앱을 다시 열어 기록을 복원합니다. 전달 상태가 모델의 의미적 반영을 증명하지는 않습니다. Codex 응답은 스트리밍하고, OpenCode는 완료된 CLI 답변을 받은 뒤 표시합니다. 모델 변경은 다음 턴부터 적용됩니다.

## 저장·설정

기본 대화 DB는 `%LOCALAPPDATA%\local.esh.conversation\chat.sqlite`에 있습니다. 기존 CLI 로그인·설정 파일은 앱에서 편집하지 않습니다. 필요할 때 앱 프로세스 환경 변수로 `ESH_CODEX_BIN`(Codex 경로), `ESH_AGY_BIN`(Antigravity 경로), `ESH_ARCH_TIMEOUT_SECONDS`(30~600초, 기본 120초), `ESH_DATA_DIR`(별도 데이터 폴더)를 지정할 수 있습니다. OpenCode 실행 파일은 에이전트 등록 화면에서 지정합니다.

Codex 세션은 모델·추론 강도, 컨텍스트 사용량과 수동 압축을 지원합니다. Antigravity 검토 모델은 고정이며 별도 선택 UI가 없습니다. OpenCode는 세션별 모델만 선택할 수 있고, 선택 모델이 CLI나 제공자에서 거부되면 실행 오류로 표시합니다.

## 제한과 검증

현재 앱은 대화 MVP입니다. 파일 수정·셸 실행·Git worktree·diff·첨부 파일·QA 에이전트는 제공하지 않습니다. Claude Code와 Gemini CLI는 등록 후보이지만 대화 실행은 지원하지 않습니다. 전체 세션은 단일 worker에서 순서대로 처리하고, 중단·종료 뒤 확인할 수 없는 전달은 자동 성공이나 자동 재실행으로 취급하지 않습니다. OpenCode에는 도구 거부 설정을 전달하지만, CLI 자체는 현재 사용자 권한으로 실행되므로 이 설정을 운영체제 수준의 파일 접근 차단으로 간주하면 안 됩니다.

코드 변경 시 필요한 범위에서 아래 검사를 실행할 수 있습니다.

```powershell
npm test
npm run build
node scripts/verify_external_threads.mjs
```

`npm test`는 Rust 단위 테스트이며 실제 CLI 연결 성공을 뜻하지 않습니다. 실제 CLI·WebView 검증 스크립트는 로그인과 모델 사용량이 필요하고 원본 Codex 저장소에 세션을 남길 수 있습니다. 실행 결과와 아직 확인하지 못한 조건은 [진행 상태](../docs/PROGRESS.md), 구성과 데이터 흐름은 [아키텍처](../docs/ARCHITECTURE.md)에 기록돼 있습니다.

## 개발·동작 상세

### 화면과 메시지

왼쪽 탐색기, 중앙 대화, 오른쪽 읽기 전용 협업 기록의 두 경계는 드래그하거나 방향키로 조절할 수 있으며 너비는 로컬에 저장됩니다. 중앙에는 사용자 요청과 선택한 Lead의 사용자 대상 답변을 표시합니다. Codex의 공개 진행 안내는 왼쪽의 접을 수 있는 기록에, 오류는 별도 배너에 표시합니다. 오른쪽에는 실제 `request_arch_review` 질문·후속 질문·Arch 응답만 표시하며 입력창이나 전송 버튼은 없습니다.

실행 중 입력한 메시지도 SQLite에 즉시 저장하고 전달 전에는 `전달 대기`로 표시합니다. 일반 Codex 실행 중 도착한 메시지는 현재 턴이 끝난 뒤 다음 턴에서 처리하며 실시간 steering은 제공하지 않습니다. Arch 검토 중 도착한 최신 사용자 요구사항은 리뷰 본문과 분리해 전달하고, 검토 버전이 다르면 오래된 검토임을 표시합니다. `delivered`는 전송 완료 상태이며 모델의 의미적 반영을 확인하는 `applied` 상태로 자동 변경하지 않습니다.

Codex·Antigravity·OpenCode 메시지는 Markdown으로 표시하며 제목·목록·강조·표·인용·코드 블록·체크리스트를 지원합니다. 긴 코드와 표는 내부에서 가로 스크롤합니다. 저장된 원문은 유지하고 HTML을 실행하지 않으며, 이미지는 설명 텍스트로 표시합니다.

### Codex와 Arch의 Blocking Delegation

1. React가 `send_message`를 호출하면 Rust는 입력을 검사하고 사용자 메시지를 SQLite에 저장한 뒤 단일 worker의 큐에 넣습니다.
2. worker는 Codex App Server와 stdio JSON-RPC로 연결하고 experimental Dynamic Tools 형식으로 `request_arch_review`를 제공합니다.
3. Codex가 검토를 요청하면 `waiting_for_arch`를 저장하고 RPC 요청 ID·callId·threadId·turnId를 보존합니다. **원래 도구 호출은 열린 상태로 유지합니다.**
4. Rust가 `agy`를 실행한 뒤 구조화된 최종 결과의 reviewId·conversationVersion·answer, 종료 코드와 `SUCCESS`를 검사합니다. 각 리뷰는 독립 대화를 사용하고 결과 식별자를 대조합니다.
5. Arch 답변을 저장하고 `archReview`와 `latestUserRequirements`를 분리한 응답을 기존 RPC 요청 ID에 한 번 전달합니다. 추가 `turn/start`로 재개를 흉내 내지 않습니다. Arch 실패도 같은 호출에 `success:false`로 반환합니다.
6. 같은 Codex 턴의 도구 완료와 후속 발언을 처리합니다. 새 메시지·상태를 SQLite에 기록한 뒤 `chat-updated` 이벤트를 보냅니다. React는 오래된 revision의 snapshot을 버리고 SQLite sequence 순서대로 표시합니다.

메시지의 `channel`, `kind`, `roomId`, `threadId`, `turnId`, `collaborationId`로 중앙 `main`과 오른쪽 `collaboration`을 구분합니다. 협업 ID는 실제 callId를 사용하며 발언 내용에서 대상을 추측하지 않습니다. 서버 요청 ID `0`도 유효하고 callId와 RPC 응답 ID는 별개입니다. `tool_deliveries`의 고유 키와 `prepared → sent → confirmed` 상태가 정상 연결에서 중복 전송을 막지만, 비정상 종료까지 포함한 exactly-once 복구를 보장하지는 않습니다.

### OpenCode와 모델 설정

OpenCode는 등록한 npm CLI의 `models --pure` 출력을 읽어 모델을 표시합니다. `provider/model`뿐 아니라 `nvidia/deepseek-ai/deepseek-r1`처럼 슬래시가 여러 개인 ID도 허용합니다. 선택 시 현재 목록과 대조해 세션별로 저장하며, 실행 시 다시 목록을 조회하지 않고 선택한 ID를 `run --model`에 전달합니다. 미선택이면 `--model`을 생략합니다. OpenCode CLI는 완료된 JSON 답변과 세션 ID를 확인해 저장하고 다음 턴에 이어 쓰며, 도구 호출은 거부합니다.

Codex의 모델·추론 강도는 App Server가 제공하는 목록과 모델별 지원값을 사용합니다. 모델을 바꾸면 해당 모델의 기본 강도가 선택되고 설정은 세션별 SQLite에 저장·복원됩니다. 변경은 다음 턴부터 적용되며 이미 실행 중인 턴·검토를 다시 시작하지 않습니다. 미설정 세션은 기존 모델과 낮음 강도의 호환 설정을 유지합니다. 실제 Codex 모델 검증 기록은 `scripts/verify_models.py`와 `evidence/models-20261018-160229/result.json`에 있습니다.

### 중단·저장·복구

중단 버튼은 현재 선택한 세션의 실행을 멈춥니다. Arch 검토 중이면 Arch 프로세스와 열린 검토 호출도 정리합니다. 같은 세션의 대기 메시지는 취소됨으로 남기고 다른 세션의 대기 작업은 유지합니다. 정리 결과를 확인하지 못하면 성공으로 표시하지 않고 `unknown`으로 남깁니다.

기존 데이터는 테이블을 재생성하지 않는 SQLite 마이그레이션으로 보존합니다. 원래 메시지 ID·내용·sequence·생성 시각을 유지하고, 이전 audit에서 확인되는 라우팅을 보완합니다. 예전 UI에 없던 검토 질문은 원래 audit 원문과 시각을 바탕으로 복구합니다. 직접 Arch에게 보낸 기록은 `legacy`, 공개 진행 안내는 `activity` 채널에 둡니다. OpenCode Lead 연결을 위한 현재 스키마는 v6이며 기존 Codex 세션을 Codex로 보존합니다.

재시작 시 메시지를 복원하되 진행 중이던 실행과 `prepared`·`sent` 도구 전달은 `unknown`, 전달되지 않은 사용자 메시지는 `interrupted`로 표시하고 자동 재실행하지 않습니다. 종료 시 신규 작업을 막고 Arch 종료 확인, 열린 Codex 도구 호출 정리, Codex 종료, SQLite 상태 확정 순서로 처리합니다. Arch 제한 시간은 기본 120초, 설정 범위 30~600초이며 ESH의 자동 재시도는 없습니다.

### 주요 파일과 실제 검증

- `src/main.tsx`, `src/panels.ts`: 대화·세션 UI, snapshot 수신, 패널 크기 조절을 담당합니다.
- `src-tauri/src/main.rs`, `src-tauri/src/engine.rs`: Tauri 명령, 단일 worker, 세션 전환, Blocking Delegation, 중단·종료를 담당합니다.
- `src-tauri/src/cli.rs`, `src-tauri/src/opencode.rs`: Codex·Antigravity 프로세스 연결과 OpenCode 모델 목록·턴 처리를 담당합니다.
- `src-tauri/src/store.rs`, `src-tauri/src/workspace.rs`: SQLite 메시지·전달 상태·세션과 마이그레이션을 담당합니다.
- `src/guiProbe.ts`, `scripts/verify_live.py`: 디버그 검사에서 실제 WebView 입력·렌더링과 CLI audit를 대조합니다. Mock 응답을 실제 연결 성공으로 취급하지 않습니다.

실제 CLI 검증은 `npm run build:desktop`으로 실행 파일을 만든 뒤 `scripts/verify_live.py`, `verify_workspace.py`, `verify_restore.py`, `verify_import_ui.py` 등 수정 범위에 맞는 스크립트로 수행합니다. 로그인과 실제 모델 사용량이 필요하고 원본 Codex 저장소에 검증용 세션이 남을 수 있으므로, 격리 DB에 `ESH_DATA_DIR`을 지정해도 그 부작용은 별도로 고려해야 합니다. `ESH_GUI_PROBE`는 디버그 빌드에서만 활성화됩니다. 초기 GUI·협업 결과는 `evidence/gui-날짜-시각/`, `evidence/panels-날짜-시각/`, 스트리밍·중단·복원 결과는 `evidence/stream-20261018-165615/result.json`에 있습니다. 최신 OpenCode 검증 범위는 [진행 상태](../docs/PROGRESS.md)를 참고하세요.

`../technical-spike/`는 읽기 전용 초기 검증 자료입니다. 현재 구현은 `src/`와 `src-tauri/src/`에 유지합니다.
