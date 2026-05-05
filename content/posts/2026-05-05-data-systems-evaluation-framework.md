+++
title = "데이터 시스템 컴포넌트 평가 프레임워크"
date = "2026-05-05T15:30:50+09:00"
description = "시스템 디자인 컴포넌트를 4축(Invariants / Performance / Workload / Failure)으로 평가하는 멘탈 모델"
math = true
+++

# 데이터 시스템 컴포넌트 평가 프레임워크

> 시스템 디자인을 할 때 컴포넌트(Redis, Postgres, Kafka, S3 …)를 머릿속에서 비교/추산하기 위한 멘탈 모델.
> 임의 나열한 축을 외우는 게 아니라, **세 가지 근원적 제약**으로부터 축이 왜 등장하는지 이해한 뒤, 그 위에서 4축으로 압축하는 방식으로 정리한다.

---

## 0. 한 장 압축판 (cheat sheet)

**축은 4개다.**

| 축 | 한 줄 정의 | 측정 단위/모델 |
|---|---|---|
| **Invariants** | 무엇을 *보장*하는가 | consistency 모델 + temporal semantics (staleness/visibility/ordering) + durability 모델 |
| **Performance envelope** | 그 보장의 *비용* | latency 분포 (p50/p99/p999) + throughput 천장 + capacity + coordination cost |
| **Workload fit** | 어떤 *모양*에 맞는가 | access pattern (point/range/scan/aggregation) + data shape (KV/relational/document/blob/log) |
| **Failure mode** | 어떻게 *깨지는가* | failure model + blast radius + backpressure 지점 + recovery RPO/RTO |

**핵심 주장.** Speed/Truth/Scale은 독립축이 아니다. **Invariant를 먼저 고르면, Performance와 Failure의 모양도 상당 부분 정해진다.** PACELC가 이걸 정확히 말한다 — consistency를 강하게 잡으면 latency가 따라온다.

**머릿속 결정 절차 (4단계).**
1. 깨지면 사고가 나는 invariant는 무엇인가? → System of Record 후보를 좁힘
2. 워크로드의 모양은? → 저장 구조(B-tree/LSM/columnar/inverted index) 결정
3. 어떤 failure를 견뎌야 하는가? → 복제(replication) 토폴로지와 RPO/RTO 결정
4. p99 latency 예산은? → PACELC의 E 선택을 결정. 1ms 예산이면 cross-region 동기 합의 불가

**4계층 분류.**
- **System of Record** — 진실의 원천 (Postgres, Spanner, ledger, S3 원본)
- **Derived View** — 파생/가속 (Redis, ES, materialized view, CDN)
- **Transport** — 이동/전파 (Kafka, RabbitMQ, CDC, queue)
- **Compute Substrate** — 읽고 계산 (Spark, Flink, DuckDB, Trino) — 저장도 이동도 아닌 *연산* 계층

**역할은 시스템이 아니라 *데이터*에 붙는다.** 같은 Postgres가 한 테이블은 authoritative이고 다른 테이블은 derived일 수 있다. 같은 Redis가 어떤 키는 transient cache이고 다른 키는 rebuildable derived view일 수 있다. 분류는 *데이터의 역할*에 매기지, 시스템 박스 단위가 아니다. 설계 미스의 정체는 거의 항상 "transient/derived여야 할 데이터를 authoritative처럼 다루거나, 그 반대".

---

## 1. 왜 이 축들이 등장하는가 — 세 가지 근원 제약

데이터 시스템의 모든 tradeoff는 결국 이 셋의 결합이다. 새 컴포넌트를 만나도 이 셋으로 환원해 보면 항상 같은 형태가 보인다.

### 1.1 물리적 제약 — latency 하한

빛의 속도는 광섬유에서 약 200 km/ms다. **이 한계는 어떤 알고리즘으로도 못 깬다.**

대략적 자릿수:

| 동작 | 시간 | 비고 |
|---|---|---|
| L1 cache | ~1 ns | |
| DRAM 접근 | ~100 ns | |
| NVMe random read | ~100 μs | |
| HDD seek | ~10 ms | 회전 + seek |
| `fsync` (NVMe + battery-backed cache) | ~100 μs ~ 수 ms | durability의 진짜 비용 |
| 같은 AZ 네트워크 RTT | ~0.5 ms | |
| 같은 region 다른 AZ | ~1 ~ 2 ms | |
| US east ↔ west | ~70 ms | 광속 한계 |
| Trans-Atlantic | ~80 ~ 100 ms | |
| Trans-Pacific | ~150 ms | |

**함의.** "강한 consistency를 cross-region에서 단일 ms로" 같은 요구는 *물리적으로* 불가능하다. Spanner가 region 간 commit에 50–100ms를 쓰는 건 게으름이 아니라 광속이다.

### 1.2 정보이론적 제약 — 저장/접근의 tradeoff

- 무작위 데이터는 압축되지 않는다 (Kolmogorov 하한).
- 인덱스는 공간을 먹는다. B+ tree는 키 수에 비례해 공간 비용을 치른다.
- 어떤 자료구조도 **읽기, 쓰기, 메모리** 셋 다를 동시에 최소화할 수 없다 — 이것이 RUM Conjecture (§2.3).

**함의.** "쓰기 빠르고 + 읽기 빠르고 + 공간 적게 쓰는 인덱스"는 없다. 어디 비용을 떠넘길지를 고를 뿐이다. LSM은 쓰기를 빠르게 하고 그 비용을 compaction(읽기/CPU/공간)으로 떠넘긴다.

### 1.3 분산 시스템 불가능성 — coordination 비용

