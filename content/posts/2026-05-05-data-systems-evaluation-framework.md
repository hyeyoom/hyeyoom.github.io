+++
title = "데이터 시스템 컴포넌트 평가 프레임워크"
date = "2026-05-05T15:30:50+09:00"
description = "시스템 디자인 컴포넌트를 4축(Invariants / Performance / Workload / Failure)으로 평가하는 멘탈 모델"
math = false
+++

# 데이터 시스템 컴포넌트 평가 프레임워크

> 시스템 디자인을 할 때 컴포넌트(Redis, Postgres, Kafka, S3 …)를 머릿속에서 비교/추산하기 위한 멘탈 모델.
> 임의 나열한 축을 외우는 게 아니라, **세 가지 근원적 제약**으로부터 축이 왜 등장하는지 이해한 뒤, 그 위에서 4축으로 압축하는 방식으로 정리한다.

---

## 0. 한 장 압축판 (cheat sheet, 요약 카드)

**축은 4개다.**

| 축 | 한 줄 정의 | 측정 단위/모델 |
|---|---|---|
| **Invariants (불변 보장)** | 무엇을 *보장*하는가 | consistency 모델 + temporal semantics (staleness/visibility/ordering) + durability 모델 |
| **Performance envelope (성능 한계)** | 그 보장의 *비용* | latency 분포 (p50/p99/p999) + throughput 천장 + capacity + coordination cost (조정 노드 수) |
| **Workload fit (워크로드 적합성)** | 어떤 *모양*에 맞는가 | access pattern (point/range/scan/aggregation) + data shape (KV/relational/document/blob/log) |
| **Failure mode (장애 양상)** | 어떻게 *깨지는가* | failure model + blast radius + backpressure 지점 + recovery RPO/RTO |

**핵심 주장.** Speed/Truth/Scale (속도/진실성/규모) 은 독립축이 아니다. **Invariant를 먼저 고르면, Performance와 Failure는 그 결과로 강제된다.** PACELC가 이걸 정확히 말한다 — 일관성을 강하게 잡으면 latency가 따라온다.

**머릿속 결정 절차 (4단계).**
1. 깨지면 사고가 나는 invariant는 무엇인가? → System of Record (진실 저장소) 후보를 좁힘
2. 워크로드의 모양은? → 저장 구조(B-tree/LSM/columnar/inverted, 즉 B-트리/로그 구조 머지 트리/컬럼형/역색인) 결정
3. 어떤 failure를 견뎌야 하는가? → 복제(replication) 토폴로지와 RPO/RTO 결정
4. p99 latency 예산은? → PACELC의 E를 강제. 1ms 예산이면 cross-region (광역, 리전 간) 동기 합의 불가

**4계층 분류.**
- **System of Record (진실 저장소)** — 진실의 원천 (Postgres, Spanner, ledger, S3 원본)
- **Derived View (파생 뷰)** — 파생/가속 (Redis, ES, materialized view, CDN)
- **Transport (전송 계층)** — 이동/전파 (Kafka, RabbitMQ, CDC, queue)
- **Compute Substrate (연산 계층)** — 읽고 계산 (Spark, Flink, DuckDB, Trino) — 저장도 이동도 아닌 *연산* 계층

**역할은 시스템이 아니라 *데이터*에 붙는다.** 같은 Postgres가 한 테이블은 authoritative (진실, 외부 복구 불가)이고 다른 테이블은 derived (재생성 가능)일 수 있다. 같은 Redis가 어떤 키는 transient cache (휘발, 재생성 불요)이고 다른 키는 rebuildable derived view일 수 있다. 분류는 *데이터의 역할*에 매기지, 시스템 박스 단위가 아니다. 설계 미스의 정체는 거의 항상 "transient/derived여야 할 데이터를 authoritative처럼 다루거나, 그 반대".

---

## 1. 왜 이 축들이 등장하는가 — 세 가지 근원 제약

데이터 시스템의 모든 tradeoff (절충, 트레이드오프) 는 결국 이 셋의 결합이다. 새 컴포넌트를 만나도 이 셋으로 환원해 보면 항상 같은 형태가 보인다.

### 1.1 물리적 제약 — latency 하한

빛의 속도는 광섬유에서 약 200 km/ms다. **이 한계는 어떤 알고리즘으로도 못 깬다.**

대략적 자릿수 (외워둘 가치가 있는 수):

| 동작 | 시간 | 비고 |
|---|---|---|
| L1 cache (1차 캐시) | ~1 ns | |
| DRAM 접근 | ~100 ns | |
| NVMe random read (랜덤 읽기) | ~100 μs | |
| HDD seek (탐색) | ~10 ms | 회전 + seek |
| `fsync` (NVMe + battery-backed cache, 배터리 백업 캐시) | ~100 μs ~ 수 ms | durability의 진짜 비용 |
| 같은 AZ (가용 영역) 네트워크 RTT (왕복 시간) | ~0.5 ms | |
| 같은 region (리전, 광역) 다른 AZ | ~1 ~ 2 ms | |
| US east ↔ west | ~70 ms | 광속 한계 |
| Trans-Atlantic (대서양 횡단) | ~80 ~ 100 ms | |
| Trans-Pacific (태평양 횡단) | ~150 ms | |

**함의.** "강한 일관성을 cross-region에서 단일 ms로" 같은 요구는 *물리적으로* 불가능하다. Spanner가 region 간 commit (커밋) 에 50–100ms를 쓰는 건 게으름이 아니라 광속이다.

### 1.2 정보이론적 제약 — 저장/접근의 tradeoff

- 무작위 데이터는 압축되지 않는다 (Kolmogorov 하한).
- 인덱스는 공간을 먹는다. B+ tree (B+ 트리) 는 키 수에 비례해 공간 비용을 치른다.
- 어떤 자료구조도 **읽기, 쓰기, 메모리** 셋 다를 동시에 최소화할 수 없다 — 이게 RUM Conjecture (RUM 추측, §2.3).

**함의.** "쓰기 빠르고 + 읽기 빠르고 + 공간 적게 쓰는 인덱스"는 없다. 어디 비용을 떠넘길지를 고를 뿐이다. LSM (Log-Structured Merge, 로그 구조 머지) 은 쓰기를 빠르게 하고 그 비용을 compaction (컴팩션, 압축 병합; =읽기/CPU/공간) 으로 떠넘긴다.

### 1.3 분산 시스템 불가능성 — coordination (조정) 비용

