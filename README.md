# Planning Workbench · Native

사람이 직접 설계하고, 실제 저장소 구조를 보며 구현 계획을 작성하는 Windows / macOS 앱입니다.
기존 화면의 어두운 색상, 레이어별 카드와 계획 캔버스를 Rust + egui/eframe으로 구현합니다. 네이티브 클라이언트는 WebView·Node·React 런타임을 사용하지 않습니다.

현재 범위는 **저장소 구조 탐색 → 수동 계획 편집 → JSON / Markdown 내보내기**입니다. AI 작업 실행은 연결하지 않았습니다.

## 실행

Rust 1.95 이상이 필요합니다. Windows에는 MSVC C++ Build Tools, macOS에는 Xcode Command Line Tools가 필요합니다.

개발 빌드에서 먼저 앱에 내장할 연결선 라우터를 준비합니다. Windows는 고정된 SHA256으로 검증한 upstream 실행 파일을 받고, macOS는 같은 소스에서 서명된 universal 실행 파일을 빌드합니다. `crates/factory-layout/build.rs`가 계산기·라이선스·원본 소스를 앱 실행 파일에 포함합니다.

```powershell
# Windows, 저장소 루트에서 한 번 실행
./scripts/native/prepare-router.ps1
```

```sh
# macOS
brew install automake libtool pkg-config
bash scripts/native/prepare-router.sh
```

```sh
# 프로젝트 루트에서 실행
cargo run -p factory-workbench --release

# 실행하면서 분석할 저장소 지정 (공백이 있는 경로는 따옴표로 감싸세요)
cargo run -p factory-workbench --release -- /path/to/your/repository

# 독립 실행 파일 빌드
cargo build -p factory-workbench --release --locked
```

Windows 실행 파일은 `target/release/factory-workbench.exe`, macOS 실행 파일은 `target/release/factory-workbench`입니다. 앱을 실행한 뒤 **폴더 열기**로 로컬 저장소를 선택할 수도 있습니다. 로컬 분석에는 서버나 AI 계정이 필요하지 않습니다.

Windows 배포는 **`factory-workbench.exe` 하나**를 받아 실행합니다. macOS는 **`.app` 하나**를 사용합니다. 연결선 계산기는 앱에 내장되어 있고, 첫 계산 시 사용자 캐시에 자동으로 준비해 재사용합니다. 실행 파일 옆에 보조 파일을 두거나 인터넷에서 추가 다운로드할 필요가 없습니다. 사용자 캐시는 Windows `%LOCALAPPDATA%/SoftwareFactoryNative/router/`, macOS `~/Library/Caches/SoftwareFactoryNative/router/` 아래입니다.

라이선스·저작권 고지와 대응하는 원본 소스도 실행 파일에 내장되어 있습니다. `factory-workbench --export-router-sources <폴더>`로 내보낼 수 있으며, 개발용 `--check-router`는 창을 열지 않고 내장 계산기를 검증합니다. 독립적으로 수정한 계산기는 `FACTORY_LIBAVOID_SERVER`로 지정할 수 있습니다. [라우터 고지](docs/ROUTER-NOTICES.txt).

Windows에서 빌드·실행을 확인합니다. macOS는 동일 소스와 CI 빌드 경로를 제공하며, 이 Windows 작업 환경에서는 macOS 실행을 검증하지 않습니다. 릴리스 태그 빌드는 Windows 단일 `.exe`와 macOS universal `.app` ZIP을 생성합니다. 배포용 코드 서명·공증은 별도입니다.

## 저장소 구조 보기