- **FLP (Fischer–Lynch–Paterson 1985)** — 비동기 메시지 모델에서 단 한 노드의 fail-stop 장애만 있어도 결정론적 합의는 불가능하다. 실용 시스템은 이걸 timeout/randomization/failure detector로 우회한다.
- **CAP (Brewer 2000 / Gilbert–Lynch 2002)** — partition이 발생하면 C와 A를 동시에 보장할 수 없다.
- **Quorum 정리** — $N$개 replica에서 read-after-write를 보장하려면 $|R| + |W| > N$. Dynamo, Cassandra의 ONE/QUORUM/ALL 옵션을 이해하는 기본 모델이다.
- **합의의 라운드 수** — Paxos/Raft 정상 경로는 최소 2 message delay. 그래서 cross-region 강일관 쓰기는 100ms 안에 끝나기 어렵다.

**함의.** Coordination에는 *항상* 비용이 든다. 지연이거나, 가용성이거나, 둘 다거나. "공짜 강일관성"은 광고문구일 뿐이다.

---

## 2. 4축 프레임워크 — 정의

흔히 쓰는 6축 — Speed / Truth / Scale / Shape / Failure / Ops — 은 기억하기 쉽지만 서로 꽤 겹친다. Speed는 Truth를 얼마나 강하게 잡느냐에 따라 달라지고, Scale은 대개 Performance envelope 안에서 드러난다. Ops cost도 독립된 속성이라기보다 invariant, workload, failure 선택의 결과에 가깝다. 그래서 이 글에서는 컴포넌트를 다음 4축으로 압축한다.

### 2.1 Invariants — 무엇을 보장하는가

**가장 먼저** 결정해야 할 축이다. 다른 축들은 이 결정의 결과를 받는다.

**Consistency 모델 (강 → 약).** 정의가 흔히 헷갈리니 정확히:

| 모델 | 정의 | 출처 |
|---|---|---|
| **Strict serializability** | Serializable + 실시간 순서 존중. Spanner의 "external consistency". | Papadimitriou 1979 + Herlihy/Wing 1990 |
| **Linearizability** | 단일 객체. 각 op이 시작과 종료 사이 어느 한 점에서 순간적으로 발생한 것처럼 보임. | Herlihy & Wing, TOPLAS 1990 |
| **Serializability** | 트랜잭션. 어떤 직렬 순서가 존재. 실시간은 보장 안 함. | Papadimitriou, JACM 1979 |
| **Snapshot Isolation** | 트랜잭션이 시작 시점 snapshot을 봄. write skew 허용. | Berenson et al., SIGMOD 1995 |
| **Causal consistency** | 인과 관계가 있는 op들의 순서만 보존. | Lamport 1978 |
| **Bounded staleness** | "최대 X초 / X 버전 뒤떨어짐"을 약속. | Cosmos DB, Spanner stale read |
| **Eventual consistency** | 충분히 시간이 흐르면 수렴. | Vogels, CACM 2009 |

**Linearizability ≠ Serializability**임에 주의. 전자는 단일 객체의 실시간 순서, 후자는 트랜잭션의 직렬화 가능성. 둘을 모두 보장하면 strict serializability.

**Temporal semantics.** 같은 "consistency" 라벨이라도 시간 차원이 다르면 *완전히 다른 시스템*이다. consistency 모델은 "어떤 보장"을 정의하지만, temporal semantics는 "*얼마나 오래된* 값을 받을 수 있는지"를 정한다.

- *Read freshness / staleness bound* — 읽기가 얼마나 오래된 값을 볼 수 있는가. ES refresh interval ~1초, Redis replica lag ms~s, Cassandra eventual은 anti-entropy repair 전까지 무한대 가능.
- *Write visibility delay* — 쓰기가 다른 reader에게 보이기까지의 시간. Spanner는 commit 후 즉시, MongoDB `readConcern: majority`는 majority commit 후, S3는 PUT 이후 read-after-write, Kafka는 ISR 복제 완료 후.
- *Ordering guarantees* — Kafka는 partition *내부* 순서, partition 간 순서는 미보장. RDB는 commit 순서. CRDTs는 순서 자체에 의존 안 함.

왜 이 구분이 본질적이냐면, **Kafka는 ordering이 있지만 "현재 상태"가 없다.** 같은 "eventual"이라도 staleness bound가 100ms vs 10초면 운영적으로 완전히 다른 결정을 낳는다.

**Durability 모델.** "내구성 있다"가 무슨 가정 아래인지 확인:

- *Single node fsync* (SQLite, Postgres 단일 노드) — 디스크 죽으면 끝
- *Sync replica fsync* (Postgres `synchronous_commit=on` + 동기 복제) — N대 동시 손실 견딤
- *Quorum durable* (Cassandra W=QUORUM, Spanner) — quorum 손실 시까지 안전
- *Multi-AZ erasure coded* (S3, DynamoDB) — region 내에서는 사실상 무손실
- *Multi-region* (Spanner, Aurora Global) — region 손실에도 RPO≈0 가능

핵심 질문: **"이 컴포넌트가 죽었을 때 데이터가 사라져도 되는가?"** 답이 NO면 그건 System of Record 후보다. 그 외 계층(Derived/Transport/Compute)은 재생성 가능해야 한다.

### 2.2 Performance envelope — 그 보장의 비용

세 가지 하위 차원:

**(a) Latency 분포.** 평균이 아니라 **분포**로 본다. Dean & Barroso의 "The Tail at Scale" 핵심 메시지: fan-out 시스템에서는 **tail latency가 시스템 latency를 지배한다**. 100개로 fan-out하면 시스템 latency ≈ 컴포넌트의 p99. 1000개면 p999.