- **FLP (Fischer–Lynch–Paterson 1985)** — 비동기 메시지 모델에서 단 한 노드의 fail-stop (정지 후 멈춤) 장애만 있어도 결정론적 합의는 불가능하다. 실용 시스템은 이걸 timeout/randomization/failure detector (타임아웃/무작위화/장애 감지기) 로 우회한다.
- **CAP (Brewer 2000 / Gilbert–Lynch 2002)** — partition (네트워크 분할) 이 발생하면 C와 A를 동시에 보장할 수 없다.
- **Quorum (쿼럼) 정리** — N개 복제본에서 read-after-write (쓰기 후 읽기 일관성) 을 보장하려면 |R| + |W| > N. (Dynamo, Cassandra의 ONE/QUORUM/ALL 옵션의 수학적 기반.)
- **합의의 라운드 수** — Paxos/Raft 정상 경로는 최소 2 message delay (메시지 왕복). 이게 cross-region 강일관 쓰기가 100ms 안에 못 끝나는 이유.

**함의.** Coordination에는 *항상* 비용이 든다. 지연이거나, 가용성이거나, 둘 다거나. "공짜 강일관성"은 광고문구일 뿐이다.

---

## 2. 4축 프레임워크 — 정확히

흔히 제안되는 6축 — Speed / Truth / Scale / Shape / Failure / Ops (속도 / 진실성 / 규모 / 형태 / 장애 / 운영) — 은 실용적이지만 직교성 (orthogonality, 서로 독립인 정도) 이 떨어진다. Speed와 Truth는 PACELC가 말하듯 *서로의 함수*다. Capacity는 Performance envelope의 일부다. Ops cost (운영 비용) 는 대부분 다른 축의 *결과*다. 그래서 다음 4축으로 재정리한다.

### 2.1 Invariants (불변 보장) — 무엇을 보장하는가

이게 **가장 먼저** 결정해야 할 축이다. 다른 축들은 이 결정의 결과를 받는다.

**Consistency (일관성) 모델 (강 → 약).** 정의가 흔히 헷갈리니 정확히:

| 모델 | 정의 | 출처 |
|---|---|---|
| **Strict serializability (엄격 직렬화 가능성)** | Serializable + 실시간 순서 존중. Spanner의 "external consistency (외부 일관성)". | Papadimitriou 1979 + Herlihy/Wing 1990 |
| **Linearizability (선형화 가능성)** | 단일 객체. 각 op (연산) 이 시작과 종료 사이 어느 한 점에서 순간적으로 발생한 것처럼 보임. | Herlihy & Wing, TOPLAS 1990 |
| **Serializability (직렬화 가능성)** | 트랜잭션. 어떤 직렬 순서가 존재. 실시간은 보장 안 함. | Papadimitriou, JACM 1979 |
| **Snapshot Isolation (스냅샷 격리)** | 트랜잭션이 시작 시점 스냅샷을 봄. write skew (쓰기 비뚤어짐) 허용. | Berenson et al., SIGMOD 1995 |
| **Causal consistency (인과 일관성)** | 인과 관계가 있는 op들의 순서만 보존. | Lamport 1978 |
| **Bounded staleness (유계 지연성)** | "최대 X초 / X 버전 뒤떨어짐"을 약속. | Cosmos DB, Spanner stale read |
| **Eventual consistency (최종 일관성)** | 충분히 시간이 흐르면 수렴. | Vogels, CACM 2009 |

**Linearizability ≠ Serializability**임에 주의. 전자는 단일 객체의 실시간 순서, 후자는 트랜잭션의 직렬화 가능성. 둘을 모두 보장하면 strict serializability.

**Temporal semantics (시간 차원).** 같은 "consistency" 라벨이라도 시간 차원이 다르면 *완전히 다른 시스템*이다. consistency 모델은 "어떤 보장"을 정의하지만, temporal semantics는 "*얼마나 오래된* 값을 받을 수 있는지"를 정한다.

- *Read freshness / staleness bound (읽기 신선도, 유계 지연성)* — 읽기가 얼마나 오래된 값을 볼 수 있는가. ES refresh interval ~1초, Redis 복제본 lag ms~s, Cassandra eventual은 무한대 가능 (anti-entropy repair 전까지).
- *Write visibility delay (쓰기 가시성 지연)* — 쓰기가 다른 reader에게 보이기까지의 시간. Spanner는 commit 후 즉시, MongoDB `readConcern: majority`는 majority commit 후, S3는 PUT 이후 read-after-write, Kafka는 ISR 복제 완료 후.
- *Ordering guarantees (순서 보장)* — Kafka는 partition *내부* 순서, partition 간 순서는 미보장. RDB는 commit 순서. CRDTs는 순서 자체에 의존 안 함 (가환 연산).

이게 왜 본질적이냐: **Kafka는 ordering이 있지만 "현재 상태"가 없다.** 같은 "eventual"이라도 staleness bound가 100ms vs 10초면 운영적으로 완전히 다른 결정을 낳는다.

**Durability (내구성) 모델.** "내구성 있다"가 무슨 가정 아래인지 확인:

- *Single node fsync (단일 노드 동기화)* (SQLite, Postgres 단일 노드) — 디스크 죽으면 끝
- *Sync replica fsync (동기 복제본 동기화)* (Postgres `synchronous_commit=on` + 동기 복제) — N대 동시 손실 견딤
- *Quorum durable (쿼럼 내구성)* (Cassandra W=QUORUM, Spanner) — 쿼럼 손실 시까지 안전
- *Multi-AZ erasure coded (다중 AZ + 소실 부호화)* (S3, DynamoDB) — region 내에서는 사실상 무손실
- *Multi-region (다중 리전)* (Spanner, Aurora Global) — region 손실에도 RPO≈0 가능

핵심 질문: **"이 컴포넌트가 죽었을 때 데이터가 사라져도 되는가?"** 답이 NO면 그건 System of Record 후보다. 그 외 계층(Derived/Transport/Compute)은 재생성 가능해야 한다.

### 2.2 Performance envelope (성능 한계) — 그 보장의 비용

세 가지 하위 차원:

**(a) Latency 분포.** 평균이 아니라 **분포**로 본다. Dean & Barroso (CACM 2013, "The Tail at Scale (꼬리 지연 문제)")의 핵심 메시지: fan-out (팬아웃, 다수 호출 분산) 시스템에서는 **꼬리 지연 (tail latency) 이 시스템 지연을 지배한다**. 100개로 fan-out하면 시스템 latency ≈ 컴포넌트의 p99. 1000개면 p999.

→ 컴포넌트를 평가할 때 *반드시* p50, p99, p999 (백분위 50/99/99.9) 를 따로 본다. p50만 빠른 컴포넌트는 함정이다.

**(b) Throughput 천장.** **Little's Law (리틀의 법칙)**: L = λ·W. concurrency (동시 처리 수) = arrival rate (도착률) × residence time (잔류 시간). 의미:
- 100 connection 풀 + 요청당 10ms = 10,000 req/s
- 목표 10k req/s + 100 worker = 요청당 10ms 이내 끝나야 함
- λ가 천장에 가까워질수록 **W가 폭발한다** (큐잉 이론 (queueing theory), M/M/1: W = 1/(μ−λ))

