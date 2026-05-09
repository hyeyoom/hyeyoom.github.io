+++
title = "일관성 모델이란 무엇인가"
date = "2026-05-09T18:20:51+09:00"
description = "분산 시스템에서 consistency model이 뜻하는 것: 데이터가 같다는 문제가 아니라 관측 가능한 세계의 순서와 인과를 어디까지 지킬 것인가의 문제"
math = false
+++

# 일관성 모델이란 무엇인가

분산 시스템에서 일관성(consistency)은 결국:

> 관측 가능한 세계가 하나의 규칙 있는 현실처럼 보이느냐

에 대한 이야기다.

조금 더 엔지니어링 언어로 말하면:

> 여러 주체(스레드, 프로세스, 노드, 사용자)가 데이터를 읽고 쓸 때,
> 시스템이 상태 변화를 어떤 순서와 규칙으로 보여줄 것인지에 대한 계약(contract)

이다.

흔히 일관성을 "데이터가 같음" 정도로 이해한다. 하지만 실제 문제는 더 넓다. 핵심은 값이 같은지 그 자체가 아니라, 어떤 write가 언제, 누구에게, 어떤 순서로 보이는가다.

Jepsen은 consistency model을 **허용되는 history의 집합**으로 설명한다.[^jepsen-models] 어떤 실행 기록이 그 집합 안에 들어가면 "그 모델을 만족한다"고 말하고, 들어가지 않으면 "위반했다"고 말한다. 이 관점이 중요하다. 일관성은 막연히 "데이터가 이상하다"는 감각이 아니라, 어떤 실행을 정상으로 인정할지 정하는 규칙이다.

주의
> 이 글만 보면 바보가 될 수 있습니다.
>
> consistency model 이름은 DB 문서, 논문, 제품 문서마다 조금씩 다르게 쓰인다. 특히 `Serializable`, `Repeatable Read`, `Snapshot Isolation`은 구현체마다 의미가 미묘하게 갈린다. 이름을 외우기보다 **무엇을 보장하고, 무엇을 허용하는지**를 봐야 한다.

---

## 0. 한 장 요약

일관성 모델은 다음 질문에 답한다.

```text
내가 방금 쓴 값을 내가 다시 읽을 수 있는가?
다른 사람이 완료한 write를 이후 read가 반드시 봐야 하는가?
여러 write의 순서는 모두에게 같게 보이는가?
transaction 안의 여러 변경은 한꺼번에 보이는가?
서로 다른 노드가 잠시 다른 현실을 봐도 되는가?
partition 중에도 계속 읽고 쓸 수 있어야 하는가?
```

대략 이렇게 잡고 읽으면 된다.

| 모델 | 핵심 질문 | 실무 감각 |
|---|---|---|
| Strong Serializability | transaction 전체가 현실 시간 순서까지 지키는가 | 가장 직관적인 "하나의 DB" |
| Serializability | transaction들이 어떤 직렬 순서로 실행된 것처럼 보이는가 | transaction 정합성은 강하지만 최신성은 별개 |
| Linearizability | 단일 객체 operation이 현실 시간 순서를 지키는가 | 완료된 write 이후 read는 그 이후 상태를 봄 |
| Sequential Consistency | 모두가 같은 전체 순서를 보되, 현실 시간은 느슨해도 되는가 | 단일 순서는 있지만 stale read 가능 |
| Causal Consistency | 원인과 결과의 순서를 보존하는가 | 질문보다 답변이 먼저 보이면 안 됨 |
| Read Your Writes | 내가 쓴 것을 내가 다시 볼 수 있는가 | 글 작성 후 내 화면에 바로 보여야 함 |
| Monotonic Reads | 내 read가 뒤로 가지 않는가 | 새로고침했더니 예전 상태로 돌아가면 안 됨 |
| Monotonic Writes | 내가 쓴 순서가 뒤집히지 않는가 | 프로필 변경 순서가 섞이면 안 됨 |
| Read Committed | commit 안 된 값을 읽지 않는가 | dirty read 방지 |
| Snapshot Isolation | transaction마다 일관된 snapshot을 보는가 | 읽기는 편하지만 write skew 가능 |

한 줄로 줄이면:

```text
consistency = 상태 변화의 가시성, 순서, 인과관계에 대한 계약
```

---

## 1. 일관성은 "같다"의 문제가 아니다

예를 들어 계좌 잔액이 있다고 하자.

```text
초기 상태: balance = 1000

A: 100원 출금
B: 잔액 조회
```

이때 B는 무엇을 봐야 할까?

```text
1000원: 출금 전 상태
900원: 출금 후 상태
깨진 중간 상태: 출금 transaction의 일부만 반영된 상태
```

정답은 하나가 아니다. A의 출금이 아직 진행 중인지, 완료됐는지, B의 조회가 어느 replica로 갔는지, 시스템이 어떤 consistency model을 약속했는지에 따라 달라진다.