→ 컴포넌트를 평가할 때 *반드시* p50, p99, p999를 따로 본다. p50만 빠른 컴포넌트는 함정이다.

**(b) Throughput 천장.** **Little's Law**:

$$
L = \lambda W
$$

즉, concurrency = arrival rate × residence time. 의미:
- 100 connection 풀 + 요청당 10ms = 10,000 req/s
- 목표 10k req/s + 100 worker = 요청당 10ms 이내 끝나야 함
- $\lambda$가 천장에 가까워질수록 **$W$가 폭발한다**. M/M/1 queue에서는:

$$
W = \frac{1}{\mu - \lambda}
$$

이것이 "사용률 80% 넘으면 latency 무너진다"의 수학적 근거다.

**스케일링.** Amdahl을 일반화한 **USL (Universal Scalability Law; Gunther 2007)**:

$$
C(N) = \frac{N}{1 + \alpha(N - 1) + \beta N(N - 1)}
$$

- $\alpha$ = 직렬화 계수 (공유 자원 contention)
- $\beta$ = coherency 계수 (노드 간 cross-talk, 캐시 동기화 등)
- $\beta > 0$이면 어떤 $N^*$ 이후 처리량이 *감소*한다. "노드 더 넣었더니 더 느려짐"의 정체.

**(c) Capacity.** 단일 숫자가 아님:
- *Working set* — 메모리에 올라가야 할 hot 데이터 크기
- *Total dataset* — 디스크/네트워크에 보관되는 cold 데이터까지 포함
- *IOPS vs bandwidth* — random small ops 한계와 sequential MB/s 한계는 다른 숫자

Redis가 "용량이 작다"는 건 working set이 RAM에 묶인다는 뜻이지, 1TB Redis가 불가능하다는 뜻이 아니다.

**(d) Coordination cost.** 한 op이 *몇 개 노드의 합의*를 필요로 하는가? 이 값이 latency 하한을 *직접* 결정한다 — PACELC의 E를 산출하는 메커니즘이다.

| 시스템 | 한 쓰기당 노드 수 | 동기/비동기 | 결과 latency 하한 |
|---|---|---|---|
| Redis 단일 | 1 | — | 메모리 접근만 (~100μs) |
| Postgres 단일 + WAL fsync | 1 + 디스크 | sync | fsync 비용 (~수 ms) |
| Postgres + 동기 복제 1대 | 2 | sync | + 같은 AZ RTT (~1ms) |
| Cassandra W=QUORUM (N=3) | 2/3 응답 대기 | sync | 가장 느린 1/2의 latency |
| Spanner cross-region commit | 다중 region Paxos | sync | 50–100ms (Paxos round + TrueTime wait) |
| Kafka `acks=all` (RF=3) | ISR 전체 ack | sync | replica RTT max |

**판단 질문.** 새 컴포넌트를 만나면 바로 묻는다: "한 쓰기에 몇 노드가 *동기적으로* 관여하는가? 그 노드들 사이 RTT는?" 이 두 숫자가 latency floor를 거의 결정한다. 어떤 최적화도 이 floor 아래로는 못 내려간다 — 광속이 거부한다 (§1.1).

### 2.3 Workload fit — 어떤 모양에 맞는가

**RUM Conjecture (Athanassoulis et al., EDBT 2016).** 자료구조 설계의 근본 trilemma: **R**ead overhead, **U**pdate overhead, **M**emory overhead 셋을 동시에 최소화할 수 없다. 둘을 잡으면 하나를 포기.

| 자료구조 | R | U | M | 잘 맞는 패턴 |
|---|---|---|---|---|
| B+ tree | 낮음 | 중간 | 중간 | point + range |
| LSM tree | 중간 (compaction tail) | 낮음 | 낮음 (write amplification은 큼) | write-heavy + range |
| Hash index | 낮음 (point) | 낮음 | 중간 | point만 |
| Inverted index | 중간 | 높음 (rebuild/reindex) | 높음 | full-text, set membership |
| Columnar (e.g., Parquet) | 낮음 (scan/agg) | 매우 높음 (immutable) | 낮음 (압축 잘 됨) | analytical scan/agg |
| Bitmap index | 낮음 (set ops) | 높음 | 낮음 (low-cardinality일 때) | low-cardinality filtering |

**Access pattern 분류:**
- *Point lookup* — 키로 한 건 (모든 인덱스 OK)
- *Range scan* — 정렬된 구간 (B-tree, LSM)
- *Full scan / aggregation* — 컬럼 단위 통계 (columnar)
- *Search / matching* — 텍스트, 다차원 (inverted, R-tree, vector)
- *Append-only log* — 추가만 (Kafka, WAL)

**Data shape:**
- *Key-Value* (Redis, DynamoDB, Riak)
- *Wide-column* (Cassandra, HBase, BigTable)
- *Document* (Mongo, Couch)
- *Relational* (Postgres, MySQL, Spanner)
- *Graph* (Neo4j, Dgraph)
- *Search* (Elasticsearch, Solr)
- *Blob* (S3, GCS)
- *Log/Stream* (Kafka, Pulsar)
- *Columnar OLAP* (ClickHouse, Druid, BigQuery, DuckDB)

### 2.4 Failure mode — 어떻게 깨지는가

**Failure model 가정.** 시스템이 어떤 장애를 견딘다고 *주장*하는지 확인:

| 모델 | 의미 |
|---|---|
| Crash-stop | 노드는 멈출 뿐, 거짓 메시지 안 보냄 |
| Crash-recovery | 멈췄다가 깨어나서 상태 복구 |
| Omission | 메시지 분실 가능 |
| Byzantine | 임의의 거짓 동작 |