이게 "사용률 80% 넘으면 latency 무너진다"의 수학적 근거다.

**스케일링.** Amdahl을 일반화한 **USL (Universal Scalability Law, 보편적 확장 법칙; Gunther 2007)**:

  C(N) = N / (1 + α(N−1) + β·N(N−1))

- α = 직렬화 계수 (공유 자원 contention, 경합)
- β = coherency (정합성) 계수 (노드 간 cross-talk (상호 간섭), 캐시 동기화 등)
- β > 0이면 어떤 N* 이후 처리량이 *감소*한다. "노드 더 넣었더니 더 느려짐"의 정체.

**(c) Capacity (용량).** 단일 숫자가 아님:
- *Working set (작업 집합)* — 메모리에 올라가야 할 hot (자주 접근하는 핫) 데이터 크기
- *Total dataset (전체 데이터셋)* — 디스크/네트워크에 보관되는 cold (드물게 접근하는 콜드) 까지 포함
- *IOPS vs bandwidth (초당 IO 횟수 vs 대역폭)* — random small ops (작은 무작위 연산) 한계와 sequential MB/s (순차 처리량) 한계는 다른 숫자

Redis가 "용량이 작다"는 건 working set이 RAM에 묶인다는 뜻이지, 1TB Redis가 불가능하다는 뜻이 아니다.

**(d) Coordination cost (조정 비용).** 한 op이 *몇 개 노드의 합의*를 필요로 하는가? 이게 latency 하한을 *직접* 결정한다 — PACELC의 E를 산출하는 메커니즘.

| 시스템 | 한 쓰기당 노드 수 | 동기/비동기 | 결과 latency 하한 |
|---|---|---|---|
| Redis 단일 | 1 | — | 메모리 접근만 (~100μs) |
| Postgres 단일 + WAL fsync | 1 + 디스크 | sync | fsync 비용 (~수 ms) |
| Postgres + 동기 복제 1대 | 2 | sync | + 같은 AZ RTT (~1ms) |
| Cassandra W=QUORUM (N=3) | 2/3 응답 대기 | sync | 가장 느린 1/2의 latency |
| Spanner cross-region commit | 다중 region Paxos | sync | 50–100ms (Paxos round + TrueTime wait) |
| Kafka `acks=all` (RF=3) | ISR 전체 ack | sync | replica RTT max |

**판단 도구.** 새 컴포넌트를 만나면 *바로* 묻는 질문: "한 쓰기에 몇 노드가 *동기적으로* 관여하는가? 그 노드들 사이 RTT는?" 이 두 숫자의 곱이 latency floor다. 어떤 최적화도 이 floor 아래로 못 내려간다 — 광속이 거부한다 (§1.1).

### 2.3 Workload fit (워크로드 적합성) — 어떤 모양에 맞는가

**RUM Conjecture (Athanassoulis et al., EDBT 2016).** 자료구조 설계의 근본 trilemma (삼중 딜레마): **R**ead overhead (읽기 오버헤드), **U**pdate overhead (갱신 오버헤드), **M**emory overhead (메모리 오버헤드) 셋을 동시에 최소화할 수 없다. 둘을 잡으면 하나를 포기.

| 자료구조 | R | U | M | 잘 맞는 패턴 |
|---|---|---|---|---|
| B+ tree | 낮음 | 중간 | 중간 | point + range (점 조회 + 범위 조회) |
| LSM tree | 중간 (compaction tail, 컴팩션 꼬리) | 낮음 | 낮음 (write-amp (쓰기 증폭) 는 큼) | write-heavy (쓰기 위주) + range |
| Hash index (해시 인덱스) | 낮음 (point) | 낮음 | 중간 | point만 |
| Inverted index (역색인) | 중간 | 높음 (rebuild, 재색인) | 높음 | full-text (전문 검색), set membership (집합 포함 여부) |
| Columnar (컬럼형, e.g., Parquet) | 낮음 (스캔/agg, 집계) | 매우 높음 (immutable, 불변) | 낮음 (압축 잘 됨) | analytical scan/agg (분석 스캔/집계) |
| Bitmap index (비트맵 인덱스) | 낮음 (set ops, 집합 연산) | 높음 | 낮음 (cardinality (고유값 수) 낮을 때) | low-cardinality filtering (낮은 카디널리티 필터링) |

**Access pattern (접근 패턴) 분류:**
- *Point lookup (단건 조회)* — 키로 한 건 (모든 인덱스 OK)
- *Range scan (범위 스캔)* — 정렬된 구간 (B-tree, LSM)
- *Full scan / aggregation (전체 스캔/집계)* — 컬럼 단위 통계 (columnar)
- *Search / matching (검색/매칭)* — 텍스트, 다차원 (inverted, R-tree, vector)
- *Append-only log (추가 전용 로그)* — 추가만 (Kafka, WAL)

**Data shape (데이터 형태):**
- *Key-Value (키-값)* (Redis, DynamoDB, Riak)
- *Wide-column (와이드 컬럼)* (Cassandra, HBase, BigTable)
- *Document (문서)* (Mongo, Couch)
- *Relational (관계형)* (Postgres, MySQL, Spanner)
- *Graph (그래프)* (Neo4j, Dgraph)
- *Search (검색)* (Elasticsearch, Solr)
- *Blob (블롭, 대용량 이진 객체)* (S3, GCS)
- *Log/Stream (로그/스트림)* (Kafka, Pulsar)
- *Columnar OLAP (컬럼형 분석)* (ClickHouse, Druid, BigQuery, DuckDB)

### 2.4 Failure mode (장애 양상) — 어떻게 깨지는가

**Failure model (장애 모델) 가정.** 시스템이 어떤 장애를 견딘다고 *주장*하는지 확인:

| 모델 | 의미 |
|---|---|
| Crash-stop (정지 후 멈춤) | 노드는 멈출 뿐, 거짓 메시지 안 보냄 |
| Crash-recovery (정지 후 복구) | 멈췄다가 깨어나서 상태 복구 |
| Omission (누락) | 메시지 분실 가능 |
| Byzantine (비잔틴) | 임의의 거짓 동작 (블록체인) |

**거의 모든 상용 데이터 시스템은 crash-recovery + omission을 가정한다.** Byzantine 가정은 비싸다.

**Failure correlation (장애 상관성).** 실제로 가장 위험한 가정 위반은 *독립* 장애 가정이 깨지는 경우다:
- 같은 rack (랙) 의 노드는 동시 손실 가능
- 같은 AZ는 정전/네트워크로 함께 죽음
- 같은 region은 대규모 장애로 함께 죽음
- 같은 software version (소프트웨어 버전) 은 같은 버그로 함께 죽음 (특히 무서움)

