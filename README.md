# ESH

ESH는 Windows에서 AI CLI와 대화하는 Tauri 2·React 기반 MVP입니다. 프로젝트와 세션을 한 화면에서 관리하고, Codex Lead의 대화 중 Antigravity Arch에게 검토를 요청할 수 있습니다. OpenCode CLI도 별도의 Lead 세션으로 사용할 수 있습니다.

## 현재 지원

- **Codex Lead:** 대화 이어쓰기, 응답 스트리밍, 실행 중단, 세션별 모델·추론 강도 선택, 컨텍스트 사용량 표시와 수동 압축을 지원합니다.
- **Antigravity Arch:** Codex가 `request_arch_review`를 호출하면 검토 결과를 같은 Codex 턴에 반환합니다. 검토 요청과 답변은 읽기 전용 협업 패널에 표시합니다.
- **OpenCode Lead:** 등록한 npm OpenCode CLI로 새 세션을 만들고 대화를 이어갈 수 있습니다. 세션별 모델을 선택할 수 있으며, 선택하지 않으면 OpenCode 기본 모델을 사용합니다.
- 프로젝트·룸·세션 생성과 선택, 기존 Codex 기록 추가, SQLite 대화 복원, 에이전트 등록 상태 확인을 지원합니다.

## 설치와 실행

Windows, Node.js/npm, Rust MSVC 도구 모음, C++ 빌드 도구, WebView2가 필요합니다. 사용할 Codex·Antigravity·OpenCode CLI는 별도로 설치하고 로그인해야 합니다. CLI별 사용 가능 여부는 계정과 설치 상태에 따라 달라집니다.

```powershell
cd mvp-app
npm install
npm run desktop
```

데스크톱 실행 파일은 `npm run build:desktop`으로 빌드하며 결과 경로는 `mvp-app/src-tauri/target/debug/esh-chat-mvp4.exe`입니다. `npm run build`는 프런트엔드만 빌드합니다. 자세한 실행·사용 안내는 [앱 README](mvp-app/README.md)를 참고하세요.

## 사용 흐름

1. 앱에서 프로젝트·룸을 선택하거나 만들고 새 세션을 추가합니다.
2. 새 세션의 대화 에이전트로 Codex Lead 또는 등록된 OpenCode Lead를 선택합니다. 기존 Codex 기록은 `Codex 추가`로 가져올 수 있습니다.
3. 입력창 아래에서 해당 세션의 모델을 선택하고 메시지를 보냅니다. Codex 세션에서는 추론 강도도 선택할 수 있습니다.
4. Codex가 Arch 검토를 요청하면 오른쪽 협업 패널에서 질문과 답변을 확인합니다.

## 현재 제한

대화 중심 MVP이므로 파일 편집·셸 실행·diff·Git worktree 관리·첨부 파일 기능은 제공하지 않습니다. Antigravity는 독립 대화 세션이 아닌 Codex 검토 경로로 연결됩니다. OpenCode 세션은 Arch 위임과 Codex 전용 추론 강도·컨텍스트 기능을 제공하지 않습니다. Claude Code와 Gemini CLI는 등록 후보이지만 대화 실행은 지원하지 않습니다. 작업은 앱 전체에서 한 번에 하나씩 처리하며, 실제 CLI 로그인과 모델 제공자 연결이 필요합니다.

구조와 구현 범위는 [아키텍처](docs/ARCHITECTURE.md), 최신 변경·검증 근거와 알려진 문제는 [진행 상태](docs/PROGRESS.md)를 참고하세요.