**거의 모든 상용 데이터 시스템은 crash-recovery + omission을 가정한다.** Byzantine 가정은 비싸다.

**Failure correlation.** 실제로 가장 위험한 가정 위반은 *독립* 장애 가정이 깨지는 경우다:
- 같은 rack의 노드는 동시 손실 가능
- 같은 AZ는 정전/네트워크로 함께 죽음
- 같은 region은 대규모 장애로 함께 죽음
- 같은 software version은 같은 버그로 함께 죽음 (특히 무서움)

**Blast radius.** 컴포넌트 하나 죽었을 때 *다른* 무엇이 영향받는가?
- Redis 죽으면 → cache miss로 origin 폭격 → thundering herd
- DB primary 죽으면 → write 정지, 그 동안 queue 적재 → 복구 시 부하 폭주

**RPO / RTO.**
- RPO (Recovery Point Objective) = 잃을 수 있는 데이터 양 (시간 단위)
- RTO (Recovery Time Objective) = 복구까지 걸리는 시간

이 두 숫자가 곧 availability SLA의 진짜 모습이다. "5 nines"는 마케팅, RPO/RTO가 엔지니어링이다.

**Harvest / Yield (Fox & Brewer, HotOS 1999).** CAP의 0/1 선택을 부드럽게 한 뷰:
- **Yield** = 응답한 요청의 비율 (= 전통적 availability)
- **Harvest** = 응답에 반영된 데이터의 비율 (예: 검색이 95%의 shard만 반환)

장애 시 "거절"이 아니라 "부분 응답"으로 graceful degradation을 설계할 수 있다. 검색 엔진과 추천 시스템이 자주 이 모델을 쓴다.

**Backpressure — 어디서 막히는가.** 분산 시스템은 *직접* 죽는 일이 드물다. 거의 항상 **backpressure 붕괴 → cascade failure**의 형태로 죽는다. 컴포넌트마다 *1차 병목*이 다르고, 그 병목이 차오르면 압력이 *어디로 흘러가는지*가 곧 blast radius다.

| 컴포넌트 | 1차 backpressure 지점 | 붕괴 양상 | 압력의 출구 |
|---|---|---|---|
| Redis | memory pressure, single-thread CPU | OOM, eviction 폭주, command timeout | client → origin DB 직격 |
| Postgres | connection pool, lock contention, WAL disk | connection 거부, lock wait timeout | upstream queue 적재 |
| Kafka | partition 처리량, consumer lag, retention | lag 증가 → retention 초과 시 영구 손실 | 상류 producer 지연 또는 disk full |
| S3 | per-prefix RPS, 5xx throttle | exponential backoff 안 하면 cascade | client retry storm |
| Elasticsearch | heap, fielddata, shard rebalancing | OOM, GC stop-the-world, query rejection | indexing pipeline 적재 |
| CDN | cache miss → origin RPS | hit ratio 붕괴 시 origin 폭격 | origin 직접 부하 |
| Spanner | split throughput, hot row | tablet split, latency spike | commit 대기열 |

**설계 시 핵심 질문.** "이 컴포넌트의 큐가 차오르면, 압력은 *어디로* 흘러가는가?" 이 질문에 답할 수 있어야 isolation, bulkhead, circuit breaker, backpressure-aware queue 같은 방어 패턴을 *어디에* 둘지 결정 가능하다. backpressure가 어디로 흐르는지 모르면, 작은 장애가 *시스템 전체* 장애로 번진다.

---

## 3. 고전 결과 — 정확히 기억하기

CAP "셋 중 하나 포기"처럼 두루뭉술한 정리는 실무 판단에 도움이 안 된다. 정확한 statement로 기억하자.

### 3.1 CAP — Brewer 2000 / Gilbert–Lynch 2002

**정확한 statement (Gilbert & Lynch 2002).** 비동기 네트워크에서 다음 셋을 동시에 보장하는 분산 데이터 객체는 존재하지 않는다:
- **Consistency** = atomic/linearizable consistency
- **Availability** = 죽지 않은 노드는 모든 요청에 응답
- **Partition tolerance** = 임의의 메시지 손실에도 시스템이 정상 동작 시도

**흔한 오해 3가지.**
1. "셋 중 둘만 고르라" — 틀림. P는 *현실*이지 선택지가 아님. 실제 선택은 *partition 발생 시* C와 A 사이. partition 외에는 둘 다 가능.
2. "C는 일반적인 의미의 consistency" — 틀림. CAP의 C는 **linearizability**다. snapshot isolation, eventual 등 다른 의미의 "일관성"은 CAP의 영역 밖.
3. "A는 SLA 가용성" — 틀림. CAP의 A는 **모든 살아있는 노드가 응답**해야 함. 일부 노드 응답하지 않아도 SLA OK인 시스템은 CAP의 A를 위반하지만 실용적으로는 가용함.

CAP은 *임팩트 있는 단순화*지만 실무 도구로는 거칠다. 다음의 PACELC가 더 유용하다.

### 3.2 PACELC — Abadi 2012

**Abadi, "Consistency Tradeoffs in Modern Distributed Database System Design", IEEE Computer, 2012.**

> If Partition (P), choose between Availability (A) and Consistency (C); Else (E), choose between Latency (L) and Consistency (C).

CAP의 진정한 확장이다. 핵심 통찰: **partition이 없을 때도 consistency와 latency는 trade된다.** consistency를 강하게 잡으려면 더 많은 노드와의 합의를 기다려야 하니까.

