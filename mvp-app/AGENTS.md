# 앱 작업 규칙

상위 [AGENTS.md](../AGENTS.md)를 적용해. 현재 구조와 진행 상태는 [ARCHITECTURE.md](../docs/ARCHITECTURE.md), [PROGRESS.md](../docs/PROGRESS.md)를 확인해.

## 실행 파일 관리

- 최신 사용자 결정: `esh-chat-mvp4.exe` 하나를 같은 이름으로 갱신해. 이후 mvp5/mvp6 또는 next/fixed/ui 실행 파일을 새로 만들지 마.
- Cargo 실행 대상과 빌드·검증 명령은 현재 `esh-chat-mvp4`를 유지해.
- 데스크톱 빌드는 `scripts/build.ps1`로 실행해. 빌드 전에 `target/debug` 바로 아래 이전 ESH `.d`, `.pdb`를 지우고 진행해.
- 실행 중인 파일을 덮어쓸 수 없으면 앱 종료가 필요하다고 설명해. 임의 종료나 다른 이름의 exe 생성으로 우회하지 마.
- 앱 식별자와 기존 데이터 경로는 유지해. 과거 검증 기록은 보존해.