**Blast radius (영향 반경).** 컴포넌트 하나 죽었을 때 *다른* 무엇이 영향받는가?
- Redis 죽으면 → cache miss (캐시 미스) 로 origin (원본 서버) 폭격 → 연쇄 장애 (thundering herd, 천둥 양 떼)
- DB primary (주 노드) 죽으면 → write 정지, 그 동안 queue 적재 → 복구 시 부하 폭주

**RPO / RTO.**
- RPO (Recovery Point Objective, 복구 시점 목표) = 잃을 수 있는 데이터 양 (시간 단위)
- RTO (Recovery Time Objective, 복구 시간 목표) = 복구까지 걸리는 시간

이 두 숫자가 곧 가용성 SLA (서비스 수준 협약) 의 진짜 모습이다. "5 nines (5개의 9, 즉 99.999%)"는 마케팅, RPO/RTO가 엔지니어링이다.

**Harvest (수확) / Yield (산출) (Fox & Brewer, HotOS 1999).** CAP의 0/1 선택을 부드럽게 한 뷰:
- **Yield** = 응답한 요청의 비율 (= 전통적 availability)
- **Harvest** = 응답에 반영된 데이터의 비율 (예: 검색이 95%의 shard (샤드) 만 반환)

장애 시 "거절"이 아니라 "부분 응답"으로 graceful degradation (우아한 성능 저하) 을 설계할 수 있다. 검색 엔진과 추천 시스템이 자주 이 모델을 쓴다.

**Backpressure (배압) — 어디서 막히는가.** 분산 시스템은 *직접* 죽는 일이 드물다. 거의 항상 **backpressure 붕괴 → cascade failure (연쇄 장애)** 의 형태로 죽는다. 컴포넌트마다 *1차 병목*이 다르고, 그 병목이 차오르면 압력이 *어디로 흘러가는지*가 곧 blast radius다.

| 컴포넌트 | 1차 backpressure 지점 | 붕괴 양상 | 압력의 출구 |
|---|---|---|---|
| Redis | memory pressure, single-thread CPU | OOM, eviction 폭주, command timeout | client → origin DB 직격 (cache stampede) |
| Postgres | connection pool, lock contention, WAL disk | connection 거부, lock wait timeout | upstream queue 적재 |
| Kafka | partition 처리량, consumer lag, retention | lag 증가 → retention 초과 시 영구 손실 | 상류 producer 지연 또는 disk full |
| S3 | per-prefix RPS, 5xx throttle | exponential backoff 안 하면 cascade | client retry storm |
| Elasticsearch | heap, fielddata, shard rebalancing | OOM, GC stop-the-world, query rejection | indexing pipeline 적재 |
| CDN | cache miss → origin RPS | hit ratio 붕괴 시 origin 폭격 | origin 직접 부하 |
| Spanner | split throughput, hot row | tablet split, latency spike | commit 대기열 |

**설계 시 핵심 질문.** "이 컴포넌트의 큐가 차오르면, 압력은 *어디로* 흘러가는가?" 이 질문에 답할 수 있어야 isolation (격리), bulkhead (격벽), circuit breaker (회로 차단기), backpressure-aware queue 같은 방어 패턴을 *어디에* 둘지 결정 가능하다. backpressure가 어디로 흐르는지 모르면, 작은 장애가 *시스템 전체* 장애로 번진다.

---

## 3. 고전 결과 — 정확히 외우기

CAP "셋 중 하나 포기"처럼 두루뭉술한 정리는 실무 판단에 도움이 안 된다. 정확한 statement (정확한 명제) 으로 내장하자.

### 3.1 CAP — Brewer 2000 / Gilbert–Lynch 2002

**정확한 statement (Gilbert & Lynch 2002).** 비동기 네트워크에서 다음 셋을 동시에 보장하는 분산 데이터 객체는 존재하지 않는다:
- **Consistency** = atomic/linearizable consistency (원자적/선형화 가능 일관성)
- **Availability (가용성)** = 죽지 않은 노드는 모든 요청에 응답
- **Partition tolerance (분할 내성)** = 임의의 메시지 손실에도 시스템이 정상 동작 시도

**흔한 오해 3가지.**
1. "셋 중 둘만 고르라" — 틀림. P는 *현실*이지 선택지가 아님 (네트워크는 partition된다). 실제 선택은 *partition 발생 시* C와 A 사이. partition 외에는 둘 다 가능.
2. "C는 일반적인 의미의 일관성" — 틀림. CAP의 C는 **linearizability** (가장 강한 단일객체 모델)이다. snapshot isolation, eventual 등 다른 의미의 "일관성"은 CAP의 영역 밖.
3. "A는 SLA 가용성" — 틀림. CAP의 A는 **모든 살아있는 노드가 응답**해야 함. 일부 노드 응답하지 않아도 SLA OK인 시스템은 CAP의 A를 위반하지만 실용적으로는 가용함.

CAP은 *임팩트 있는 단순화*지만 실무 도구로는 거칠다. 다음의 PACELC가 더 유용하다.

### 3.2 PACELC — Abadi 2012

**Abadi, "Consistency Tradeoffs in Modern Distributed Database System Design (현대 분산 데이터베이스 설계의 일관성 트레이드오프)", IEEE Computer, 2012.**

> If Partition (P, 분할 발생 시), choose between Availability (A) and Consistency (C); Else (E, 평상시), choose between Latency (L) and Consistency (C).

CAP의 진정한 확장. 핵심 통찰: **partition이 없을 때도 일관성과 지연은 trade된다.** 일관성을 강하게 잡으려면 더 많은 노드와의 합의를 기다려야 하니까.

대표 분류:

| 시스템 | PACELC | 의미 |
|---|---|---|
| Dynamo, Cassandra, Riak | PA / EL | partition 시 A 우선, 평상시 latency 우선 (eventual) |
| Spanner, FaunaDB | PC / EC | 항상 C 우선 (latency 비용 감수) |
| Postgres (single node, 단일 노드) | — | partition 무관 |
| MongoDB (default, 기본 설정) | PA / EC | partition 시 A, 평상시 C (튜닝 가능) |
| BigTable, HBase | PC / EC | C 우선 |

**실무적 함의.** "이 DB는 일관성이 강한가?"가 아니라 "**평상시 어떤 latency를 받아들일 준비가 됐는가?**"가 진짜 질문이다. p99 1ms 예산이라면 cross-region 합의는 불가능하니 EL 시스템밖에 못 쓴다.

### 3.3 RUM Conjecture (RUM 추측) — Athanassoulis et al. EDBT 2016

**"Designing Access Methods: The RUM Conjecture (접근 방식 설계: RUM 추측)", EDBT 2016.**

> Read overhead, Update overhead, Memory overhead 셋을 동시에 최소화하는 접근 방법은 없다.