대표 분류:

| 시스템 | PACELC | 의미 |
|---|---|---|
| Dynamo, Cassandra, Riak | PA / EL | partition 시 A 우선, 평상시 latency 우선 (eventual) |
| Spanner, FaunaDB | PC / EC | 항상 C 우선 (latency 비용 감수) |
| Postgres (single node) | — | partition 무관 |
| MongoDB (default) | PA / EC | partition 시 A, 평상시 C (튜닝 가능) |
| BigTable, HBase | PC / EC | C 우선 |

**실무적 함의.** "이 DB는 일관성이 강한가?"가 아니라 "**평상시 어떤 latency를 받아들일 준비가 됐는가?**"가 진짜 질문이다. p99 1ms 예산이라면 cross-region 합의는 불가능하니 EL 시스템밖에 못 쓴다.

### 3.3 RUM Conjecture — Athanassoulis et al. EDBT 2016

**"Designing Access Methods: The RUM Conjecture", EDBT 2016.**

> Read overhead, Update overhead, Memory overhead 셋을 동시에 최소화하는 접근 방법은 없다.

증명된 정리는 아니지만 강력한 디자인 가이드다. 새 인덱스/스토리지 엔진이 등장하면 *어떤 두 개에 강하고 어디에 비용을 떠넘기는지* 묻는 게 첫 질문.

예시:
- **B+ tree (Postgres, MySQL)** — R 좋고 U/M 중간. 정렬된 트리 유지 비용.
- **LSM tree (RocksDB, Cassandra, ScyllaDB)** — U 좋고, R/M에 비용. compaction이 R 꼬리를 만들고 write amplification이 M에 영향.
- **Hash table** — R 좋고(point) U도 좋지만 range scan 불가.
- **Fractal tree / Bε-tree** — B-tree와 LSM 사이 균형 (TokuDB).
- **Learned index (Kraska et al. 2018)** — M을 줄이려고 모델로 인덱스 대체. 분포가 안정적이어야 동작.

### 3.4 Little's Law — Little 1961

$$
L = \lambda W
$$

여기서 $L$은 system 내 평균 요청 수, $\lambda$는 arrival rate, $W$는 평균 residence time이다.

가장 단순하지만 가장 자주 쓰는 식. throughput 추산의 기본 산수다.

**활용 예.**
- DB 커넥션 풀 100개, 평균 쿼리 10ms → 최대 10,000 qps
- 목표 5,000 qps, p50 4ms → 평균 동시 in-flight 20개. 풀 100이면 충분, 10이면 부족
- Kafka consumer N대, 메시지당 처리 50ms → throughput = N / 0.05

**확장.** queueing theory M/M/1에서 평균 잔류 시간은:

$$
W = \frac{1}{\mu - \lambda}
$$

사용률은:

$$
\rho = \frac{\lambda}{\mu}
$$

$\rho$가 1에 가까워지면 $W$는 쌍곡선처럼 치솟는다. 이것이 "DB CPU 80% 넘어가면 latency가 무너진다"의 수학적 정체.

### 3.5 USL (Universal Scalability Law) — Gunther 2007

$$
C(N) = \frac{N}{1 + \alpha(N - 1) + \beta N(N - 1)}
$$

- $\alpha$ = contention (공유 자원 직렬화 비율)
- $\beta$ = coherency (노드 간 동기화 cross-talk)

**핵심 인사이트.** $\beta > 0$이면 어떤 *최적 노드 수*가 존재한다:

$$
N^* = \sqrt{\frac{1 - \alpha}{\beta}}
$$

그 이상에서는 throughput이 *감소*한다 (retrograde scaling).

**실무 적용.** 부하 테스트 결과 $(N, throughput)$ 점들을 USL로 fit하면 $\alpha$, $\beta$가 나옴. $\beta$가 0에 가깝지 않으면 아키텍처에 본질적 cross-talk가 있다는 신호 (distributed lock, 글로벌 카운터, cache invalidation 폭주 등). 더 많이 넣어서 해결이 안 되는 클래스의 문제.

### 3.6 Tail at Scale — Dean & Barroso, CACM 2013

> "The Tail at Scale", Communications of the ACM 56(2), 2013.

핵심 한 문장: **fan-out 아키텍처에서는 시스템 latency가 평균이 아닌 tail에 의해 지배된다.**

수학. 컴포넌트 한 개의 p99이 10ms일 때, 100개로 fan-out한 요청의 시스템 latency는 최소 한 개라도 10ms 이상 걸릴 확률에 지배된다:

$$
1 - 0.99^{100} \approx 0.63
$$

즉 **p99이었던 게 p50이 된다.**

**완화 기법 (논문에서 제시된 것):**
- *Hedged requests* — 두 복제본에 동시 요청, 빠른 쪽 사용
- *Tied requests* — 두 복제본에 보내되 한쪽이 시작하면 다른 쪽 취소
- *Micro-partitioning* — partition을 더 잘게 쪼개 hot spot을 흩뿌림
- *Selective replication* — 인기 데이터 복제 늘려서 부하 분산

**실무 함의.** 마이크로서비스/검색/추천처럼 fan-out 패턴이 있는 곳에서는 컴포넌트 p50 최적화가 거의 무의미하다. **p99, p999가 진짜 지표다.**

### 3.7 Harvest & Yield — Fox & Brewer, HotOS 1999

CAP의 0/1 선택을 연속적으로:
- **Yield** = 완료된 요청의 비율 (`completed / total`)
- **Harvest** = 응답에 반영된 데이터의 비율 (`returned / available`)