강한 모델에서는 A의 출금이 완료된 뒤 시작한 B의 조회가 출금 전 상태를 보면 안 된다. 반대로 eventually consistent한 read replica라면 잠깐 예전 잔액을 볼 수도 있다. 대신 언젠가는 새 상태로 수렴해야 한다.

그래서 consistency는 "모든 복사본이 항상 같은가"가 아니다. 더 정확히는:

```text
어떤 write가
언제
누구에게
어떤 순서로
보이는가
```

의 문제다.

---

## 2. history: 시스템이 남긴 실행 기록

일관성 모델을 이해하려면 먼저 history를 봐야 한다.

history는 operation들의 실행 기록이다. 여기서 operation은 단순한 함수 호출일 수도 있고, DB transaction일 수도 있다. Jepsen식으로 보면 operation에는 시작(invocation)과 종료(completion)가 있고, 두 operation의 실행 시간이 겹치면 concurrent하다고 본다.[^jepsen-history]

예를 들어:

```text
time ─────────────────────────>

A: write x=1  [ start ───── end ]
B: read x             [ start ───── end ]
C: read x                           [ start ─ end ]
```

A와 B는 시간이 겹친다. 그러면 B가 `0`을 읽어도, `1`을 읽어도 모델에 따라 허용될 수 있다.

하지만 A가 끝난 뒤 C가 시작했다면 이야기가 달라진다. linearizability는 C가 A 이후 상태를 봐야 한다고 요구한다. 반면 serializability는 transaction들이 어떤 직렬 순서로 설명되기만 하면 되므로, 현실 시간 순서를 반드시 반영하지 않을 수 있다.

이 차이가 핵심이다.

---

## 3. 핵심은 시간 그 자체가 아니라 순서다

분산 시스템에서 물리적 시각은 믿기 어렵다. 노드마다 clock drift가 있고, 메시지는 지연된다. 어떤 요청은 클라이언트에서는 timeout이 났지만 서버에서는 성공했을 수도 있다.

그래서 consistency model은 "지금 몇 시 몇 분 몇 초인가"보다 **사건 사이의 순서**를 다룬다.

다만 모든 순서가 같은 것은 아니다.

| 순서 | 뜻 | 예 |
|---|---|---|
| Session order | 같은 process/client 안에서 일어난 순서 | 내가 글을 쓰고, 그다음 내 글 목록을 봄 |
| Causal order | 원인과 결과로 연결된 순서 | 질문이 있었기 때문에 답변이 생김 |
| Real-time order | operation A가 끝난 뒤 B가 시작되는 현실 시간 순서 | 결제 승인 완료 후 주문 조회 |
| Total order | 모든 operation을 하나의 전체 순서로 배열 | 모든 노드가 write 순서를 같게 봄 |

약한 모델은 session order의 일부만 보존한다. causal consistency는 인과관계를 보존한다. sequential consistency는 모든 operation의 total order를 요구하지만 real-time order는 요구하지 않는다. linearizability는 total order에 real-time order까지 요구한다.[^jepsen-linearizable]

그래서 "일관성은 시간보다 순서다"라는 말은 이렇게 바꿔 말하는 게 정확하다.

> 일관성 모델은 물리적 시각 자체보다 관측 가능한 사건들 사이의 순서를 다룬다. 어떤 모델은 프로그램 순서만 보존하고, 어떤 모델은 인과 순서를 보존하며, 더 강한 모델은 현실 시간 순서까지 보존한다.

---

## 4. 강한 일관성은 단일 현실을 만든다

강한 일관성의 본질은:

> 시스템 전체가 마치 하나의 컴퓨터처럼 보이게 하는 것

이다.

사용자는 시스템이 분산되어 있다는 사실을 거의 느끼지 못한다. 이미 완료된 변화가 이후 관측에서 사라지지 않고, 여러 사용자가 하나의 순서 있는 현실을 바라보는 것처럼 행동할 수 있다.

하지만 이 현실은 공짜가 아니다.

```text
quorum
leader coordination
replication synchronization
distributed transaction
lock
consensus
```

같은 비용을 낸다. latency가 늘고, throughput이 줄고, partition 상황에서는 일부 요청을 거절하거나 대기시켜야 한다.

즉 강한 일관성의 비용은:

```text
현실을 하나로 유지하는 비용
```

이다.

---

## 5. Eventually consistent 시스템은 현실이 갈라지는 것을 허용한다

반대로 eventually consistent한 시스템은 잠시 동안 현실이 갈라지는 것을 허용한다.

```text
A가 본 세계 != B가 본 세계
```

예를 들어:

```text
사용자 A: 방금 수정한 프로필 이름을 봄
사용자 B: 아직 예전 프로필 이름을 봄
```

이 상태는 잠깐 허용된다. 단, 새로운 변경이 멈추고 복제와 전파가 충분히 진행되면 결국 같은 상태로 수렴해야 한다.