이론 (conjecture (추측), 미증명)이지만 강력한 디자인 가이드. 새 인덱스/스토리지 엔진이 등장하면 *어떤 두 개에 강하고 어디에 비용을 떠넘기는지* 묻는 게 첫 질문.

예시:
- **B+ tree (Postgres, MySQL)** — R 좋고 U/M 중간. 정렬된 트리 유지 비용.
- **LSM tree (RocksDB, Cassandra, ScyllaDB)** — U 좋고 (sequential append, 순차 추가), R/M에 비용. compaction이 R 꼬리를 만들고 write amplification (쓰기 증폭) 이 M에 영향.
- **Hash table (해시 테이블)** — R 좋고 (point) U도 좋지만 range scan 불가.
- **Fractal tree / Bε-tree (프랙탈 트리/Bε-트리)** — B-tree와 LSM 사이 균형 (TokuDB).
- **Learned index (학습 기반 인덱스, Kraska et al. 2018)** — M을 줄이려고 모델로 인덱스 대체. 분포가 안정적이어야 동작.

### 3.4 Little's Law (리틀의 법칙) — Little 1961

> L = λ · W
> (system 내 평균 요청 수) = (도착률) × (평균 잔류 시간)

가장 단순하지만 가장 자주 쓰는 식. throughput 추산의 산수.

**활용 예.**
- DB 커넥션 풀 100개, 평균 쿼리 10ms → 최대 10,000 qps (queries per second, 초당 쿼리 수)
- 목표 5,000 qps, p50 4ms → 평균 동시 in-flight (처리 중) 20개. 풀 100이면 충분, 10이면 부족
- Kafka consumer (소비자) N대, 메시지당 처리 50ms → throughput = N / 0.05

**확장.** 큐잉 이론 M/M/1에서 평균 잔류 시간 W = 1/(μ−λ). 사용률 ρ = λ/μ가 1에 가까워지면 W는 폭발적으로 증가한다 (hyperbolic, 쌍곡선적으로). 이게 "DB CPU 80% 넘어가면 latency가 무너진다"의 수학적 정체.

### 3.5 USL (Universal Scalability Law, 보편적 확장 법칙) — Gunther 2007

  C(N) = N / (1 + α(N−1) + β·N(N−1))

- α = contention (공유 자원 직렬화 비율)
- β = coherency (노드 간 동기화 cross-talk)

**핵심 인사이트.** β > 0 이면 어떤 *최적 노드 수* N* = √((1−α)/β) 가 존재하고, 그 이상에서는 throughput이 *감소*한다 (retrograde scaling, 역행 확장).

**실무 적용.** 부하 테스트 결과 (N, throughput) 점들을 USL로 fit (적합) 하면 α, β가 나옴. β가 0에 가깝지 않으면 아키텍처에 본질적 cross-talk가 있다는 신호 (분산 락 (distributed lock), 글로벌 카운터, 캐시 invalidation (무효화) 폭주 등). 더 많이 넣어서 해결이 안 되는 클래스의 문제.

### 3.6 Tail at Scale (대규모에서의 꼬리 지연) — Dean & Barroso, CACM 2013

> "The Tail at Scale", Communications of the ACM 56(2), 2013.

핵심 한 문장: **fan-out 아키텍처에서는 시스템 latency가 평균이 아닌 꼬리(tail, 분포의 끝부분)에 의해 지배된다.**

수학. 컴포넌트 한 개의 p99이 10ms일 때, 100개로 fan-out한 요청의 시스템 latency는 *최소 한 개라도 10ms 이상 걸릴 확률* = 1 − 0.99^100 ≈ 63%. 즉 **p99이었던 게 p50이 된다.**

**완화 기법 (논문에서 제시된 것):**
- *Hedged requests (헤지 요청)* — 두 복제본에 동시 요청, 빠른 쪽 사용
- *Tied requests (묶인 요청)* — 두 복제본에 보내되 한쪽이 시작하면 다른 쪽 취소
- *Micro-partitioning (마이크로 파티셔닝)* — 파티션을 더 잘게 쪼개 핫스팟 (hot spot, 부하 집중점) 을 흩뿌림
- *Selective replication (선택적 복제)* — 인기 데이터 복제 늘려서 부하 분산

**실무 함의.** 마이크로서비스/검색/추천처럼 fan-out 패턴이 있는 곳에서는 컴포넌트 p50 최적화가 거의 무의미하다. **p99, p999가 진짜 지표다.**

### 3.7 Harvest & Yield (수확 & 산출) — Fox & Brewer, HotOS 1999

CAP의 0/1 선택을 연속적으로:
- **Yield (산출)** = 완료된 요청의 비율 (`completed / total`)
- **Harvest (수확)** = 응답에 반영된 데이터의 비율 (`returned / available`)

검색 엔진에서 1000개 shard 중 950개만 응답해도 결과를 *부분적으로* 반환하면 Yield = 100%, Harvest = 95%. 이게 CAP을 거부하지 않으면서도 사용자 경험을 살리는 방식.

**적용 가능 여부 판단.** 결제, 재고 차감, ledger entry (원장 기입) 같은 *모든-아니면-아무것도-아닌* 작업은 Harvest 트레이드 못 함. 검색, 추천, 통계, 로그 분석은 자주 가능.

---

## 4. 컴포넌트 카드 — 4축 정량 평가

각 카드는 [Invariants / Performance / Workload / Failure] 4축으로. 숫자는 *대략적 자릿수* — 정확한 값은 워크로드/하드웨어/설정 의존이지만 멘탈 모델로는 자릿수 감각이 더 중요.

---

### Redis

- **Invariants.** 단일 키 op는 단일 스레드라 linearizable (primary (주 노드) 기준). 복제는 **비동기 기본** → replica (복제본) 는 stale (오래된 값) 가능. Durability는 옵션: RDB (주기 snapshot (스냅샷)), AOF (Append-Only File, 추가 전용 파일; `appendfsync everysec` ≈ 최대 1초 손실, `always` ≈ 매 쓰기 fsync로 ms-scale 비용). **Cluster mode (클러스터 모드) 는 cross-shard (샤드 간) 트랜잭션 미지원.**
- **Performance.** p50 50–200μs (single node, in-RAM, 단일 노드 메모리 내). p99 ~1ms 단일, persistence (영속화) 켜면 fsync에 따라 튐. throughput 100k+ ops/s/core (코어당 초당 10만+ 연산). capacity = RAM (cluster로 수평 확장 시 TB급 가능, 단 비쌈).
- **Workload.** point KV, atomic counter (`INCR`, 원자적 카운터), sorted set leaderboard (정렬 집합 리더보드), pub/sub (발행/구독), set membership, simple Lua. **약함**: 복잡 query, range scan over large dataset (대용량 데이터에 대한 범위 스캔), secondary index on values (값에 대한 보조 인덱스).
- **Failure.** master-replica failover (주-복제 페일오버, 장애 조치) ~수초 (Sentinel/Cluster). 비동기 복제로 인한 **데이터 손실 가능 (window: 복제 lag, 복제 지연 구간)**. memory pressure (메모리 압박) 시 eviction policy (축출 정책; LRU/LFU) 가 데이터를 *조용히* 버린다 — SSoT (Single Source of Truth, 단일 진실 원천) 로 쓰면 사고 난다.