1. FSD 저장소를 엽니다. 모노레포 안에서도 소스 루트를 찾습니다. 직접 지정하려면 `FACTORY_SOURCE_ROOT`를 저장소 기준 상대 경로로 설정합니다.
2. **의존 관계 그래프 안에서 Layer → Slice → 내부 폴더 / index 그룹 → 하위 그룹·파일** 순서로 노드를 펼칩니다. 펼친 부모 박스 안에 자식 노드가 나타나며 다른 레이어·형제 노드와 연결선도 같은 캔버스에 유지됩니다. `app`과 `shared`는 FSD 특성상 slice 없이 내부 그룹을 펼칩니다.
3. **카드 클릭은 연결 선택**, 왼쪽 **▸ / ▾ 버튼은 펼치기 / 접기**입니다. 레이어 제목 클릭은 펼치기 / 접기만 수행합니다. 접힌 그룹의 외부 import는 그룹 경계에 모이고, 펼치면 실제 내부 노드로 연결됩니다. **Layer 보기 / Slice 보기**, 확대 도구, 스크롤과 **마우스 드래그로 가로·세로 이동**하며 탐색합니다. 그래프 위에서 **Ctrl+휠**로 커서 위치를 기준으로 확대·축소합니다(macOS는 Cmd+휠도 지원). 일반 휠은 스크롤에 사용하며, 오른쪽 상세 패널에서는 그래프 배율을 바꾸지 않습니다. 검색 결과나 의존 대상 파일을 선택하면 필요한 상위 그룹을 펼쳐 그래프에서 해당 위치를 보여줍니다.
4. `index` 배지는 `index.ts`, `index.tsx`, `index.js` 등의 공개 진입점이 있는 디렉터리를 뜻합니다. 일반 폴더도 실제 계층을 유지하며 표시합니다. 오른쪽 **공개 진입점** 링크를 누르면 해당 index 파일로 이동합니다. 파일 카드는 실제 import·재노출 연결을 보여주므로 index에서 내보내는 파일과 index를 사용하는 곳도 탐색할 수 있습니다.
5. 연결선에 마우스를 올리면 import 이름과 파일 관계 수를 확인합니다. 오른쪽에서는 선택한 그룹의 파일 목록과 경계를 넘는 실제 import 관계를 봅니다. **이 그룹을 계획에 추가**로 세그먼트·그룹·파일을 계획에 복사합니다.

저장소 그래프는 FSD 레이어가 위에서 아래로 쌓이는 **층별 배치**입니다. 각 레이어의 카드는 **줄바꿈 없이 한 줄**로 놓으며, 중첩 그룹 내부도 같은 수평 배치를 사용합니다. 카드를 선택하면 선택 카드와 직접 연결된 카드를 앞에 모으고, 무관한 카드는 **오른쪽으로 이동해 흐리게 표시**합니다. 상위 그룹과 펼침 상태는 유지합니다. 선택이 없으면 연결선을 숨기고, 선택 시 그 카드가 사용하는 곳과 사용되는 곳의 연결만 표시합니다. 펼친 그룹을 선택하면 그룹 경계를 넘는 연결만 표시하며, 내부 관계는 해당 하위 카드를 선택해 확인합니다. 빈 공간이나 **선택 해제**를 누르면 원래 순서와 밝기로 돌아갑니다. 레이어 자체는 연결 선택 대상이 아닙니다.

선택한 연결만 백그라운드에서 **ELK의 고정 노드용 Libavoid 라우터**로 계산합니다. 카드의 한 줄 배치와 중첩 구조는 앱이 정하고, 라우터는 카드 본문과 그룹 제목을 우회하는 직각 연결선만 정합니다. 선택 시 정렬·흐림 처리, 드래그 탐색과 레이어 선택 규칙도 유지합니다. 보라색은 사용하는 곳, 청록색은 이 카드를 사용하는 곳입니다. 선택 연결 수와 전체 연결 수를 함께 표시하며, 선택은 저장소나 계획 데이터를 변경하지 않습니다. **화면 맞춤**은 그래프 너비를 맞춥니다. ELK Layered가 카드까지 배치하는 방식은 아래 비교 CLI에서 시험할 수 있습니다. 어느 방식도 별도 Node·JVM 프로세스를 사용하지 않습니다.

구조 탐색은 저장소를 읽기만 합니다. `git pull`이나 소스 변경은 수행하지 않습니다. 분석은 기존 정적 스캐너를 공유하며 TypeScript AST 분석은 아닙니다. 상대 import, `@/`, FSD 레이어 별칭과 index 재노출을 추적하지만 모든 tsconfig 별칭이나 런타임 의존성을 해석하지는 않습니다. 확인하지 못한 import 수를 하단에 표시합니다. 타입과 동작은 사람이 직접 작성합니다.

## 계획 작성과 내보내기

- **계획**에서 모듈을 추가하고 책임, 신규 / 수정 / 기존 참조, 배치, 파일, 인터페이스의 조건·결과·입력·출력을 편집합니다.
- 모듈을 선택하고 오른쪽에서 의존 대상을 골라 연결합니다. 관계 종류와 사용하는 인터페이스도 편집합니다. 삭제와 변경은 되돌리기 / 다시 실행이 가능합니다.
- **내보내기 → JSON**은 재편집용 원본입니다. 기존 Tauri 클라이언트의 JSON도 불러올 수 있습니다.
- **내보내기 → Markdown**은 사람이 작성한 계획을 AI에 전달하기 위한 문서입니다. 클립보드로 복사할 수도 있습니다.
- `Ctrl+S` / `⌘S`로 JSON 내보내기, 편집 필드 밖에서 `Ctrl+Z` / `⌘Z`로 그래프 되돌리기, Shift를 함께 누르면 다시 실행합니다. 텍스트 필드 안에서는 일반 텍스트 편집 단축키가 동작합니다.