이 느슨함 덕분에 시스템은 더 멀리 확장되고, 장애 상황에서도 더 많은 요청을 받아낼 수 있다.

대표적인 예는 다음과 같다.

```text
CDN cache
search index
analytics
feed materialization
read replica
multi-region eventually consistent store
```

중요한 건 "eventual이면 대충 해도 된다"가 아니다. eventual consistency는 **불일치가 잠깐 노출될 수 있음을 제품과 도메인이 감당한다**는 선택이다.

---

## 6. Jepsen 모델 지도

Jepsen의 consistency model 지도는 크게 두 계열을 합쳐 보여준다.[^jepsen-map]

하나는 multi-object transaction 계열이다.

```text
Strong Serializability
└─ Serializability
   ├─ Repeatable Read ────────┬─ Cursor Stability ─┐
   │                           └─ Monotonic Atomic View
   └─ Snapshot Isolation ──────── Monotonic Atomic View

Cursor Stability ───────┐
Monotonic Atomic View ──┴─ Read Committed ── Read Uncommitted
```

다른 하나는 single-object operation 계열이다.

```text
Strong Serializability
└─ Linearizability
   └─ Sequential Consistency
      └─ Causal Consistency
         ├─ Writes Follow Reads
         └─ PRAM
            ├─ Monotonic Reads
            ├─ Monotonic Writes
            └─ Read Your Writes
```

여기서 "A가 B를 imply한다"는 말은 A를 만족하는 모든 history가 B도 만족한다는 뜻이다. A가 더 강한 모델일수록 허용하는 history는 더 적다.

즉:

```text
강한 모델 = 더 적은 이상 현상을 허용
약한 모델 = 더 많은 실행을 정상으로 인정
```

이다.

다만 모든 모델을 한 줄로 세울 수는 없다. 예를 들어 Jepsen은 Snapshot Isolation과 Repeatable Read를 서로 직접 비교하기 어려운 모델로 설명한다. Snapshot Isolation은 write skew를 허용할 수 있고, Repeatable Read는 predicate read에서 phantom 계열 현상을 허용할 수 있다. 둘은 서로 다른 이상 현상을 막고, 서로 다른 이상 현상을 허용한다.

---

## 7. Strong Serializability: transaction 전체가 하나의 현실처럼 보인다

Strong Serializability는 strict serializability라고도 부른다.

직관적으로는:

```text
transaction들이 하나의 순서로 실행된 것처럼 보이고,
그 순서가 현실 시간 순서와도 맞아야 한다.
```

예를 들어 transaction A가 끝난 뒤 transaction B가 시작됐다면, 직렬화 순서에서도 A가 B보다 앞서야 한다.[^jepsen-strong-serializable]

이 모델은 두 가지를 합친 것으로 볼 수 있다.

```text
Serializability: transaction들이 직렬 순서로 보임
Linearizability: 완료된 operation 이후의 operation은 그 이후 상태를 봄
```

그래서 strong serializable한 DB는 "DB 전체가 하나의 linearizable object처럼 보인다"고 생각해도 크게 틀리지 않다.

실무에서는 가장 직관적인 모델이다.

```text
방금 완료된 결제 transaction은 이후 주문 조회에서 보여야 한다.
재고 차감과 주문 생성은 중간 상태 없이 함께 보이거나 함께 안 보여야 한다.
동시에 여러 객체를 바꿔도 하나의 순서 있는 현실처럼 보여야 한다.
```

대신 가장 비싸다. network partition 상황에서 모든 노드가 계속 요청을 처리할 수는 없다.

---

## 8. Serializability: transaction은 직렬처럼 보이지만, 최신성은 별개다

Serializability는 transaction들이 하나의 전체 순서로 실행된 것처럼 보이면 된다.[^jepsen-serializable]

```text
실제 실행:
T1과 T2가 동시에 실행됨

관측 결과:
T1 다음 T2로 실행된 것처럼 보이거나
T2 다음 T1으로 실행된 것처럼 보이면 됨
```

중요한 함정이 있다.

Serializability는 real-time order를 요구하지 않는다. A가 write를 완료한 뒤 B가 read를 시작했더라도, B가 반드시 A의 write를 봐야 하는 것은 아니다. 심지어 같은 process가 이전 transaction에서 본 write를 다음 transaction에서 못 볼 수도 있다.

처음 들으면 이상하다. 하지만 serializability의 핵심은 "어떤 직렬 순서로 설명할 수 있는가"이지, "현실 시간 순서와 맞는가"가 아니다.

그래서 "Serializable이면 최신값을 읽겠지"는 위험한 오해다.

```text
transaction 정합성: 강함
현실 시간 최신성: 보장하지 않음
```

최신성과 직관적인 사용자 경험까지 필요하다면 strong serializability를 봐야 한다.

---

## 9. Linearizability: 단일 객체에 대한 현실 시간 보장

