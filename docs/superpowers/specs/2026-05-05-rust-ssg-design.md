# Rust Static Site Generator — Design

**상태:** approved (2026-05-05)
**작성자:** hyeyoom + Claude

## 1. Goal

개인 기술 블로그용 미니멀 Rust SSG. 마크다운 글을 RFC 스타일 정적 사이트로 변환하여 GitHub Pages에 배포한다. 본인 사용 목적이며 다른 사용자/테마 커스터마이징은 고려하지 않는다.

## 2. Scope

**In scope**

- 마크다운(CommonMark + 각주 + 표 + GFM 일부) → HTML
- 수식: `$...$`, `$$...$$` 통과 — 클라이언트 KaTeX(auto-render)가 렌더
- 이미지: `static/` 디렉토리 자원을 마크다운에서 `![](/foo.png)`로 참조
- 페이지: 홈(글 목록), 개별 글, About
- SEO: title, description, canonical, Open Graph, Twitter Card, JSON-LD(Article), `sitemap.xml`, `robots.txt`, `rss.xml`
- 단일 RFC 스타일 테마 (직접 작성한 CSS, 의존성 없음)
- CLI: `build`, `new`
- GitHub Actions 배포 워크플로

**Out of scope**

- `serve` 개발 서버, 라이브 리로드 (사용자가 `python3 -m http.server`로 대체)
- 코드 syntax highlighting (RFC 모노크롬 정신상 의도적 제외; `<pre>` 그대로)
- 태그/카테고리/시리즈
- 다국어
- 검색, 댓글
- 테마 커스터마이징, 멀티 저자
- 이미지 최적화/리사이즈
- 드래프트, 예약 발행

## 3. Tech stack

| 라이브러리 | 용도 |
|---|---|
| `clap` (derive) | CLI 서브커맨드 |
| `serde` + `toml` | 설정/frontmatter 파싱 |
| `pulldown-cmark` | 마크다운 → HTML |
| `tera` | HTML 템플릿 |
| `walkdir` | 콘텐츠 디렉토리 스캔 |
| `chrono` | 날짜 파싱/포매팅 |
| `slug` | URL-safe 슬러그 |
| `anyhow` | 에러 |
| `tempfile` (dev) | 통합 테스트 임시 디렉토리 |

## 4. Site & content layout

```
project-root/
├── config.toml
├── content/
│   ├── about.md
│   └── posts/
│       ├── 2026-05-05-hello-world.md
│       └── 2026-05-10-math-test.md
├── templates/
│   ├── base.html
│   ├── index.html
│   ├── post.html
│   └── about.html
├── static/
│   ├── style.css
│   ├── favicon.ico
│   └── images/...
├── .github/workflows/deploy.yml
└── public/                     # 빌드 산출물 (gitignore)
    ├── index.html
    ├── posts/<slug>/index.html
    ├── about/index.html
    ├── sitemap.xml
    ├── robots.txt
    ├── rss.xml
    └── (static/* 복사)
```

### 4.1 `config.toml`

```toml
title = "사이트 이름"
author = "Hyeyoom"
description = "기본 사이트 설명"
base_url = "https://hyeyoom.github.io/ssg"
language = "ko"
```

### 4.2 Frontmatter (TOML, `+++` 구분자)

```
+++
title = "글 제목"
date = "2026-05-05"
description = "메타 디스크립션 (선택, 없으면 본문 첫 문장 잘라서 사용 안 함 — 그냥 사이트 description)"
math = false
+++

본문 내용
```

- `title`, `date` 필수
- `description` 선택 (없으면 사이트 default description 사용)
- `math = true`이면 해당 글의 HTML에 KaTeX 스크립트/CSS 삽입

### 4.3 슬러그 규칙

- `content/posts/2026-05-05-hello-world.md` → URL `/posts/hello-world/` (날짜 prefix 자동 제거)
- `content/about.md` → URL `/about/`
- 글 파일명이 `YYYY-MM-DD-` 패턴이 아니면 stem 그대로 슬러그

## 5. Markdown features

- CommonMark 기본
- `Options::ENABLE_FOOTNOTES` — `[^1]` 각주
- `Options::ENABLE_TABLES`, `ENABLE_STRIKETHROUGH`, `ENABLE_TASKLISTS`
- 수식: pulldown-cmark는 `$...$`, `$$...$$`를 일반 텍스트로 통과 → 클라이언트 KaTeX `auto-render` 확장이 처리. 빌드 시 별도 처리 없음.
- 이미지: 표준 `![alt](path)` → `<img>` 태그
- 코드 블록: 그대로 `<pre><code class="language-X">` 출력. CSS는 monospace 박스만.

## 6. Visual design (RFC style)