**한 줄.** 빠른 휘발성 acceleration (가속) 계층. 쓰면 안 되는 곳: ledger (원장), primary 주문 데이터.

---

### Postgres

- **Invariants.** 트랜잭션 SERIALIZABLE 가능 (SSI (Serializable Snapshot Isolation, 직렬화 가능 스냅샷 격리), Cahill et al. 2008 — true serializability (진정한 직렬화 가능성) 를 MVCC (Multi-Version Concurrency Control, 다중 버전 동시성 제어) 위에서). 기본은 READ COMMITTED (커밋 된 것만 읽기). WAL (Write-Ahead Log, 미리 쓰기 로그) fsync로 단일 노드 durability, 동기 복제 (`synchronous_commit=remote_apply`)로 멀티노드 durability. **Logical replication (논리적 복제)** 으로 cross-version/cross-DB (버전 간/DB 간) 가능.
- **Performance.** OLTP (Online Transaction Processing, 온라인 트랜잭션 처리) p50 1–5ms, p99 10–50ms. throughput 단일 노드 5k–30k tps (transactions per second, 초당 트랜잭션) (워크로드/HW에 따라 크게 다름). 수직 스케일링 (vertical scaling) 이 기본, 수평 (horizontal) 은 Citus/Patroni/sharding (샤딩) 직접.
- **Workload.** 관계, 트랜잭션, JOIN (조인), 다양한 인덱스 (B-tree, GIN, GiST, BRIN). JSONB 지원. **약함**: 쓰기 처리량 매우 큰 워크로드 (단일 master 한계), 페타 스케일.
- **Failure.** streaming replication (스트리밍 복제) + Patroni/Pacemaker로 자동 failover 가능 (수초~분). long-running transaction (장시간 트랜잭션) 이 vacuum (가비지 정리) 을 막아 bloat (테이블 비대화) 유발 (운영 함정). connection 폭주 시 fork (프로세스 생성) 비용 → pgbouncer 필수.

**한 줄.** SoR 후보 0순위. 수직 스케일링 천장에 닿기 전까지는 거의 항상 정답.

---

### Cassandra

- **Invariants.** **Tunable consistency (조정 가능 일관성)** — `ONE/QUORUM/ALL`, R+W>N이면 read-after-write. 기본 eventual. AP 시스템 (PACELC: PA/EL). row-level (행 수준) 만 atomic, multi-row (다중 행) 트랜잭션 없음 (LWT (Lightweight Transaction, 경량 트랜잭션) 는 비싸고 권장 안 함).
- **Performance.** 쓰기 매우 빠름 (LSM, log-structured (로그 구조화)): p50 ms 자릿수. 읽기는 QUORUM에서 p99 꼬리 길어짐 (compaction, repair (수리)). **선형에 가까운 수평 확장** (수백 노드 검증). capacity는 본질적 제약 없음.
- **Workload.** 시계열 (time-series), write-heavy, primary key based access (기본 키 기반 접근; partition key (파티션 키) + clustering key (정렬 키) 설계가 핵심). **약함**: ad-hoc query (즉석 질의), JOIN, secondary index (가능은 하지만 느림), 페이징 (paging).
- **Failure.** gossip (가십 프로토콜) 기반, 노드 다수 손실 견딤. partition 시 AP. 운영 난이도 높음 (compaction, repair, tombstone (묘비, 삭제 마커)).

**한 줄.** 쓰기 폭주 + key 기반 access + 수평 확장 필수일 때. SQL 비슷해 보이지만 NoSQL 마인드 필수.

---

### DynamoDB

- **Invariants.** Eventually consistent reads (기본) / Strongly consistent reads (1.0× RU (Read Capacity Unit, 읽기 용량 단위) 비용). **Transactions** (TransactWrite/TransactGet) 지원하나 25개 항목 제한, 비용 2×. multi-region active-active (다중 리전 양방향) 는 *eventual*만.
- **Performance.** p99 single-digit ms in-region (한 자리 ms, 같은 리전 내). throughput 프로비저닝 (provisioning, 사전 할당) / 온디맨드 (on-demand, 사용량 기반). **hot partition (특정 파티션 부하 집중) 주의** — 단일 partition key 천장 ~1000 WCU (Write Capacity Unit) / 3000 RCU.
- **Workload.** KV / 단순 secondary index (GSI (Global Secondary Index, 전역 보조 인덱스) /LSI (Local Secondary Index, 지역 보조 인덱스)). 단일 키 access 패턴이면 무한에 가까운 확장. **약함**: ad-hoc query, full-text, 분석.
- **Failure.** 매니지드 (managed, 관리형), 3 AZ 복제, 99.999% SLA (글로벌 테이블). 추상화 잘 돼서 직접 운영 부담 거의 없음. 비용 모델이 함정 — scan은 비싸고, hot partition은 throttle (제한, 스로틀).

**한 줄.** AWS-native KV의 표준. 액세스 패턴이 명확할 때 강력.

---

### Spanner

- **Invariants.** **External consistency (외부 일관성) = strict serializability**, globally (전역적으로). TrueTime API (GPS + atomic clock (원자 시계) 의 시간 불확실성 ε)로 commit timestamp (커밋 타임스탬프) 결정. multi-region 트랜잭션 가능.
- **Performance.** 단일 region 읽기 ~5–10ms. multi-region write commit ~50–100ms (Paxos 라운드 + TrueTime wait (대기)). throughput은 split (분할 단위) 단위로 수평 확장. capacity는 사실상 무한 (Google이 EB (엑사바이트) scale 운영).
- **Workload.** 글로벌 SQL OLTP. interleaved tables (인터리브 테이블, 부모-자식 데이터 인접 저장) 로 join 지역성 (locality) 확보. **약함**: write가 매우 빈번한 단일 row (split 안에 묶임), latency-critical sub-ms (지연 민감, 1ms 미만) 워크로드.
- **Failure.** Paxos group (Paxos 그룹) 으로 region 손실까지 견딤 (multi-region config (구성)). RPO ≈ 0, RTO 수초~수분.

**한 줄.** 글로벌 강일관 SQL이 필요한 드문 케이스의 정답. latency 비용 감수 가능할 때만.

---

### Kafka