Linearizability는 single-object consistency model 중 가장 강한 축에 속한다.

직관적으로는:

```text
모든 operation이 어느 한순간에 원자적으로 일어난 것처럼 보이고,
그 순서가 현실 시간 순서와 맞아야 한다.
```

operation A가 끝난 뒤 operation B가 시작됐다면, B는 A 이후에 일어난 것처럼 보여야 한다.[^jepsen-linearizable]

예를 들어 key-value store에서 같은 key `x`에 대해:

```text
A: write x=1 완료
B: read x 시작
```

B가 A 이후에 시작했다면 B는 `x=1` 또는 그 이후 값을 봐야 한다. `x=0`을 보면 linearizability 위반이다.

다만 "object"의 범위가 중요하다. 어떤 시스템은 key 하나에 대해서만 linearizable할 수 있고, 어떤 시스템은 table 단위나 DB 전체 단위로 제공할 수 있다.

```text
single key linearizable
multi-key linearizable
table-level linearizable
database-level linearizable
```

각각 비용과 구현 난이도가 다르다.

여러 객체에 걸친 transaction 전체에 linearizability가 필요하다면 strong serializability 쪽 문제다.

---

## 10. Sequential Consistency: 모두가 같은 순서를 보지만, 현실 시간은 느슨하다

Sequential consistency는 모든 operation이 하나의 전체 순서로 실행된 것처럼 보이고, 각 process 내부의 순서는 그 전체 순서 안에서 보존되어야 한다.[^jepsen-sequential]

예를 들어:

```text
Client A:
1. write x=1
2. write x=2

전체 순서에서도 x=1은 x=2보다 앞서야 한다.
```

하지만 real-time order는 보장하지 않는다. 어떤 process는 다른 process보다 훨씬 앞서거나 뒤처진 상태를 읽을 수 있다. stale read가 가능하다.

그래도 모든 process가 공유하는 total order가 있다는 점에서 생각보다 강한 모델이다.

비유하면 이렇다.

```text
모두가 같은 영화 필름을 보긴 한다.
다만 누군가는 늦게 보고 있을 수 있다.
```

이다.

---

## 11. Causal Consistency: 인과관계는 뒤집지 않는다

Causal consistency는 causally related operation의 순서를 모든 process가 같게 보도록 요구한다.[^jepsen-causal]

예를 들어 채팅방을 생각해보자.

```text
1. A: "점심 먹을래?"
2. B: "좋아"
3. C: "난 안 돼"
```

B와 C의 답변은 서로 독립적일 수 있다. 어떤 사람은 "좋아"를 먼저 보고, 다른 사람은 "난 안 돼"를 먼저 볼 수 있다.

하지만 누구도 답변을 질문보다 먼저 보면 안 된다.

```text
가능:
질문 -> 좋아 -> 난 안 돼
질문 -> 난 안 돼 -> 좋아

불가능:
좋아 -> 질문
난 안 돼 -> 질문
```

이것이 causal consistency의 직관이다.

이 모델은 total order를 요구하지 않는다. 모든 사건을 하나의 순서로 줄 세우지는 않는다. 대신 원인과 결과로 연결된 사건의 순서만 지킨다.

사용자 경험에서는 이 보장이 매우 중요하다.

```text
결제 승인 후 주문 생성
댓글 작성 후 알림 발송
파일 업로드 후 공유 링크 생성
초대 수락 후 워크스페이스 접근 허용
```

이런 흐름에서 인과가 깨지면 사람은 즉시 "버그났다"고 느낀다.

---

## 12. Session guarantee: 작지만 사용자에게는 큰 보장

분산 시스템에서 항상 linearizability가 필요한 것은 아니다. 하지만 최소한 사용자 한 명의 경험은 자연스러워야 할 때가 많다.

이때 중요한 것이 session guarantee다.

### Read Your Writes

Read your writes는 내가 쓴 값을 내가 나중에 읽으면 볼 수 있어야 한다는 보장이다.[^jepsen-ryw]

```text
1. 내가 게시글 작성
2. 내 게시글 목록 새로고침
3. 방금 쓴 게시글이 보여야 함
```

다른 사용자에게도 바로 보여야 한다는 말은 아니다. **적어도 나에게는** 보여야 한다는 뜻이다.

이 보장이 없으면 사용자는 저장 버튼을 여러 번 누르거나 "글이 날아갔다"고 생각한다.

### Monotonic Reads

Monotonic reads는 같은 process의 read가 뒤로 가지 않는다는 보장이다.[^jepsen-monotonic-reads]

```text
1. 내 프로필 이름이 "Mijin"으로 바뀐 것을 봄
2. 새로고침
3. 다시 예전 이름 "User123"을 보면 안 됨
```

한 번 새 현실을 봤다면, 같은 사용자의 이후 read는 그보다 과거로 돌아가면 안 된다.

### Monotonic Writes