**원칙: 80년대 plain-text RFC 문서 + 인라인 이미지/수식만 허용된 모양.**

- 폰트: pure monospace 스택 (`ui-monospace, "SF Mono", "JetBrains Mono", Menlo, Consolas, monospace`)
- 색: 검정(#000) / 흰색(#fff) / 회색(#555) 3색만
- 본문 max-width: 72ch
- 헤더: ALL CAPS (h1, h2)는 `text-transform: uppercase`, h3+는 일반
- 섹션 번호 매기는 건 사용자가 직접 (자동 번호 매기기 X)
- 링크: 밑줄, 검정 유지 (`color: inherit`)
- 코드 블록: `#f4f4f4` 배경, 1px 회색 테두리
- 수평 구분선: `border-top: 1px solid #000` (얇게)
- 이미지: `display: block; margin: 1em auto; max-width: 100%`
- 각주: 페이지 하단 `<hr>` 아래 작은 글씨로
- 헤더 영역: 사이트명 + 글 메타(`AUTHOR | DATE | YYYY-MM-DD`)는 RFC 헤더처럼 좌우 배치 plain text

CSS는 `static/style.css` 한 파일에 작성, 100줄 미만 목표.

## 7. SEO

각 페이지(`<head>`)에 포함:

- `<title>` — 글: `"글제목 — 사이트명"`, 인덱스: `사이트명`, about: `"About — 사이트명"`
- `<meta name="description" content="...">`
- `<link rel="canonical" href="<base_url>/<path>/">`
- Open Graph: `og:title`, `og:description`, `og:type`(article|website), `og:url`
- Twitter Card: `twitter:card=summary`
- 글 페이지 한정: JSON-LD `<script type="application/ld+json">` Article 스키마 (headline, datePublished, author, mainEntityOfPage)

사이트 단위 산출물:

- `sitemap.xml` — 홈 + about + 모든 글, `<lastmod>`는 글 date
- `robots.txt` — `User-agent: *`, `Allow: /`, `Sitemap: <base_url>/sitemap.xml`
- `rss.xml` — RSS 2.0, 최신 20개 글, 본문 HTML 포함(content:encoded는 생략, description에 truncated)

## 8. CLI

```
ssg new "글 제목"     # content/posts/YYYY-MM-DD-slugified-title.md 생성, frontmatter 채워서
ssg build              # public/ 에 정적 사이트 생성 (clean rebuild)
```

`build` 단계:

1. config.toml 로드
2. `content/`, `templates/`, `static/` 존재 확인
3. `public/` 비우기 (또는 생성)
4. content 스캔 → Post 리스트
5. Tera로 페이지 렌더 (홈, 글 N개, about)
6. sitemap.xml, robots.txt, rss.xml 생성
7. `static/` → `public/` 디렉토리 재귀 복사

## 9. Module decomposition

```
src/
├── main.rs        # CLI dispatch
├── config.rs      # config.toml 로드
├── content.rs     # Frontmatter 파싱, Post struct, slug 추출
├── scanner.rs     # walkdir로 content/ 스캔
├── markdown.rs    # md → html
├── render.rs      # Tera 통합, 페이지별 렌더 함수
├── feed.rs        # RSS 생성
├── sitemap.rs     # sitemap.xml + robots.txt
├── build.rs       # 빌드 오케스트레이션 (모듈 묶어서 public/ 생성)
└── new.rs         # `new` 명령 구현
```

각 파일 < 200줄 목표.

## 10. Testing strategy

- 모든 모듈에 unit test (TDD: 테스트 먼저 → 구현)
- 통합 테스트 `tests/build_integration.rs`: 픽스쳐 디렉토리(`tests/fixtures/site/`)로 `build` 실행 후 `public/` 산출물 검증 (HTML 포함 여부, sitemap URL, RSS 존재 등)

## 11. Deployment

`.github/workflows/deploy.yml` — `main` 브랜치 push 시:

1. `actions/checkout`
2. `dtolnay/rust-toolchain@stable`
3. `Swatinem/rust-cache@v2` — cargo 캐시
4. `cargo run --release -- build`
5. `actions/upload-pages-artifact@v3` (path: `public/`)
6. `actions/deploy-pages@v4`

저장소 Settings → Pages → "GitHub Actions" 모드 사용. `gh-pages` 브랜치 안 씀.

## 12. Repository starter content

빌드 가능한 최소 예제:

- `config.toml` 채워둠
- `content/about.md` 한 줄
- `content/posts/2026-05-05-hello-world.md` — 헤더, 단락, **bold**, 각주, 이미지 placeholder, 수식 인라인+디스플레이 포함
- `static/style.css` — 위 6절 디자인
- 빌드 후 정상 출력 확인

---

**다음 단계:** 이 spec을 바탕으로 implementation plan 작성.