작성 중인 계획은 사용자 폴더의 `SoftwareFactoryNative/session.json`에 자동 저장됩니다. Windows에서는 `%LOCALAPPDATA%`, macOS에서는 `~/Library/Application Support` 아래입니다. 손상된 저장본은 자동으로 덮어쓰지 않으며 오류를 표시합니다. 기존 WebView의 localStorage는 자동 이관하지 않으므로 기존 앱에서 JSON을 내보낸 뒤 가져오세요.

## 원격 저장소

기존 Factory 서버 자원을 그대로 활용할 수 있습니다. 이 버전의 서버를 실행하고 네이티브 앱의 **원격 저장소**에서 HTTPS 주소와 API 토큰을 입력합니다. 로컬 테스트용 HTTP는 loopback에서만 허용합니다.

```sh
export FACTORY_REPO_PATH=/path/to/repository
export FACTORY_API_TOKEN=your-random-secret-at-least-24-characters
export FACTORY_DATA_DIR=./data
cargo run -p factory-server --release
```

Windows PowerShell에서는 `export NAME=value` 대신 `$env:NAME = 'value'`를 사용하세요. 서버는 기본적으로 `127.0.0.1:8787`에 바인딩합니다. 원격 접속에는 기존 Tailscale Serve 구성을 사용할 수 있습니다.

네이티브 앱은 `GET /api/repository/tree`만 호출합니다. 토큰 기억을 선택하면 연결 성공 후 Windows Credential Manager / macOS Keychain에 보관하며, 계획 JSON이나 자동 저장 파일에는 토큰을 기록하지 않습니다. 기존 Tauri 앱이 저장한 토큰을 읽는 버튼도 있습니다.

## 코드 구성

```text
apps/native/                  Rust 네이티브 클라이언트
  build.rs                   Rust 파일별 400줄 제한 검사
  assets/icons/              Windows / macOS 공용 앱 아이콘
  src/app/                   앱 상태, 계획 편집, 저장소 탐색, 파일·연결 작업
  src/app/tests/             창 없이 실제 앱을 조작하는 egui_kittest 회귀 테스트
  src/canvas/                카드, 계획 그래프, 커서 기준 확대·축소
  src/repository_graph/      중첩 그래프 UI, 계산 결과 반영과 그리기
  src/file_access.rs         파일 선택기와 자동 저장 경로의 주입 경계
apps/server/                  원격 저장소 API와 기존 계획 API 실행 프로그램
  src/main.rs                설정과 서버 시작
  src/api.rs                 HTTP 라우터와 읽기 전용 저장소 조회
  src/legacy/                기존 계획 저장·검사·승인과 AI 작업
  src/legacy/planner/        계획 자료형·변환·검증과 테스트
crates/factory-repository/    저장소 구조와 import 관계 관찰
  src/graph/                 경로 탐색, 슬라이스 발견, import 추적과 분석 결과
  src/tree.rs                재귀 계층, index 그룹과 경계의 import 관계
  src/projection.rs          펼침 상태에 따른 그래프와 import 집계
crates/factory-plan/          사용자가 작성하는 계획
  src/lib.rs                 계획 자료형, JSON 호환, 무결성 검사와 Markdown 출력
  src/repository.rs          실제 저장소 그룹을 계획 모듈로 가져오기
crates/factory-session/       작업 세션 저장·복원과 OS별 기본 경로
crates/factory-remote-repository/  원격 저장소 연결
  src/lib.rs                 HTTPS 조회와 인증·리다이렉트 정책
  src/credentials.rs         OS 자격 증명 저장소의 API 토큰
crates/factory-layout/        UI와 독립적인 그래프 배치·연결선 계산
  src/scene.rs               화면에 전달할 카드·연결선 좌표
  src/arrangement.rs         한 줄 계층 배치와 선택 카드 정렬
  src/focus.rs               선택 경계와 직접 연결 관계
  src/layout_engine.rs       ELK 중첩 배치 어댑터
  src/elk_routing.rs         고정 카드 구조의 ELK Libavoid 연결선
  src/embedded_router.rs     내장 계산기의 준비·검증·캐시와 소스 내보내기
  src/routing.rs             연결 지점 배정과 비교용 이전 알고리즘
  src/worker.rs              요청을 합쳐 처리하는 백그라운드 계산기
crates/factory-layout-comparison/  배치 비교, 측정과 JSON/SVG 출력
scripts/macos/                서버 운영 스크립트와 네이티브 .app 패키징
```