Monotonic writes는 같은 process가 수행한 write 순서가 모든 곳에서 보존되어야 한다는 보장이다.[^jepsen-monotonic-writes]

```text
1. 이름 변경: "Mijin"
2. 소개 변경: "Backend engineer"
```

시스템이 이 순서를 뒤집어 적용하면 사용자의 의도가 깨질 수 있다.

### Writes Follow Reads

Writes follow reads는 내가 어떤 값을 읽고 나중에 write를 했다면, 그 write가 내가 읽은 값 이후의 세계에 붙어야 한다는 보장이다.[^jepsen-wfr]

쉽게 말하면:

```text
내가 읽은 과거를, 내 다음 write가 부정하면 안 된다.
```

예를 들어 문서 버전 `v3`를 읽고 거기에 댓글을 달았다면, 그 댓글은 `v3` 이후의 문맥에 붙어야 한다. `v1`만 아는 세계에 댓글이 들어가면 인과가 깨진다.

### PRAM

PRAM은 같은 process가 수행한 write들의 순서를 모든 process가 동일하게 관측해야 한다는 모델이다. Jepsen은 PRAM을 read your writes, monotonic writes, monotonic reads의 조합과 동등하게 설명한다.[^jepsen-pram]

다만 서로 다른 process가 수행한 write의 순서는 process마다 다르게 볼 수 있다.

---

## 13. Transaction isolation 계열: DB가 어떤 이상 현상을 막는가

DB isolation level도 consistency model의 한 계열이다.

여기서 중요한 질문은:

```text
transaction 안에서 읽고 쓰는 동안 어떤 중간 상태와 이상 현상을 허용할 것인가?
```

이다.

### Read Uncommitted

Read uncommitted는 매우 느슨하다. Jepsen은 Berenson/Adya 계열 해석을 따라 dirty write는 막지만 dirty read는 허용하는 모델로 설명한다.[^jepsen-read-uncommitted]

즉 commit되지 않은 값을 읽을 수 있다.

```text
T1: x=1로 변경, 아직 commit 안 함
T2: x=1을 읽음
T1: rollback
```

그러면 T2는 결국 존재하지 않게 된 값을 본 셈이다.

### Read Committed

Read committed는 dirty read를 막는다.[^jepsen-read-committed]

```text
commit되지 않은 write는 읽지 않는다.
```

하지만 non-repeatable read와 phantom은 허용한다. 같은 transaction 안에서 같은 row를 다시 읽었더니 값이 바뀔 수 있고, 같은 predicate를 다시 조회했더니 row 집합이 달라질 수 있다.

또한 read committed도 real-time order나 per-process order를 보장하지 않는다. 이름만 보고 "commit된 최신값을 읽는다"고 생각하면 안 된다.

### Cursor Stability

Cursor stability는 read committed보다 강하게 lost update를 막으려는 모델이다.[^jepsen-cursor-stability]

transaction이 cursor로 특정 object를 읽고 있는 동안에는 다른 transaction이 그 object를 수정할 수 없다.

```text
T1: x를 읽고 수정 준비
T2: x를 수정해서 commit
T1: 예전 x 기준으로 덮어씀
```

이런 lost update를 막는다. 다만 transaction이 읽은 모든 record를 끝까지 안정적으로 유지하는 repeatable read보다는 약하다.

### Repeatable Read

Repeatable read는 한 transaction 안에서 이미 읽은 개별 object를 다시 읽어도 같은 상태로 보여야 한다는 보장이다.[^jepsen-repeatable-read]

```text
T1: user 1을 읽음
T2: user 1 수정 후 commit
T1: user 1을 다시 읽음
```

T1은 처음 읽은 user 1의 상태를 계속 봐야 한다.

하지만 predicate read에는 phantom이 생길 수 있다.

```text
T1: name = "Dikembe"인 사용자 목록 조회
T2: name = "Dikembe"인 사용자 추가 후 commit
T1: 같은 조건 재조회
```

결과 집합이 달라질 수 있다.

### Snapshot Isolation

Snapshot isolation에서는 각 transaction이 독립적이고 일관된 snapshot 위에서 동작하는 것처럼 보인다.[^jepsen-snapshot-isolation]

transaction의 변경은 commit 전까지 자기 자신에게만 보이고, commit하면 이후 시작하는 transaction들에게 원자적으로 보인다. 같은 객체를 동시에 쓰려고 하면 한쪽은 abort해야 한다.

읽기 성능과 개발자 경험은 좋다. 하지만 serializable과 같지는 않다.

대표적인 허용 현상은 write skew다.

```text
불변식: 당직 의사는 최소 1명 있어야 한다.

T1: 의사 A와 B가 모두 당직인 것을 읽음
T2: 의사 A와 B가 모두 당직인 것을 읽음

T1: A를 당직에서 뺌
T2: B를 당직에서 뺌

둘 다 commit
결과: 당직 의사 0명
```

