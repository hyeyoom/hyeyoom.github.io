# ssg

Rust로 작성한 본인용 미니멀 정적 사이트 생성기.

## 사용법

새 글:

    cargo run -- new "글 제목"

빌드:

    cargo run -- build

미리보기:

    cd public && python3 -m http.server 8000

## 기능

- 마크다운 (각주, 표, 취소선, 태스크리스트)
- 코드 syntax highlighting (syntect, InspiredGitHub theme)
- 수식 (KaTeX 클라이언트 렌더; frontmatter `math = true`)
- 이미지 (`static/images/...` → `/images/...`)
- SEO: title/description/canonical/Open Graph/Twitter Card/JSON-LD
- sitemap.xml, robots.txt, rss.xml
- RFC 스타일 모노크롬 CSS 단일 테마

## 디렉토리

```
config.toml
content/
  about.md
  posts/YYYY-MM-DD-slug.md
templates/*.html
static/style.css
static/images/...
```

## 배포

저장소 Settings → Pages → Source: **GitHub Actions**.
`main` 브랜치 push 시 `.github/workflows/deploy.yml`이 자동으로 빌드 후 배포.
