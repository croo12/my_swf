# Planning Workbench · Native

사람이 직접 설계하고, 실제 저장소 구조를 보며 구현 계획을 작성하는 Windows / macOS 앱입니다.
기존 화면의 어두운 색상, 레이어별 카드와 계획 캔버스를 Rust + egui/eframe으로 구현합니다. 네이티브 클라이언트는 WebView·Node·React 런타임을 사용하지 않습니다.

현재 범위는 **저장소 구조 탐색 → 수동 계획 편집 → JSON / Markdown 내보내기**입니다. AI 작업 실행은 연결하지 않았습니다.

## 실행

Rust 1.95 이상이 필요합니다. Windows에는 MSVC C++ Build Tools, macOS에는 Xcode Command Line Tools가 필요합니다.

```sh
# 프로젝트 루트에서 실행
cargo run -p factory-workbench --release

# 실행하면서 분석할 저장소 지정 (공백이 있는 경로는 따옴표로 감싸세요)
cargo run -p factory-workbench --release -- /path/to/your/repository

# 독립 실행 파일 빌드
cargo build -p factory-workbench --release --locked
```

Windows 실행 파일은 `target/release/factory-workbench.exe`, macOS 실행 파일은 `target/release/factory-workbench`입니다. 앱을 실행한 뒤 **폴더 열기**로 로컬 저장소를 선택할 수도 있습니다. 로컬 분석에는 서버나 AI 계정이 필요하지 않습니다.

Windows에서 빌드·실행을 확인합니다. macOS는 동일 소스와 CI 빌드 경로를 제공하며, 이 Windows 작업 환경에서는 macOS 실행을 검증하지 않습니다. 릴리스 태그 빌드는 Windows ZIP과 macOS universal `.app` ZIP을 생성합니다. 배포용 코드 서명·공증은 별도입니다.

## 저장소 구조 보기

1. FSD 저장소를 엽니다. 모노레포 안에서도 소스 루트를 찾습니다. 직접 지정하려면 `FACTORY_SOURCE_ROOT`를 저장소 기준 상대 경로로 설정합니다.
2. **의존 관계 그래프 안에서 Layer → Slice → 내부 폴더 / index 그룹 → 하위 그룹·파일** 순서로 노드를 펼칩니다. 펼친 부모 박스 안에 자식 노드가 나타나며 다른 레이어·형제 노드와 연결선도 같은 캔버스에 유지됩니다. `app`과 `shared`는 FSD 특성상 slice 없이 내부 그룹을 펼칩니다.
3. 펼친 그룹의 제목을 누르면 접힙니다. 접힌 그룹의 외부 import는 그룹 경계에 모이고, 펼치면 실제 내부 노드로 연결됩니다. **Layer 보기 / Slice 보기**, 확대 도구, 스크롤로 탐색합니다. 검색 결과나 의존 대상 파일을 선택하면 필요한 상위 그룹을 펼쳐 그래프에서 해당 위치를 보여줍니다. Shift+클릭은 펼침 상태를 바꾸지 않고 관계만 선택합니다.
4. `index` 배지는 `index.ts`, `index.tsx`, `index.js` 등의 공개 진입점이 있는 디렉터리를 뜻합니다. 일반 폴더도 실제 계층을 유지하며 표시합니다.
5. 연결선에 마우스를 올리면 import 이름과 파일 관계 수를 확인합니다. 오른쪽에서는 선택한 그룹의 파일 목록과 경계를 넘는 실제 import 관계를 봅니다. **이 그룹을 계획에 추가**로 세그먼트·그룹·파일을 계획에 복사합니다.

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
  src/app.rs                  화면 구성, 탐색, 수동 편집, 파일 작업
  src/canvas.rs               기존 스타일의 카드와 레이어별 계획 그래프
  src/repository_graph.rs     그룹 안에서 펼치고 접는 중첩 저장소 그래프
  src/routing.rs              노드와 그룹 제목을 우회하는 직교 연결선
  src/storage.rs              로컬 초안 자동 저장
  src/transport.rs            선택적인 원격 구조 조회 / OS 토큰 저장
crates/factory-core/          클라이언트와 서버가 공유하는 Rust 코드
  src/graph.rs                기존 FSD / import 스캐너
  src/repository.rs           재귀 계층, index 그룹, 그룹 경계의 import 관계
  src/repository_graph.rs     펼침 상태에 따른 그래프와 import 집계
  src/plan.rs                 계획 JSON 호환, 기본 무결성 검사, Markdown 출력
crates/factory-server/        기존 Axum 서버 + 읽기 전용 계층 API
apps/client/                  기존 Tauri / React UI와 아이콘 (이전 버전 참고용)
scripts/macos/                서버 운영 스크립트와 네이티브 .app 패키징
```

화면 공용 코드에 Windows/macOS API를 섞지 않고 저장·통신을 분리했습니다. Android 패키징과 작은 화면용 UI는 후속 범위입니다. Android 앱 빌드가 제공되는 것은 아닙니다.

## 확인

```sh
cargo test -p factory-core -p factory-server --locked
cargo clippy -p factory-workbench --locked -- -D warnings
cargo fmt -p factory-workbench -- --check
cargo build -p factory-workbench --release --locked
```

[현재 구조와 데이터 흐름](docs/ARCHITECTURE.md), [이전 Tauri 실행 안내](docs/LEGACY-TAURI.md), [이전 AI 계획 생성 계약](docs/LEGACY-PLANNING.md).