각 transaction은 자기 snapshot 안에서는 문제가 없어 보였지만, 합쳐진 결과는 불변식을 깨뜨린다.

### Monotonic Atomic View

Monotonic atomic view는 transaction의 효과를 일부만 보는 것을 막는다.[^jepsen-mav]

어떤 transaction T1의 write 하나를 T2가 봤다면, T1의 다른 효과들도 T2에게 보여야 한다.

예를 들어:

```text
T1:
1. orders row 생성
2. order_items row 생성

T2:
orders는 봤는데 order_items는 못 봄
```

이런 부분 관측을 막는 모델이다.

외래키, 인덱스, materialized view 같은 곳에서 중요하다. "transaction은 all-or-nothing"이라는 ACID의 atomicity가 관측 관점에서도 유지되어야 하기 때문이다.

---

## 14. Availability와 consistency는 같이 봐야 한다

강한 모델은 이상 현상을 덜 허용하는 대신, 장애 상황에서 더 많은 coordination을 요구한다.

Jepsen의 지도는 각 모델이 asynchronous network에서 얼마나 available할 수 있는지도 함께 보여준다.[^jepsen-availability]

대략 이렇게 볼 수 있다.

| 모델 | partition 중 처리 가능성 |
|---|---|
| Strong Serializability | total/sticky availability 불가 |
| Linearizability | total/sticky availability 불가 |
| Serializability | total/sticky availability 불가 |
| Sequential Consistency | total/sticky availability 불가 |
| Causal Consistency | sticky availability 가능 |
| PRAM | sticky availability 가능 |
| Read Your Writes | sticky availability 가능 |
| Monotonic Reads | total availability 가능 |
| Monotonic Writes | total availability 가능 |
| Writes Follow Reads | total availability 가능 |
| Read Committed | total availability 가능 |
| Monotonic Atomic View | total availability 가능 |

sticky availability는 client가 같은 server에 계속 붙어 있으면 요청을 처리할 수 있다는 뜻이다. client가 요청마다 다른 server로 이동하면 보장이 깨질 수 있다.

이 지점이 실무에서 중요하다.

예를 들어 read your writes를 제공하려면 다음 중 하나가 필요할 수 있다.

```text
write한 leader로 read 보내기
session stickiness 유지
replication lag가 따라잡을 때까지 기다리기
client에게 version/token을 주고 그 이상을 보장하는 replica에서 읽기
```

아무 replica에서나 읽으면 빠르고 available하지만, 내가 쓴 글이 내 화면에서 사라질 수 있다.

---

## 15. "최신값"이라는 말은 위험하다

일관성 이야기를 할 때 가장 위험한 단어 중 하나가 "최신값"이다.

```text
최신값을 읽나요?
```

라는 질문은 듣기에는 단순하지만 실제로는 애매하다.

```text
누구 기준의 최신인가?
어떤 객체의 최신인가?
write가 완료됐다는 기준은 client 응답인가, leader commit인가, quorum commit인가?
concurrent write가 있으면 무엇이 최신인가?
transaction 여러 개가 동시에 진행 중이면 어떤 snapshot이 최신인가?
```

그래서 더 좋은 질문은 이렇다.

```text
이미 완료된 write를 이후 read가 반드시 봐야 하는가?
같은 사용자의 session order를 보존해야 하는가?
서로 인과관계가 있는 operation의 순서를 보존해야 하는가?
여러 객체 변경이 하나의 transaction처럼 보여야 하는가?
모든 observer가 같은 total order를 봐야 하는가?
```

이렇게 물어야 필요한 모델이 보인다.

---

## 16. 실무에서는 invariant부터 정해야 한다

좋은 설계는 "무조건 강한 일관성"을 고르는 게 아니다.

먼저 깨지면 안 되는 불변식(invariant)을 정해야 한다.

```text
이중 결제는 절대 안 된다.
재고가 음수가 되면 안 된다.
쿠폰은 한 번만 사용되어야 한다.
주문 확정 후 결제 상태가 사라지면 안 된다.
내가 쓴 댓글은 내 화면에는 바로 보여야 한다.
좋아요 수는 몇 초 늦어도 된다.
검색 결과 반영은 1분 늦어도 된다.
analytics는 나중에 맞아도 된다.
```

이걸 정해야 어디에 coordination 비용을 낼지 보인다.

### 강한 보장이 필요한 영역

```text
결제 승인
송금
재고 차감
주문 확정
쿠폰 사용
idempotency key
분산 락
권한 변경
```

이런 곳은 현실이 갈라지면 돈이나 신뢰가 깨진다.

### 약한 보장으로 충분한 영역

```text
좋아요 수
조회수
추천 피드
로그
analytics
검색 인덱스
CDN cache
비핵심 알림
```

몇 초 어긋나도 비즈니스 불변식이 깨지지 않는다면 더 약한 모델을 선택할 수 있다.

