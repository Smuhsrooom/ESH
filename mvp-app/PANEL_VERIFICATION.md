# 3패널 대화 UI 변경과 검증

## 변경 내용

- 왼쪽: 현재 ESH 프로젝트·기본 대화룸과 저장된 실제 Codex 세션 탐색기. 세션 선택은 해당 threadId의 기록을 읽기 전용으로 필터하고, 대화룸으로 돌아오면 중앙에서 전송할 수 있어.
- 중앙: 사용자 요청과 Codex의 사용자 대상 답변만 표시하고 입력 대상은 Codex로 고정했어. 연결 실패는 대화 메시지와 섞지 않고 별도 오류 배너로 표시해.
- 오른쪽: `request_arch_review`의 실제 질문, Antigravity의 실제 리뷰, 같은 턴의 후속 검토 질문만 표시해. 입력창·전송 버튼·편집 가능 요소는 없어.
- 두 경계에 포인터 드래그와 방향키 조절을 구현했어. 최소 패널 너비를 지키고 너비를 localStorage에 저장해.

별도의 프로젝트·룸 생성이나 외부 Codex 세션 가져오기를 추가한 것은 아니야. 탐색기는 기존 앱이 가진 프로젝트·룸과 실제 실행 이력에 한정해.

## 데이터 호환성과 유지한 실행 흐름

기존 `messages` 테이블에 `channel`, `kind`, `room_id`, `thread_id`, `turn_id`, `collaboration_id`를 추가했어. 기존 메시지 ID·발화자·본문·sequence·생성 시각은 보존해.

- `main`: 사용자 요청, Codex의 사용자 대상 답변.
- `collaboration`: `review_request`, `review_reply`, `review_followup`.
- `activity`: Codex가 공개한 진행 안내.
- `legacy`: 이전 UI에서 사용자와 Arch가 직접 주고받은 기록.
- `system`: 오류 기록.

Codex 발언은 프로토콜의 phase로, 협업 요청과 답변은 실제 도구 호출·callId로 분류해. 문장 내용으로 받는 사람을 추측하지 않아. 공개 진행 안내와 이전 직접 대화는 왼쪽 접이식 기록에 보존해.

SQLite 트랜잭션과 `user_version=1`로 마이그레이션해. 예전 audit에 있는 실제 검토 요청은 원문·원래 시각으로 복구하고, 실제 리뷰 응답에 callId를 연결해. 새 sequence로 복구된 과거 요청을 올바른 위치에 표시하려고 오른쪽은 생성 시각·sequence로 정렬해. 반복 실행 시 복구 메시지를 중복 생성하지 않아. 답변이 없던 예전 사용자 메시지를 과거의 복구 요청에 잘못 연결하지 않는 경계 사례도 검사했어.

Codex App Server와 Antigravity CLI 실행 방식, 세션 수명, Blocking Delegation, 원래 RPC ID에 응답하는 방법, 중복 전달 방지와 타임아웃 정책은 유지했어. Codex의 최종 사용자 답변을 AI 간 후속 메시지로 복제하지 않아. 실제 추가 `request_arch_review` 호출만 후속 질문으로 저장해.

## 실제 CLI·GUI 검증

실행 기록: `evidence/panels-20261018-035628/`.

| 항목 | 결과 |
|---|---|
| 중앙 | 사용자 요청 + Codex 최종 답변 2개 |
| 오른쪽 | 첫 요청 + 실제 리뷰 + 실제 후속 요청 + 재리뷰 4개 |
| 도구 응답 | 각 callId마다 wire 전송 1회·완료 이벤트 1회 |
| Blocking Delegation | 두 리뷰 모두 같은 Codex threadId·turnId에서 재개 |
| 협업 패널 | 편집 요소 0개 |
| 크기 조절 | 실제 WebView에 포인터 이벤트를 전달해 양쪽 경계의 픽셀 너비 변화 확인 |
| 탐색기 | 실제 세션 선택 시 해당 스레드 기록만 표시·입력창 없음 |
| 재시작 | 패널에 표시된 메시지 6개와 에이전트 상태 복원 |
| 오류 | 없는 Arch 실행 경로가 오류 배너에 표시되고 성공 리뷰는 생성되지 않음 |
| 이전 DB | 원래 메시지 4개의 식별자·본문·순서·시각 보존, 실제 audit의 검토 기록 복구 |

- threadId: `01a14b39-001b-7360-b75c-df8320be0da8`.
- turnId: `01a14b39-010c-7693-973c-53a11eb8a733`.
- 첫 callId: `exec-bc38d647-354b-4601-9040-a425769761f9`.
- 후속 callId: `exec-99a519bd-d6ca-479e-8497-e2f4aaed037b`.

GUI의 요청 본문은 실제 Codex 도구 arguments.question과, 리뷰 본문은 실제 Arch structured_output.answer와, 중앙 답변은 실제 Codex agentMessage와 대조했어. Mock AI 응답은 사용하지 않았어.

원본 자료:

- `live-gui.json`: 각 DOM 메시지의 실제 패널·채널·kind·callId와 SQLite snapshot.
- `live-audit.json`, `live-data/chat.sqlite`: 실제 두 CLI의 이벤트와 도구 전달 기록.
- `restore-gui.json`: 정상 종료·재시작 후 같은 기록.
- `error-gui.json`: 실제 실행 경로 오류의 화면 표시.
- `migration-gui.json`: 기존 DB 복사본의 마이그레이션 후 화면.
- `result.json`: 전체 자동화 판정.

최종 빌드는 답변 없는 예전 메시지의 마이그레이션 경계 수정을 포함해. `evidence/panels-final-20261018-035938/`에서 실제 CLI 대화의 복사본과 예전 DB 복사본을 다시 열어 최종 GUI·마이그레이션을 검사했어. 이 재검증은 저장된 실제 대화를 사용했고 새 모델 호출은 하지 않았어. 최종 소스와 실행 파일 해시도 그 폴더의 result.json에 있어.

Rust 테스트 5개와 React 타입 검사·Vite 빌드·Tauri 실행 파일 빌드가 통과했어. 기존 `technical-spike` 전체 파일 및 확인 대상 Strata/OpenCode·CLI 설정·인증 파일의 전후 해시는 동일했어.

## 재현

```powershell
npm test
npm run build:desktop
$env:PYTHONUTF8='1'
python scripts/verify_live.py
```

실제 두 CLI 검증은 모델 사용량을 소비해. 최종 빌드의 저장 기록·마이그레이션만 확인하려면 다음을 사용해.

```powershell
python scripts/verify_restore.py --live-run panels-20261018-035628
```

기존 DB와 이전 검증 자료는 읽기 전용으로 열어 새 검증 폴더에 복사해 사용해. 패널 크기는 브라우저 저장소에 저장되지만 CLI 설정과 인증 정보는 편집하지 않아.
