# MVP 최종 검증 기록

2026-10-18 한국 시각에 실제 Tauri WebView와 설치된 두 CLI로 검증했어. Mock AI 응답은 사용하지 않았어.

- 최종 판정: **통과**.
- 원본 로그와 DB: `evidence/gui-20261018-032922/`.
- GUI 표시: 사용자 → Codex → Antigravity → Codex 발언 4개.
- 같은 Codex threadId: `01a14b20-31c4-75a1-be28-eb47bf8663e9`.
- 같은 Codex turnId: `01a14b20-3348-7d30-b4e3-e0740cbe1ce8`.
- callId: `exec-08c0c102-c92e-4a45-9054-b4456fb39838`.
- RPC 요청 ID: `0`.
- 도구 응답 wire 전송 **1회**, 도구 완료 이벤트 **1회**, SQLite 전달 상태 `confirmed`.
- Arch의 실제 structured_output.answer와 GUI Antigravity 발언이 동일해.
- 리뷰 후 Codex의 실제 CLI 발언이 GUI에 표시됐고 같은 턴이 완료됐어.
- 앱을 정상 종료·재시작한 뒤 메시지 4개의 ID·내용·순서와 에이전트 상태가 그대로 복원됐어.
- 존재하지 않는 CLI 실행 경로를 지정한 별도 GUI 검증에서 실패 상태·시스템 오류가 표시됐어. 이 실행은 연결 성공으로 집계하지 않았어.
- technical-spike 전체 파일 해시 동일. 확인 대상 Strata/OpenCode 및 CLI 설정·인증 파일 해시 동일.

`live-gui.json`과 `restore-gui.json`에는 실제 WebView DOM의 메시지·상태 및 Rust snapshot을 저장했어. `live-audit.json`과 `live-data/chat.sqlite`에는 CLI 원본 이벤트·요청·응답을 저장했어. DOM 내용, SQLite snapshot, 실제 CLI 출력이 일치하는지 검증기가 대조했어.

첫 실행의 Gemini 정수 enum 스키마 거부는 실패로 남겼고, `conversationVersion`을 정수 필드로 받고 Rust에서 검증하는 방식으로 수정했어. 첫 실패 기록은 `evidence/gui-20261018-032444/`, 첫 통과 기록은 `evidence/gui-20261018-032635/`에 보존했어. 최종 통과 기록은 수정된 결과 검증 함수와 상태 복원 검사까지 포함해.

Rust 테스트 3개가 통과했어: SQLite 메시지 복구·미완료 상태 unknown 처리·중복 전달 예약 차단, 타임아웃 범위, 이전 리뷰 ID/버전·빈 답변 거부. React 타입 검사·Vite 빌드·Rust 실행 파일 빌드도 통과했어.

이 검증은 살아 있는 동일 연결의 단일 응답과 정상 종료 후 기록 복원이야. 장애 이후 end-to-end exactly-once 또는 열린 RPC 호출 자동 복원을 주장하지 않아. 실제 인증 실패를 만들기 위해 로그인을 해제하지 않았어. 실행 중 사용자 개입, 120초 타임아웃, 진행 중 취소는 이번 GUI 완료 검증 항목에 포함하지 않았어.

검증한 소스와 실행 파일의 SHA-256은 `evidence/final-build-manifest.json`에 있어.