핵심은:

```text
강한 일관성이 필요한 곳에만 비싸게 쓴다.
약해도 되는 곳은 의도적으로 약하게 둔다.
```

이다.

---

## 17. 인과율이 깨지면 사용자는 버그라고 느낀다

나는 consistency를:

> 시스템이 인과관계(causality)를 얼마나 보존하려 하는가

로 보는 관점이 가장 본질에 가깝다고 생각한다.

예를 들어:

```text
1. 결제 승인
2. 주문 생성
```

라는 인과가 있다.

그런데 사용자에게:

```text
결제는 완료됨
주문은 없음
```

이 상태가 먼저 보이면 사람은 시스템이 버그났다고 느낀다.

물론 내부적으로는 설명이 있을 수 있다.

```text
payment service는 성공
order service event 처리 지연
read model 반영 전
cache invalidation 지연
search index lag
```

하지만 사용자에게 중요한 건 내부 사정이 아니다. 사용자는 자신이 관측한 세계의 인과율을 본다.

그래서 consistency는 단순 데이터 문제가 아니다.

```text
사용자가 관측하는 세계의 인과율을 어디까지 지켜줄 것인가
```

의 문제다.

---

## 18. 실무에서 문서를 읽는 법

DB나 시스템 문서에서 "strong consistency", "eventual consistency", "serializable", "read committed" 같은 표현을 만나면 이름만 믿으면 안 된다.

대신 이런 질문을 해야 한다.

```text
1. 보장 범위는 무엇인가?
   - key 하나?
   - partition key 하나?
   - table 하나?
   - transaction 전체?
   - database 전체?

2. read는 어디로 가는가?
   - leader?
   - follower?
   - quorum?
   - local replica?
   - cache?

3. write 완료의 의미는 무엇인가?
   - local write?
   - leader append?
   - quorum commit?
   - all replica apply?

4. session guarantee가 있는가?
   - read your writes?
   - monotonic reads?
   - monotonic writes?

5. transaction이 있다면 어떤 anomaly를 허용하는가?
   - dirty read?
   - non-repeatable read?
   - phantom?
   - lost update?
   - write skew?

6. partition이나 replica lag 중에는 어떻게 동작하는가?
   - block?
   - stale read?
   - error?
   - best effort?
```

이 질문에 답할 수 있어야 "이 시스템은 consistent하다"는 말을 제대로 해석할 수 있다.

---

## 19. 흔한 오해

### 오해 1. Consistency는 데이터가 항상 같은 것이다

아니다. consistency는 값이 언제, 누구에게, 어떤 순서로 보이는지에 대한 규칙이다.

### 오해 2. Serializable이면 최신값을 읽는다

아니다. serializability는 transaction들이 어떤 직렬 순서로 보이면 된다. 현실 시간 순서와 session order는 별개다.

### 오해 3. Linearizable이면 모든 DB transaction이 안전하다

아니다. linearizability는 기본적으로 single-object 모델이다. 여러 객체 transaction 전체에 대한 보장은 strong serializability 쪽이다.

### 오해 4. Eventual consistency는 대충 맞는 것이다

아니다. eventual consistency도 계약이다. 잠깐 현실이 갈라지는 것을 허용하지만, 변경이 멈추면 수렴해야 한다.

### 오해 5. 강한 일관성이 항상 좋다

아니다. 강한 일관성은 coordination 비용을 낸다. latency, availability, throughput 비용을 감당할 만큼 중요한 invariant에 써야 한다.

---

## 20. 한 줄로 정리하면

일관성 모델은:

```text
분산된 시스템이 사용자에게 어떤 현실을 보여줄지 정하는 계약
```

이다.

조금 더 정확히는:

```text
operation의 가시성, 순서, 인과관계, 원자성을 어디까지 보장할지 정하는 규칙
```

이다.

강한 일관성은 하나의 현실을 만든다. 약한 일관성은 잠깐 현실이 갈라지는 것을 허용한다. 좋은 설계는 둘 중 하나를 종교처럼 고르는 것이 아니라, 도메인의 invariant를 보고 어디에서 현실을 하나로 묶을지 결정하는 것이다.

그리고 사용자는 평균적인 시스템을 경험하지 않는다. 사용자는 자기가 방금 누른 버튼, 자기가 방금 쓴 글, 자기가 방금 결제한 주문을 경험한다.

그래서 consistency의 마지막 질문은 결국 이것이다.

> 이 시스템은 사용자가 기대하는 인과율을 어디까지 지켜줄 것인가?

[^jepsen-models]: Jepsen, "Consistency Models". Jepsen은 consistency model을 어떤 history들이 "good" 또는 "legal"한지 정의하는 집합으로 설명한다. https://jepsen.io/consistency/models

[^jepsen-history]: Jepsen, "Consistency Models - Fundamental Concepts". Jepsen은 operation의 invocation/completion time, concurrency, history 개념을 먼저 정의한 뒤 consistency model을 설명한다. https://jepsen.io/consistency/models