검색 엔진에서 1000개 shard 중 950개만 응답해도 결과를 *부분적으로* 반환하면 Yield = 100%, Harvest = 95%. CAP을 거부하지 않으면서도 사용자 경험을 살리는 방식이다.

**적용 가능 여부 판단.** 결제, 재고 차감, ledger entry 같은 *모든-아니면-아무것도-아닌* 작업은 Harvest 트레이드 못 함. 검색, 추천, 통계, 로그 분석은 자주 가능.

---

## 4. 컴포넌트 카드 — 4축 정량 평가

각 카드는 [Invariants / Performance / Workload / Failure] 4축으로. 숫자는 *대략적 자릿수* — 정확한 값은 워크로드/하드웨어/설정 의존이지만 멘탈 모델로는 자릿수 감각이 더 중요.

---

### Redis

- **Invariants.** 단일 키 op는 단일 스레드라 primary 기준 linearizable. 복제는 **비동기 기본** → replica는 stale 가능. Durability는 옵션: RDB(snapshot), AOF(`appendfsync everysec` ≈ 최대 1초 손실, `always` ≈ 매 쓰기 fsync로 ms-scale 비용). **Cluster mode는 cross-shard 트랜잭션 미지원.**
- **Performance.** p50 50–200μs (single node, in-RAM). p99 ~1ms 단일, persistence 켜면 fsync에 따라 튐. throughput 100k+ ops/s/core. capacity = RAM (cluster로 수평 확장 시 TB급 가능, 단 비쌈).
- **Workload.** point KV, atomic counter(`INCR`), sorted set leaderboard, pub/sub, set membership, simple Lua. **약함**: 복잡 query, large range scan, secondary index on values.
- **Failure.** master-replica failover ~수초 (Sentinel/Cluster). 비동기 복제로 인한 **데이터 손실 가능 (replication lag window)**. memory pressure 시 eviction policy(LRU/LFU)가 데이터를 *조용히* 버린다 — SSoT로 쓰면 사고 난다.

**한 줄.** 빠른 휘발성 acceleration 계층. 쓰면 안 되는 곳: ledger, primary 주문 데이터.

---

### Postgres

- **Invariants.** 트랜잭션 SERIALIZABLE 가능 (SSI, Cahill et al. 2008 — MVCC 위에서 true serializability 제공). 기본은 READ COMMITTED. WAL fsync로 단일 노드 durability, 동기 복제(`synchronous_commit=remote_apply`)로 멀티노드 durability. **Logical replication**으로 cross-version/cross-DB 가능.
- **Performance.** OLTP p50 1–5ms, p99 10–50ms. throughput 단일 노드 5k–30k tps (워크로드/HW에 따라 크게 다름). vertical scaling이 기본, horizontal은 Citus/Patroni/sharding 직접.
- **Workload.** 관계, 트랜잭션, JOIN, 다양한 인덱스(B-tree, GIN, GiST, BRIN). JSONB 지원. **약함**: 쓰기 처리량 매우 큰 워크로드(단일 primary 한계), 페타 스케일.
- **Failure.** streaming replication + Patroni/Pacemaker로 자동 failover 가능(수초~분). long-running transaction이 vacuum을 막아 bloat 유발. connection 폭주 시 fork 비용 → pgbouncer 필수.

**한 줄.** SoR 후보 0순위. 수직 스케일링 천장에 닿기 전까지는 거의 항상 정답.

---

### Cassandra

- **Invariants.** **Tunable consistency** — `ONE/QUORUM/ALL`, R+W>N이면 read-after-write를 기대할 수 있다. 단, hinted handoff/repair/conflict resolution 같은 운영 현실이 붙으므로 수식 자체를 절대 보장으로 읽으면 안 된다. 기본 eventual. AP 시스템(PACELC: PA/EL). row-level만 atomic, multi-row 트랜잭션 없음(LWT는 비싸고 조심해서 사용).
- **Performance.** 쓰기 매우 빠름(LSM, log-structured): p50 ms 자릿수. 읽기는 QUORUM에서 p99 꼬리 길어짐(compaction, repair). **선형에 가까운 수평 확장**(수백 노드 검증). capacity는 본질적 제약 없음.
- **Workload.** time-series, write-heavy, primary-key based access(partition key + clustering key 설계가 핵심). **약함**: ad-hoc query, JOIN, secondary index, paging.
- **Failure.** gossip 기반, 노드 다수 손실 견딤. partition 시 AP. 운영 난이도 높음(compaction, repair, tombstone).

**한 줄.** 쓰기 폭주 + key 기반 access + 수평 확장 필수일 때. SQL 비슷해 보이지만 NoSQL 마인드 필수.

---

### DynamoDB

- **Invariants.** Eventually consistent reads가 기본. Strongly consistent reads는 table/LSI에서만 지원되고 eventual read 대비 2× 비용이다.[^dynamodb-read-consistency] **Transactions**(TransactWrite/TransactGet)는 최대 100개 action/items, 총 4MB 제한, 비용 2×.[^dynamodb-transactions] multi-region active-active(global tables)는 *eventual*.
- **Performance.** p99 single-digit ms in-region. throughput은 provisioned / on-demand. **hot partition 주의** — 단일 partition key 천장 ~1000 WCU / 3000 RCU.
- **Workload.** KV / 단순 secondary index(GSI/LSI). 단일 키 access 패턴이면 무한에 가까운 확장. **약함**: ad-hoc query, full-text, 분석.
- **Failure.** managed, 3 AZ 복제, 99.999% SLA(글로벌 테이블). 추상화 잘 돼서 직접 운영 부담 거의 없음. 비용 모델이 함정 — scan은 비싸고, hot partition은 throttle.