`crates/`는 기능별 라이브러리이며, 실행 프로그램의 시작과 조합은 `apps/`가 담당합니다. 각 기능은 자신의 자료형·규칙·입출력 구현을 함께 가집니다. 예를 들어 토큰 저장은 원격 저장소 연결 기능 안에 있습니다. 저장소 관찰은 계획에 의존하지 않고, 계획이 저장소 관찰 결과를 받아 모듈을 가져옵니다. 이 기준은 [crates/AGENTS.md](crates/AGENTS.md)에 기록했습니다.

화면 공용 코드에 Windows/macOS API를 섞지 않고 저장·통신을 분리했습니다. Android 패키징과 작은 화면용 UI는 후속 범위입니다. Android 앱 빌드가 제공되는 것은 아닙니다.

`apps/native`의 모든 Rust 파일은 **파일당 400줄 이하**여야 합니다. 빈 줄·주석·테스트·빌드 스크립트도 포함하며, 아직 모듈에 연결하지 않은 새 파일도 검사합니다. 401줄부터 로컬 Cargo 빌드·검사·테스트와 CI가 실패합니다. 빌드 산출물인 `target`은 제외합니다. 화면 기능은 하위 모듈로 나누고, 재사용할 수 있는 계산·데이터 로직은 공유 crate에 둡니다. 기존 Tauri 클라이언트 코드는 Git 이력에서 확인할 수 있습니다.

각 클론에서 한 번 Git 훅을 연결하면 **커밋 단계에서 먼저 차단**합니다.

```sh
git config --local core.hooksPath .githooks
```

Windows Git Bash와 macOS에서 동작하며 Rust 개발 환경의 `rustc`만 사용합니다. Cargo 빌드나 외부 패키지 설치는 하지 않습니다. 훅은 Git 인덱스에 들어 있는 모든 네이티브 Rust 파일을 검사하므로, 부분 스테이징·이름 변경·삭제도 실제 커밋 내용에 맞게 처리합니다. 작업 폴더에서 파일을 줄였더라도 스테이징된 버전이 401줄이면 커밋이 실패합니다. 새 클론에는 위 설정을 다시 적용해야 합니다.

## 확인

Computer Use나 데스크톱 창 없이 실제 `Workbench`의 UI 코드를 테스트할 수 있습니다.

```sh
# 헤드리스 UI 회귀 테스트만 실행
cargo test -p factory-workbench headless_ --locked
```

`egui_kittest`로 이름이 붙은 위젯에 클릭·키보드·Ctrl+휠 이벤트를 전달하고 앱 상태와 실제 연결선 좌표를 검사합니다. 테스트 저장소, 계획 파일, 자동 저장 경로는 테스트마다 임시 폴더에 만들고 파일 선택기는 미리 지정한 응답으로 대체합니다. 기존 사용자 초안이나 OS 파일 대화상자에 접근하지 않습니다. 저장소 스캐너, 비동기 작업자와 내장 라우터는 실제 구현을 사용하며, 최신 계산이 완료됐는지 확인한 뒤 결과를 검사합니다.

레이어 펼침·접기, 한 줄 배치·선택 연결·흐림 처리, 검색·index 진입점·그룹 복사, Ctrl+휠의 그래프 범위, 계획 편집·되돌리기·다시 실행, 인터페이스·관계 편집, JSON/Markdown 내보내기, JSON 가져오기·거절·취소, 자동 저장·앱 재생성과 손상된 초안 보존을 검사합니다. 접근성 이름과 글꼴 누락 검사도 활성화되어 있습니다. 한글 표시는 실제 네이티브 앱과 같은 시스템 글꼴 대체를 사용합니다.

테스트 도구와 글꼴 설정은 개발 의존성이며 배포 앱에는 추가되지 않습니다. 이 테스트는 UI 로직과 파일 작업의 회귀 검사입니다. OS 창 관리, 시스템 파일 선택기, 자격 증명 저장소 및 GPU 픽셀 출력은 별도의 통합 검사 범위입니다. Windows/macOS의 기존 native CI 테스트에도 자동으로 포함됩니다.

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all -- --check
cargo build -p factory-workbench --release --locked

# 같은 입력으로 ELK / 이전 배치의 계산 시간과 겹침 비교
cargo run -p factory-workbench --release --locked -- --compare-layout /path/to/repository target/elk-comparison
```

비교 결과는 `report.json`, 시나리오별 `*-input.json`, `*-elk.json`, `*-elk.svg`, `*-previous.svg`로 저장됩니다. Layer / Slice 전체 화면을 비교하고, `features/variants-filter`가 있으면 실제 중첩 API·index 그룹도 비교합니다. 지표는 단일 로컬 실행의 측정값이며 `milliseconds`에는 소스 분석 시간이 포함되지 않습니다.

[현재 구조와 데이터 흐름](docs/ARCHITECTURE.md), [이전 AI 계획 생성 계약](docs/LEGACY-PLANNING.md).
