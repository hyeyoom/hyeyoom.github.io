# 왜 만들었는가

개인적으로 휘갈기는 노트를 정리하려고 만들었다. 초기 MVP 설정은 다음과 같이 잡았다.

- 지식을 [제텔카스텐](제텔카스텐) 형태로 정리하고 싶다.
- 웹에서 어디서든 보고 싶다.
- 하지만 만드는데 큰 공은 들이고 싶지 않다.
    - 심지어 스타일 없는 html도 괜찮음
    - 검색? 당장 필요 없음
    - SEO? 당장 필요 없음
    - 서버 관리 하고 싶지 않음. 회사에서 충분히 하고 있음.
      - 그러므로 [github pages](https://pages.github.com/)를 사용할 것임

위와 같은 방향을 잡으니 다음 세부사항들이 결정되었다.

- 문서 편집은 vimwiki 사용
- 문서 저장은 github repository 사용
- 문서 배포는 github pages 사용
- 도메인도 github pages 사용

이를 방향으로 목표 설정을 했다.

Goals

- vimwiki로 정리하는 지식을 markdown으로 저장한다.
- markdown으로 저장된 지식을 html으로 변경한다.
- ghpage에 배포한다.

Non-Goals

- 디자인 X
- SEO X
- 검색 지원 X
- 파서를 직접 만들지 마라

위에서 설정한 것들을 토대로 0.0.1 버전이 나왔다. 뚝딱뚝딱 만든거라 시간 소요도 거의 없었다. 
홍천에서 휴가 중에 만들었다. 개인 위키를 갖고 싶다는 생각은 있었는데 그렇게 강하진 않아서 남는 시간에 만들고 싶었다.
결과적으로 만족스럽다. 디자인은 개판이지만.

# 릴리즈 노트

## 0.0.2

심하게 많이 건너 뛴거 같지만..

- [x] 갱신된 문서만 다시 렌더링하기
- [x] 기본적인 스타일 적용하기
- [x] sitemap.xml 생성하기
- [x] Google Analytics

what is next?

- [ ] 제외된 문서 자동으로 제거하기
- [ ] client side 검색 기능 ([lunr.js](https://lunrjs.com/))
- [ ] SEO 지원하기

## 0.0.1

- [x] markdown to HTML 변환
- [x] 다른 문서로 연결하기 기능

what is next?

- [ ] 제외된 문서 자동으로 제거하기
- [ ] 갱신된 문서만 다시 렌더링하기
- [ ] 기본적인 스타일 적용하기
- [ ] sitemap.xml 생성하기
- [ ] client side 검색 기능 ([lunr.js](https://lunrjs.com/))
- [ ] Google Analytics
- [ ] SEO 지원하기