**한 줄.** AWS-native KV의 표준. 액세스 패턴이 명확할 때 강력.

---

### Spanner

- **Invariants.** **External consistency = strict serializability**, globally. TrueTime API(GPS + atomic clock의 시간 불확실성 ε)로 commit timestamp 결정. multi-region 트랜잭션 가능.
- **Performance.** 단일 region 읽기 ~5–10ms. multi-region write commit ~50–100ms(Paxos round + TrueTime wait). throughput은 split 단위로 수평 확장. capacity는 사실상 무한(Google이 EB scale 운영).
- **Workload.** 글로벌 SQL OLTP. interleaved tables로 join locality 확보. **약함**: write가 매우 빈번한 단일 row(split 안에 묶임), latency-critical sub-ms 워크로드.
- **Failure.** Paxos group으로 region 손실까지 견딤(multi-region config). RPO ≈ 0, RTO 수초~수분.

**한 줄.** 글로벌 강일관 SQL이 필요한 드문 케이스의 정답. latency 비용 감수 가능할 때만.

---

### Kafka

- **Invariants.** **Per-partition ordered log**. Cross-partition 순서 *보장 없음*. durability는 `acks` 설정(0/1/all) + ISR 수에 따름. `acks=all` + `min.insync.replicas=2` + `replication.factor=3`이 표준 안전 설정. exactly-once는 idempotent producer + transactions로 *내부* 가능, end-to-end는 consumer idempotency가 필요.
- **Performance.** producer p99 ~10ms(`acks=all`, in-region). 처리량 broker당 수십 MB/s ~ GB/s(크기/배치/압축에 따라). consumer는 pull 기반.
- **Workload.** event log, stream, fan-out, replay. **약함**: random key access, 현재 상태 query(그건 KTable/materialized view로 풀어야 함), 작은 메시지 매우 많을 때 metadata 오버헤드.
- **Failure.** ISR 줄어들면 availability vs consistency trade(`unclean.leader.election`). consumer lag이 silent failure의 주범. retention 만료 시 데이터 영구 손실.

**한 줄.** "현재 상태"가 아닌 "변화의 흐름"을 다루는 시스템의 척추. 저장소처럼 보이지만 시간순 로그다.

---

### S3 (그리고 호환 object storage)

- **Invariants.** 2020년 12월부터 **strong read-after-write consistency**. PUT/overwrite/delete 이후 GET/LIST가 최신 상태를 반영한다.[^s3-consistency] 11 nines durability — 다중 AZ erasure coding. object는 partial update가 아니라 whole-object overwrite 단위이며, versioning을 켜면 PUT이 새 version을 만든다.
- **Performance.** GET first-byte latency ~수십 ms. throughput per-prefix는 최소 ~5,500 RPS(GET) / 3,500 RPS(PUT)이며 prefix 수에는 제한이 없다.[^s3-performance] 무제한 capacity. Glacier는 cold storage.
- **Workload.** blob, 백업, 정적 자원, data lake(Parquet 위에 Athena/Spark). **약함**: hot small-key 워크로드, append(multipart upload는 됨), 진짜 random write(object 단위만).
- **Failure.** managed, region 단위 outage는 드물지만 발생. cross-region replication(CRR)으로 DR 가능.

**한 줄.** 가장 저렴한 무한 durable storage. 데이터 레이크와 백업의 기본기.

---

### Elasticsearch

- **Invariants.** **Near real-time**(refresh interval 기본 1초). 같은 shard 내에선 eventual. translog로 durability. Quorum-style write(`wait_for_active_shards`). split-brain은 zen2 / 7.x 이후 완화.
- **Performance.** query 10–100ms(복잡도 의존). bulk indexing 처리량 좋음. aggregation은 cardinality와 shard 수에 강하게 좌우.
- **Workload.** full-text 검색, 로그/observability, 다차원 aggregation, geo, vector. **약함**: SoR(재색인 가능해야 함), 복잡 트랜잭션, 강일관성.
- **Failure.** shard rebalancing이 부하 spike 유발. 메모리 압박(heap, fielddata)이 가장 흔한 운영 이슈. version 호환성 깐깐.

**한 줄.** 검색 DB지 진실 DB가 아니다. SoR을 따로 두고 색인은 derived view로.

---

### CDN (Content Delivery Network; CloudFront, Fastly, Cloudflare 등)

- **Invariants.** TTL 기반 eventual. immutable object identity(URL + ETag). stale-while-revalidate로 availability 우선.
- **Performance.** edge p50 single-digit ms. cache miss 시 origin shield 거쳐 origin RTT.
- **Workload.** static, cacheable, immutable. API GET 응답도 적절한 TTL이면 가능. **약함**: 개인화, 실시간, write.
- **Failure.** stale 응답으로 graceful degradation. origin 장애도 cache hit ratio만큼은 무관.

**한 줄.** 사용자 가까이 있는 *빠른 거짓말쟁이*. 빠르지만 진실 반영이 느리다.

---

### SQLite

- **Invariants.** ACID, single-writer serializable(WAL 모드 포함). file-as-DB.
- **Performance.** 로컬 in-process, μs 자릿수. throughput는 single-writer 천장.
- **Workload.** embedded, 단일 앱, 작은~중간 데이터셋, 분석/보고서(CTE, window 함수 잘 지원). **약함**: 동시 writer, 네트워크 access(LiteFS/Cloudflare D1 같은 wrapper로 보강 가능).
- **Failure.** 파일 손상이 주된 모드. corruption은 매우 드물지만 발생 가능 → 백업 필수.