- **Invariants.** **Per-partition ordered log (파티션별 순서 보장 로그)**. Cross-partition (파티션 간) 순서 *보장 없음*. durability는 `acks` (acknowledgement, 확인) 설정 (0/1/all) + ISR (in-sync replicas, 동기화된 복제본) 수에 따름. `acks=all` + `min.insync.replicas=2` + `replication.factor=3`이 표준 안전 설정. exactly-once (정확히 한 번) 는 idempotent producer (멱등 생산자) + transactions로 *내부* 가능, end-to-end (종단 간) 는 consumer 멱등성 (idempotency) 이 필요.
- **Performance.** producer (생산자) p99 ~10ms (`acks=all`, in-region). 처리량 broker (브로커) 당 수십 MB/s ~ GB/s (크기/배치/압축에 따라). consumer는 pull (당겨가기) 기반.
- **Workload.** event log (이벤트 로그), stream, fan-out, replay (재생). **약함**: random key access, 현재 상태 query (그건 KTable/materialized view (구체화 뷰) 로 풀어야 함), 작은 메시지 매우 많을 때 metadata 오버헤드.
- **Failure.** ISR 줄어들면 가용성 vs 일관성 trade (`unclean.leader.election`, 동기화 안 된 리더 선출 설정). consumer lag (소비자 지연) 이 사일런트 장애 (silent failure, 조용한 장애) 의 주범. retention (보존 기간) 만료 시 데이터 영구 손실.

**한 줄.** "현재 상태"가 아닌 "변화의 흐름"을 다루는 시스템의 척추. 저장소처럼 보이지만 시간순 로그다.

---

### S3 (그리고 호환 object storage, 객체 저장소)

- **Invariants.** 2020년 12월부터 **strong read-after-write consistency (강한 쓰기 후 읽기 일관성)** (PUT 후 즉시 GET 가능). 11 nines durability (10⁻¹¹ 손실 확률) — 다중 AZ erasure coding (소실 부호화). object는 immutable (불변; PUT은 새 버전).
- **Performance.** GET first-byte latency (첫 바이트 도착 지연) ~수십 ms. throughput per-prefix (접두사 경로별) ~5,500 RPS (Requests Per Second, 초당 요청 수; GET) / 3,500 RPS (PUT). 무제한 capacity. Glacier는 cold (콜드 스토리지; 분~시간 단위 retrieve (회수)).
- **Workload.** blob, 백업, 정적 자원, data lake (데이터 레이크; Parquet 위에 Athena/Spark). **약함**: hot small-key 워크로드, append (multipart upload (다중 파트 업로드) 는 됨), 진짜 random write (object 단위만).
- **Failure.** 매니지드, region 단위 outage (장애) 는 드물지만 발생. cross-region replication (CRR, 리전 간 복제) 으로 DR (Disaster Recovery, 재해 복구) 가능.

**한 줄.** 가장 저렴한 무한 durable storage. 데이터 레이크와 백업의 기본기.

---

### Elasticsearch

- **Invariants.** **Near real-time (준실시간)** (refresh interval (재색인 주기) 기본 1초). 같은 shard 내에선 eventual. translog (트랜잭션 로그) 로 durability. Quorum-style write (쿼럼 방식 쓰기, `wait_for_active_shards`). split-brain (분할 뇌, 클러스터 분열) 은 zen2 / 7.x 이후 완화.
- **Performance.** query 10–100ms (복잡도 의존). bulk indexing (대량 색인) 처리량 좋음. aggregation은 cardinality와 shard 수에 강하게 좌우.
- **Workload.** full-text 검색, 로그/observability (관측성), 다차원 aggregation, geo (지리 정보), vector (recent, 최근 벡터 검색 추가). **약함**: SoR (재색인 가능해야 함), 복잡 트랜잭션, 강일관성.
- **Failure.** shard rebalancing (샤드 재분배) 이 부하 spike (급증) 유발. 메모리 압박 (heap (힙 메모리), fielddata (필드 데이터 캐시)) 이 가장 흔한 운영 이슈. version 호환성 깐깐.

**한 줄.** 검색 DB지 진실 DB가 아니다. SoR을 따로 두고 색인은 derived view로.

---

### CDN (Content Delivery Network, 콘텐츠 전송 네트워크; CloudFront, Fastly, Cloudflare 등)

- **Invariants.** TTL (Time To Live, 만료 시간) 기반 eventual. immutable object identity (불변 객체 식별자; URL + ETag (개체 태그)). stale-while-revalidate (재검증 중 오래된 값 반환) 으로 가용성 우선.
- **Performance.** edge (엣지, 사용자 인접 노드) p50 single-digit ms (사용자 가까이). cache miss 시 origin shield (오리진 보호 캐시 계층) 거쳐 origin (원본 서버) RTT.
- **Workload.** static (정적), cacheable (캐시 가능), immutable. API GET 응답도 적절한 TTL이면 가능. **약함**: 개인화, 실시간, write.
- **Failure.** stale 응답으로 graceful degradation. origin 장애도 cache hit ratio (캐시 적중률) 만큼은 무관.

**한 줄.** 사용자 가까이 있는 *빠른 거짓말쟁이*. 빠르지만 진실 반영이 느리다.

---

### SQLite

- **Invariants.** ACID, single-writer serializable (단일 작성자 직렬화 가능; WAL 모드 포함). file-as-DB (파일 자체가 DB).
- **Performance.** 로컬 in-process (같은 프로세스 내), μs 자릿수. throughput는 single-writer 천장.
- **Workload.** embedded (임베디드), 단일 앱, 작은~중간 데이터셋, 분석/보고서 (CTE (Common Table Expression, 공통 테이블 표현), window 함수 잘 지원). **약함**: 동시 writer, 네트워크 access (LiteFS/Cloudflare D1 같은 wrapper (래퍼) 로 보강 가능).
- **Failure.** 파일 손상이 주된 모드. corruption (손상) 은 매우 드물지만 발생 가능 → 백업 필수.

**한 줄.** "단일 프로세스에서 충분"이라는 조건만 맞으면 거의 항상 옳은 선택. Hipp이 만든 가장 많이 deploy된 DB.

---

### DuckDB

- **Invariants.** ACID 단일 프로세스. columnar (열 기반), vectorized execution (벡터화 실행). Parquet/CSV/Arrow를 native (네이티브) 로 read.
- **Performance.** 단일 노드 GB/s 스캔. analytical (분석형) 쿼리 초~수십 초.
- **Workload.** ad-hoc analytics (즉석 분석), embedded analytics (임베디드 분석), 데이터 사이언스. **약함**: OLTP, 동시 writer, 분산.
- **Failure.** 프로세스 로컬, 디스크 손상이 주.

**한 줄.** "분석용 SQLite". S3 + Parquet + DuckDB는 작은 팀의 무서운 데이터 스택.