[^jepsen-linearizable]: Jepsen, "Linearizability". Linearizability는 single-object operation들이 원자적으로 일어난 것처럼 보이고, 그 순서가 real-time ordering과 맞아야 한다. https://jepsen.io/consistency/models/linearizable

[^jepsen-map]: Jepsen, "Consistency Models". 이 글의 모델 관계도는 Jepsen의 consistency model hierarchy 설명을 바탕으로 정리했다. https://jepsen.io/consistency/models

[^jepsen-strong-serializable]: Jepsen, "Strong Serializability". Strong serializability는 transaction들이 serial order로 보이고, 그 순서가 real-time ordering과 호환되어야 한다. https://jepsen.io/consistency/models/strong-serializable

[^jepsen-serializable]: Jepsen, "Serializability". Serializability는 transaction들이 어떤 total order로 실행된 것처럼 보이면 되지만, real-time order나 per-process order를 요구하지 않는다. https://jepsen.io/consistency/models/serializable

[^jepsen-sequential]: Jepsen, "Sequential Consistency". Sequential consistency는 operation들이 어떤 total order로 실행된 것처럼 보이고, 각 process 내부 순서가 그 total order에 보존되어야 한다. https://jepsen.io/consistency/models/sequential

[^jepsen-causal]: Jepsen, "Causal Consistency". Causal consistency는 causally-related operation들이 모든 process에서 같은 순서로 보여야 하지만, causally independent operation의 순서는 달라도 된다고 설명한다. https://jepsen.io/consistency/models/causal

[^jepsen-ryw]: Jepsen, "Read Your Writes". 같은 process가 write 후 read를 수행하면, 그 read는 자신의 write 효과를 관측해야 한다. https://jepsen.io/consistency/models/read-your-writes

[^jepsen-monotonic-reads]: Jepsen, "Monotonic Reads". 같은 process의 read는 이미 관측한 write 이전의 상태로 되돌아가면 안 된다. https://jepsen.io/consistency/models/monotonic-reads

[^jepsen-monotonic-writes]: Jepsen, "Monotonic Writes". 같은 process가 수행한 write들의 순서는 모든 process에서 동일한 순서로 관측되어야 한다. https://jepsen.io/consistency/models/monotonic-writes

[^jepsen-wfr]: Jepsen, "Writes Follow Reads". 어떤 process가 읽은 값에서 비롯된 write 이후에 같은 process가 새 write를 수행하면, 그 write는 앞선 read가 본 write 이후에 보여야 한다. https://jepsen.io/consistency/models/writes-follow-reads

[^jepsen-pram]: Jepsen, "PRAM". PRAM은 같은 process가 수행한 write 순서를 모든 곳에서 보존하며, Jepsen은 이를 read your writes, monotonic writes, monotonic reads의 조합과 동등하게 설명한다. https://jepsen.io/consistency/models/pram

[^jepsen-read-uncommitted]: Jepsen, "Read Uncommitted". Jepsen은 read uncommitted를 dirty write는 금지하지만 dirty read, fuzzy read, phantom은 허용하는 transactional model로 설명한다. https://jepsen.io/consistency/models/read-uncommitted

[^jepsen-read-committed]: Jepsen, "Read Committed". Read committed는 dirty read를 막지만 fuzzy read와 phantom은 허용한다. https://jepsen.io/consistency/models/read-committed

[^jepsen-cursor-stability]: Jepsen, "Cursor Stability". Cursor stability는 cursor로 읽고 있는 object가 cursor release 또는 transaction commit 전까지 다른 transaction에 의해 수정되지 않도록 해 lost update를 막는다. https://jepsen.io/consistency/models/cursor-stability

[^jepsen-repeatable-read]: Jepsen, "Repeatable Read". Repeatable read는 개별 object read는 안정적으로 유지하지만 predicate read에는 phantom을 허용한다. https://jepsen.io/consistency/models/repeatable-read

[^jepsen-snapshot-isolation]: Jepsen, "Snapshot Isolation". Snapshot isolation은 transaction이 독립적이고 일관된 snapshot 위에서 실행되는 것처럼 보이게 하지만 write skew 같은 현상을 허용할 수 있다. https://jepsen.io/consistency/models/snapshot-isolation

[^jepsen-mav]: Jepsen, "Monotonic Atomic View". Monotonic atomic view는 어떤 transaction의 효과 일부를 봤다면 그 transaction의 다른 효과들도 함께 보여야 한다는 atomic visibility를 보장한다. https://jepsen.io/consistency/models/monotonic-atomic-view

[^jepsen-availability]: Jepsen, "Consistency Models". Jepsen은 asynchronous network에서 각 consistency model이 total availability, sticky availability를 가질 수 있는지 색으로 표시한다. https://jepsen.io/consistency/models