**한 줄.** "단일 프로세스에서 충분"이라는 조건만 맞으면 거의 항상 옳은 선택. Hipp이 만든 가장 많이 deploy된 DB.

---

### DuckDB

- **Invariants.** ACID 단일 프로세스. columnar, vectorized execution. Parquet/CSV/Arrow를 native로 read.
- **Performance.** 단일 노드 GB/s 스캔. analytical 쿼리 초~수십 초.
- **Workload.** ad-hoc analytics, embedded analytics, 데이터 사이언스. **약함**: OLTP, 동시 writer, 분산.
- **Failure.** 프로세스 로컬, 디스크 손상이 주.

**한 줄.** "분석용 SQLite". S3 + Parquet + DuckDB는 작은 팀의 무서운 데이터 스택.

---

## 5. 결정 트리 — 4단계 압축 절차

새 워크로드 / 새 컴포넌트 선택 시 머릿속에서 돌리는 절차.

### Step 1 — Invariants 먼저

> *"이게 깨지면 사고 나는 보장은 무엇인가?"*

- **Linearizable coordination 필요?** → Spanner, etcd, ZooKeeper, FoundationDB. Cassandra/DynamoDB(eventual) 탈락.
- **트랜잭션(multi-row) 필요?** → SQL 계열, Spanner, FoundationDB. NoSQL 다수 탈락.
- **Durability 사고 발생 시 데이터 손실 허용 X?** → SoR로 지정. Redis/ES/CDN은 후보 아님.
- **부분 손실 OK(cache, search index, log analytics)?** → Derived view 후보로 자유롭게.

이 단계에서 후보 50%가 빠진다.

### Step 2 — Workload shape

> *"이 데이터/쿼리는 어떤 모양인가?"*

- *Point KV* → KV store (Redis, DynamoDB, Mongo, Postgres OK)
- *Range scan + 정렬* → B-tree / LSM (Postgres, Cassandra, MySQL)
- *Full-text / fuzzy* → ES, OpenSearch, Vespa
- *Aggregation over many rows* → Columnar (ClickHouse, Druid, BigQuery, DuckDB)
- *Append-only event log* → Kafka, Pulsar
- *Blob* → S3
- *Graph traversal* → Neo4j, Dgraph, JanusGraph
- *Vector similarity* → Faiss/HNSW, pgvector, Qdrant, Weaviate

### Step 3 — Failure 모델

> *"어떤 장애를 견뎌야 하는가? RPO/RTO는?"*

- *단일 노드*면 충분 → SQLite, single Postgres
- *AZ 손실*까지 → multi-AZ (RDS Multi-AZ, DynamoDB, Spanner zonal config)
- *Region 손실*까지 → multi-region (S3 CRR, Spanner multi-region, Aurora Global, Cassandra multi-DC)
- *Software bug correlation*까지 → 다른 software/version으로 backup. (실전에서 무시되지만 가장 위험한 카테고리.)

### Step 4 — Latency 예산

> *"p99이 얼마면 사용자가 행복한가? 혹은 SLA를 충족하는가?"*

이 숫자가 PACELC의 E 선택을 결정한다.

| p99 예산 | 함의 |
|---|---|
| < 1 ms | 로컬 메모리. Redis on same host, in-process cache, SQLite, embedded. **다른 host로 가면 끝.** |
| < 10 ms | same-AZ 동기 복제. 보통의 OLTP. cross-AZ는 빠듯. |
| < 100 ms | cross-AZ 동기 OK. cross-region 읽기 OK (write는 빠듯). |
| < 1 s | cross-region 동기 가능 (Spanner). user-perceivable 한계. |
| > 1 s | 분석/batch. consistency vs latency 부담 거의 없음. |

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
- AWS. *DynamoDB read consistency*. 공식 문서.
- AWS. *DynamoDB Transactions: How it works*. 공식 문서.
- AWS. *Amazon S3 Strong Consistency*. 공식 문서.

---

*마지막 한 마디.* 이 문서가 가장 잘 작동하는 사용법: 새 컴포넌트를 만나면 §4의 카드 형식으로 직접 한 장 채워보라. 채울 수 없는 칸이 있으면 그 컴포넌트에 대해 *모른다*는 뜻이다. 그 빈칸이 곧 다음 공부 거리다.

[^dynamodb-read-consistency]: AWS 공식 문서에 따르면 DynamoDB의 eventually consistent read는 기본값이며, strongly consistent read는 table과 LSI에서만 지원된다. 비용도 다르다. 4KB 이하 item 기준 strongly consistent read는 1 RCU, eventually consistent read는 0.5 RCU를 소비한다. GSI와 stream read는 eventually consistent만 지원한다. <https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/HowItWorks.ReadConsistency.html>

[^dynamodb-transactions]: DynamoDB `TransactWriteItems`/`TransactGetItems`는 최대 100개 action/items를 하나의 all-or-nothing operation으로 묶을 수 있고, aggregate item size는 4MB를 넘을 수 없다. 예전 자료에는 25개 제한으로 적힌 경우가 있으나 현재 공식 문서는 100개로 안내한다. <https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/transaction-apis.html>

[^s3-consistency]: Amazon S3는 2020년 12월부터 모든 region에서 PUT, overwrite, delete 이후 GET/LIST에 대해 strong read-after-write consistency를 제공한다. <https://aws.amazon.com/s3/consistency/>

[^s3-performance]: AWS는 S3 성능이 prefix 단위로 scaling되며 prefix당 최소 3,500 PUT/s, 5,500 GET/s를 지원하고, prefix 수에는 제한이 없다고 설명한다. <https://aws.amazon.com/s3/consistency/>