---

## 5. 결정 트리 — 4단계 압축 절차

새 워크로드 / 새 컴포넌트 선택 시 머릿속에서 돌리는 절차.

### Step 1 — Invariants 먼저

> *"이게 깨지면 사고 나는 보장은 무엇인가?"*

- **Linearizability 필요?** → Spanner, Postgres (단일 노드), etcd, ZooKeeper. Cassandra/DynamoDB(eventual) 탈락.
- **트랜잭션 (multi-row, 다중 행)?** → SQL 계열, Spanner, FoundationDB. NoSQL 다수 탈락.
- **Durability 사고 발생 시 데이터 손실 허용 X?** → SoR로 designate (지정). Redis/ES/CDN은 후보 아님.
- **부분 손실 OK (cache, search index, log analytics, 즉 캐시/검색 색인/로그 분석)?** → Derived view 후보로 자유롭게.

이 단계에서 후보 50%가 빠진다.

### Step 2 — Workload shape (워크로드 형태)

> *"이 데이터/쿼리는 어떤 모양인가?"*

- *Point KV* → KV store (Redis, DynamoDB, Mongo, Postgres OK)
- *Range scan + 정렬* → B-tree / LSM (Postgres, Cassandra, MySQL)
- *Full-text / fuzzy (전문 검색/유사 일치)* → ES, OpenSearch, Vespa
- *Aggregation over many rows (다행 집계)* → Columnar (ClickHouse, Druid, BigQuery, DuckDB)
- *Append-only event log* → Kafka, Pulsar
- *Blob* → S3
- *Graph traversal (그래프 순회)* → Neo4j, Dgraph, JanusGraph
- *Vector similarity (벡터 유사도)* → Faiss/HNSW, pgvector, Qdrant, Weaviate

### Step 3 — Failure 모델

> *"어떤 장애를 견뎌야 하는가? RPO/RTO는?"*

- *단일 노드*면 충분 → SQLite, single Postgres
- *AZ 손실*까지 → multi-AZ (RDS Multi-AZ, DynamoDB, Spanner zonal config (영역 구성))
- *Region 손실*까지 → multi-region (S3 CRR, Spanner multi-region, Aurora Global, Cassandra multi-DC (다중 데이터센터))
- *Software bug correlation (소프트웨어 버그 상관성)* 까지 → 다른 software/version으로 backup. (실전에서 무시되지만 가장 위험한 카테고리.)

### Step 4 — Latency 예산

> *"p99이 얼마면 사용자가 행복한가? (혹은 SLA를 충족하는가?)"*

이 숫자가 PACELC의 E를 강제한다.

| p99 예산 | 함의 |
|---|---|
| < 1 ms | 로컬 메모리. Redis on same host (같은 호스트), in-process cache (같은 프로세스 내 캐시), SQLite, embedded. **다른 host로 가면 끝.** |
| < 10 ms | same-AZ 동기 복제. 보통의 OLTP. cross-AZ는 빠듯. |
| < 100 ms | cross-AZ 동기 OK. cross-region 읽기 OK (write는 빠듯). |
| < 1 s | cross-region 동기 가능 (Spanner). 사람 user-perceivable (체감 가능) 한계. |
| > 1 s | 분석/배치 (batch). consistency vs latency 부담 거의 없음. |

이 단계에서 "강일관성을 cross-region에서 sub-10ms로" 같은 비현실적 요구가 걸러진다 — 광속이 거부한다.

---

## 6. 자주 쓰는 한 줄 요약

> 컴포넌트를 처음 볼 때 던지는 질문은 단 하나.
>
> **"이건 진실로 믿을 건가, 빠르게 보여줄 건가, 이동시킬 건가, 계산할 건가?"**
>
> 그리고 분류한 뒤,
>
> **"이게 깨졌을 때 데이터가 사라져도 되는가?"**
>
> 답이 NO이면 SoR이고, YES이면 derived/transport/compute다. **SoR이 아닌 것을 SoR로 쓰면 사고가 난다.**

---

## 부록 — 참고 문헌

- Brewer, E. *Towards Robust Distributed Systems*. PODC 2000 keynote.
- Gilbert, S., & Lynch, N. *Brewer's Conjecture and the Feasibility of Consistent, Available, Partition-Tolerant Web Services*. ACM SIGACT News 33(2), 2002.
- Abadi, D. *Consistency Tradeoffs in Modern Distributed Database System Design: CAP is Only Part of the Story*. IEEE Computer 45(2), 2012.
- Athanassoulis, M. et al. *Designing Access Methods: The RUM Conjecture*. EDBT 2016.
- Fischer, M., Lynch, N., & Paterson, M. *Impossibility of Distributed Consensus with One Faulty Process*. JACM 32(2), 1985.
- Herlihy, M., & Wing, J. *Linearizability: A Correctness Condition for Concurrent Objects*. ACM TOPLAS 12(3), 1990.
- Lamport, L. *Time, Clocks, and the Ordering of Events in a Distributed System*. CACM 21(7), 1978.
- Berenson, H. et al. *A Critique of ANSI SQL Isolation Levels*. SIGMOD 1995.
- Cahill, M., Röhm, U., & Fekete, A. *Serializable Isolation for Snapshot Databases*. SIGMOD 2008.
- Vogels, W. *Eventually Consistent*. CACM 52(1), 2009.
- Dean, J., & Barroso, L. A. *The Tail at Scale*. CACM 56(2), 2013.
- Fox, A., & Brewer, E. *Harvest, Yield, and Scalable Tolerant Systems*. HotOS-VII, 1999.
- Gunther, N. *Guerrilla Capacity Planning*. Springer, 2007. (USL 식)
- Little, J. D. C. *A Proof for the Queuing Formula L = λW*. Operations Research 9(3), 1961.
- Kleppmann, M. *Designing Data-Intensive Applications*. O'Reilly, 2017. (위 결과들의 실무적 통합 정리)
- Bailis, P., Davidson, A., Fekete, A., Ghodsi, A., Hellerstein, J., Stoica, I. *Highly Available Transactions: Virtues and Limitations*. VLDB 2014.
- Corbett, J. C. et al. *Spanner: Google's Globally Distributed Database*. OSDI 2012.
- DeCandia, G. et al. *Dynamo: Amazon's Highly Available Key-Value Store*. SOSP 2007.
- Kraska, T. et al. *The Case for Learned Index Structures*. SIGMOD 2018.

---

*마지막 한 마디.* 이 문서가 가장 잘 작동하는 사용법: 새 컴포넌트를 만나면 §4의 카드 형식으로 직접 한 장 채워보라. 채울 수 없는 칸이 있으면 그 컴포넌트에 대해 *모른다*는 뜻이다. 그 빈칸이 곧 다음 공부 거리다.
