+++
title = "데이터 시스템 컴포넌트 평가 프레임워크"
date = "2026-05-05T15:30:50+09:00"
description = "시스템 디자인 컴포넌트를 네 가지 축(보장/성능/워크로드/실패)으로 평가하는 멘탈 모델"
math = true
+++

# 데이터 시스템 컴포넌트 평가 프레임워크

> 시스템을 설계할 때 컴포넌트(Redis, Postgres, Kafka, S3 …)를 머릿속에서 비교하고 추산하기 위한 멘탈 모델.
> 축을 무작정 외우기보다, 먼저 세 가지 근본 제약에서 출발해 각 축이 왜 필요한지 짚고, 마지막에 네 축으로 압축한다.

---

## 0. 10분 버전

축은 네 개다.

| 축 | 한 줄 정의 | 무엇을 보는가 |
|---|---|---|
| 보장(Invariants) | 무엇을 보장하는가 | 일관성 모델(consistency model), 시간적 의미(얼마나 오래된 값까지 보일 수 있는지), 내구성(durability) 모델 |
| 성능(Performance) | 그 보장의 비용 | 지연 분포(p50/p99/p999), 처리량 천장, 용량, 코디네이션(coordination) 비용 |
| 워크로드(Workload) | 어떤 모양에 맞는가 | 접근 패턴(point/range/scan/aggregation), 데이터 모양(KV, 관계형, 문서, blob, 로그 등) |
| 실패(Failure) | 어떻게 깨지는가 | 실패 모델, 영향 범위(blast radius), 백프레셔(backpressure)가 빠져나가는 지점, 복구 시간(RPO/RTO) |

순서가 중요하다. Speed / Truth / Scale은 독립된 축이 아니다. 보장을 먼저 정하면 성능과 실패의 모양도 상당 부분 따라 정해진다. PACELC가 이걸 말한다. 일관성을 강하게 잡으면 지연이 따라 붙는다.

빠른 결정 절차는 네 단계로 압축된다.

1. 깨지면 사고로 이어지는 보장은 무엇인가. 여기서 진실의 원천(System of Record) 후보가 좁혀진다.
2. 워크로드의 모양은 어떤가. 여기서 저장 구조(B-tree, LSM, columnar, inverted index)가 결정된다.
3. 어떤 실패까지 견뎌야 하는가. 여기서 복제(replication) 토폴로지와 RPO/RTO가 결정된다.
4. p99 지연 예산은 얼마인가. PACELC의 E를 어디로 둘지 결정한다. 예산이 1ms라면 리전(region) 간 동기 합의는 불가능하다.

컴포넌트는 네 계층으로 나누면 사고가 명확해진다.

- 진실의 원천(System of Record). Postgres, Spanner, 원장(ledger), S3 원본 같은 것.
- 파생/가속(Derived View). Redis, Elasticsearch, materialized view, CDN.
- 이동/전파(Transport). Kafka, RabbitMQ, CDC, queue.
- 연산 계층(Compute Substrate). Spark, Flink, DuckDB, Trino. 저장도 이동도 아닌 계산.

이 분류는 시스템 박스가 아니라 데이터에 매긴다. 같은 Postgres에서도 어떤 테이블은 권위 있는(authoritative) 데이터이고 다른 테이블은 파생(derived) 데이터일 수 있다. 같은 Redis에서도 어떤 키는 일시적(transient) 캐시이고 다른 키는 재생성 가능한 파생 뷰일 수 있다. 사고는 대개 일시적이거나 파생이어야 할 데이터를 권위 있는 것처럼 다루거나, 그 반대일 때 일어난다.

이 글은 처음부터 끝까지 한 번에 읽는 에세이라기보다, 새 컴포넌트를 만났을 때 꺼내 보는 평가표에 가깝다. 시간이 없으면 이 10분 버전과 §4의 컴포넌트 카드만 먼저 읽어도 된다. §1~§3은 왜 이런 평가표가 나오는지 설명하는 배경이고, §5는 실제 선택 절차다.

---

## 1. 왜 이 축들이 등장하는가: 세 가지 근본 제약

데이터 시스템의 거의 모든 트레이드오프(trade-off)는 결국 다음 셋의 조합으로 환원된다. 새 컴포넌트를 만나도 이 셋으로 풀어 보면 같은 형태가 드러난다.

### 1.1 물리적 제약: 지연의 하한

빛은 광섬유 안에서 약 200km/ms로 전파된다. 이 한계는 어떤 알고리즘으로도 줄일 수 없다.

자릿수 감각을 정리해 두면 다음과 같다.

| 동작 | 시간 | 비고 |
|---|---|---|
| L1 캐시 | 약 1ns | |
| DRAM 접근 | 약 100ns | |
| NVMe random read | 약 100μs | |
| HDD seek | 약 10ms | 회전 + seek |
| fsync (NVMe + battery-backed cache) | 약 100μs ~ 수 ms | 내구성의 진짜 비용 |
| 같은 AZ 네트워크 RTT | 약 0.5ms | |
| 같은 리전, 다른 AZ | 약 1~2ms | |
| US east ↔ west | 약 70ms | 광속 한계 |
| 대서양 횡단 | 약 80~100ms | |
| 태평양 횡단 | 약 150ms | |

그래서 "리전 간 강한 일관성(strong consistency)을 한 자릿수 ms 안에" 같은 요구는 물리적으로 불가능하다. Spanner가 리전 간 commit에 50~100ms를 쓰는 건 게을러서가 아니라 광속 때문이다.

### 1.2 정보이론적 제약: 저장과 접근의 트레이드오프

- 무작위 데이터는 압축되지 않는다. Kolmogorov 하한이 막는다.
- 인덱스는 공간을 먹는다. B+ tree는 키 개수에 비례해 공간 비용을 치른다.
- 어떤 자료구조도 읽기, 쓰기, 메모리 셋을 동시에 최소화할 수는 없다. §2.3의 RUM Conjecture가 이걸 정리한다.

그래서 "쓰기 빠르고, 읽기 빠르고, 공간도 적게 쓰는 인덱스"는 없다. 어디로 비용을 떠넘길지를 고를 뿐이다. LSM은 쓰기를 빠르게 하는 대신 그 비용을 컴팩션(compaction)으로 떠넘긴다. 그 부담은 읽기, CPU, 공간에 분산된다.

### 1.3 분산 시스템 불가능성: 코디네이션 비용

- FLP (Fischer–Lynch–Paterson, 1985). 비동기 메시지 모델에서는 단 한 노드의 fail-stop 장애만 있어도 결정론적 합의가 불가능하다. 실용 시스템은 타임아웃, 무작위화, failure detector로 이를 우회한다.
- CAP (Brewer 2000, Gilbert–Lynch 2002). 파티션(partition)이 발생하면 C와 A를 동시에 보장할 수 없다.
- 쿼럼(quorum) 정리. N개 복제본(replica)에서 쓰기 직후 읽기(read-after-write)를 보장하려면 $|R| + |W| > N$이어야 한다. Dynamo와 Cassandra의 ONE/QUORUM/ALL 옵션을 이해하는 기본 모델이다.
- 합의의 라운드 수. Paxos와 Raft의 정상 경로는 최소 두 번의 메시지 지연이 필요하다. 그래서 리전 간 강한 일관성 쓰기를 100ms 안에 끝내기는 어렵다.

코디네이션에는 늘 비용이 따라온다. 지연이거나, 가용성이거나, 둘 다거나. "공짜 강한 일관성"이라는 표현은 마케팅에서나 본다.

---

## 2. 네 축 프레임워크

흔히 6축(Speed / Truth / Scale / Shape / Failure / Ops)으로 쪼개기도 한다. 기억하기는 좋지만 축끼리 꽤 겹친다. Speed는 Truth를 얼마나 강하게 잡느냐에 따라 달라지고, Scale은 대개 성능 envelope 안에서 드러난다. Ops 비용도 독립된 속성이라기보다 보장과 워크로드, 실패 선택의 결과로 따라온다. 그래서 이 글에서는 네 축으로 압축한다.

### 2.1 보장(Invariants): 무엇을 보장하는가

가장 먼저 결정해야 할 축이다. 나머지 축은 이 결정에 따라 정해진다.

일관성 모델은 강한 것부터 약한 것까지 다음과 같이 정리된다. 정의가 자주 헷갈리니 정확히 짚는다.

| 모델 | 정의 | 출처 |
|---|---|---|
| 엄격한 직렬화 가능성 (strict serializability) | 직렬화 가능성 + 실시간 순서 존중. Spanner의 "external consistency"가 여기. | Papadimitriou 1979 + Herlihy/Wing 1990 |
| 선형화 가능성 (linearizability) | 단일 객체. 각 op이 시작과 종료 사이의 한 점에서 순간적으로 발생한 것처럼 보인다. | Herlihy & Wing, TOPLAS 1990 |
| 직렬화 가능성 (serializability) | 트랜잭션. 어떤 직렬 순서가 존재한다. 실시간은 보장하지 않는다. | Papadimitriou, JACM 1979 |
| 스냅샷 격리 (snapshot isolation) | 트랜잭션이 시작 시점의 스냅샷을 본다. 쓰기 비대칭(write skew)은 허용된다. | Berenson et al., SIGMOD 1995 |
| 인과 일관성 (causal consistency) | 인과 관계가 있는 op들의 순서만 보존한다. | Lamport 1978 |
| 제한된 staleness (bounded staleness) | "최대 X초 또는 X 버전만큼 뒤떨어진다"를 약속한다. | Cosmos DB, Spanner stale read |
| 결과적 일관성 (eventual consistency) | 충분히 시간이 흐르면 수렴한다. | Vogels, CACM 2009 |

선형화 가능성과 직렬화 가능성은 다르다. 전자는 단일 객체의 실시간 순서, 후자는 트랜잭션의 직렬화 가능성을 다룬다. 둘을 동시에 보장하면 엄격한 직렬화 가능성이다.

같은 "일관성" 라벨이라도 시간 차원이 다르면 사실 다른 시스템이다. 일관성 모델이 "어떤 보장"을 정의한다면, 시간적 의미는 "얼마나 오래된 값까지 보일 수 있는지"를 정한다.

- 읽기 신선도(read freshness), staleness 한도. 읽기가 얼마나 오래된 값을 볼 수 있는가. Elasticsearch refresh interval은 기본 1초, Redis 복제 지연(replication lag)은 ms~s, Cassandra의 결과적 일관성은 anti-entropy repair 전까지 사실상 무한대까지 갈 수 있다.
- 쓰기 가시화 지연(write visibility delay). 쓰기가 다른 reader에게 보이기까지 걸리는 시간. Spanner는 commit 직후, MongoDB는 `readConcern: majority`라면 majority commit 이후, S3는 PUT 이후 쓰기 직후 읽기, Kafka는 ISR 복제 완료 이후.
- 순서 보장(ordering guarantee). Kafka는 파티션 내부 순서만 보장하고 파티션 간 순서는 보장하지 않는다. RDB는 commit 순서. CRDT는 순서 자체에 의존하지 않는다.

이 구분이 필요한 이유는 단순하다. Kafka에는 순서는 있어도 "현재 상태"가 없다. 같은 결과적 일관성이라도 staleness 한도가 100ms인지 10초인지에 따라 운영 판단은 완전히 달라진다.

내구성 모델도 같은 방식으로 짚어 둔다. "내구성이 있다"가 어떤 가정 위에 성립하는지 확인할 필요가 있다.

- 단일 노드 fsync (SQLite, 단일 노드 Postgres). 디스크가 죽으면 끝.
- 동기 복제 fsync (Postgres `synchronous_commit=on` + 동기 복제). N대 동시 손실까지 견딘다.
- 쿼럼 durable (Cassandra W=QUORUM, Spanner). 쿼럼이 깨지기 전까지 안전.
- 다중 AZ erasure coded (S3, DynamoDB). 리전 내에서는 사실상 무손실.
- 다중 리전(multi-region) (Spanner, Aurora Global). 리전 손실에도 RPO≈0이 가능.

핵심 질문은 늘 같다. 이 컴포넌트가 죽었을 때 데이터가 사라져도 되는가. 답이 NO이면 진실의 원천 후보다. 나머지 계층(파생, 이동, 연산)은 재생성 가능해야 한다.

### 2.2 성능(Performance): 그 보장의 비용

세 가지 하위 차원으로 본다.

지연은 평균이 아니라 분포로 본다. Dean과 Barroso의 "The Tail at Scale"이 짚었듯이, 팬아웃(fan-out) 아키텍처에서는 꼬리 지연(tail latency)이 시스템 전체 지연을 지배한다. 100개로 팬아웃하면 시스템 지연은 사실상 컴포넌트의 p99이 되고, 1000개로 팬아웃하면 p999가 된다. 그래서 컴포넌트를 평가할 때 p50만 빠른 컴포넌트는 함정이다. p50, p99, p999를 따로 본다.

처리량(throughput) 천장은 Little's Law로 추산한다.

$$
L = \lambda W
$$

즉 동시성 = 도착률 × 잔류 시간이다.

- 100 connection 풀에서 요청당 10ms면 10,000 req/s가 천장이다.
- 목표가 10k req/s이고 worker가 100대라면 요청당 10ms 안에 끝나야 한다.
- $\lambda$가 천장에 가까워질수록 $W$는 폭발한다. M/M/1 queue에서는

$$
W = \frac{1}{\mu - \lambda}
$$

가 된다. "사용률이 80%를 넘으면 지연이 무너진다"는 말의 수학적 근거다.

스케일링은 Amdahl을 일반화한 USL(Universal Scalability Law; Gunther 2007)로 본다.

$$
C(N) = \frac{N}{1 + \alpha(N - 1) + \beta N(N - 1)}
$$

$\alpha$는 직렬화 계수(공유 자원 경합, contention), $\beta$는 일관성 계수(노드 간 간섭, coherency/cross-talk)다. $\beta > 0$이면 어떤 $N^*$ 이후 처리량이 오히려 감소한다. "노드를 더 넣었더니 오히려 느려졌다"가 여기서 나온다.

용량(capacity)은 단일 숫자가 아니다.

- 작업 셋(working set). 메모리에 올라가야 할 hot 데이터 크기.
- 전체 데이터셋. 디스크와 네트워크에 보관되는 cold 데이터까지 포함한 크기.
- IOPS vs 대역폭(bandwidth). random small ops 한계와 sequential MB/s 한계는 다른 숫자다.

Redis의 용량이 "작다"는 말은 작업 셋이 RAM에 묶인다는 뜻이지, 1TB Redis가 불가능하다는 뜻이 아니다.

코디네이션 비용은 한 op이 몇 개 노드의 합의를 요구하는가의 문제다. 이 값이 지연 하한을 직접 결정한다. PACELC의 E를 만들어 내는 메커니즘이다.

| 시스템 | 한 쓰기당 노드 수 | 동기/비동기 | 지연 하한 |
|---|---|---|---|
| Redis 단일 | 1 | — | 메모리 접근(~100μs) |
| Postgres 단일 + WAL fsync | 1 + 디스크 | sync | fsync 비용(~수 ms) |
| Postgres + 동기 복제 1대 | 2 | sync | + 같은 AZ RTT(~1ms) |
| Cassandra W=QUORUM (N=3) | 2/3 응답 대기 | sync | 가장 느린 1/2의 지연 |
| Spanner 리전 간 commit | 다중 리전 Paxos | sync | 50~100ms (Paxos round + TrueTime wait) |
| Kafka `acks=all` (RF=3) | ISR 전체 ack | sync | replica RTT max |

새 컴포넌트를 만나면 늘 이 두 질문을 던진다. 한 쓰기에 몇 노드가 동기적으로 참여하는가. 그 노드들 사이 RTT는 얼마인가. 이 두 숫자가 지연 floor를 사실상 결정한다. 어떤 최적화도 이 floor 아래로는 못 내려간다. §1.1에서 본 광속 한계가 허락하지 않는다.

### 2.3 워크로드(Workload): 어떤 모양에 맞는가

RUM Conjecture (Athanassoulis et al., EDBT 2016)는 자료구조 설계의 근본 트릴레마(trilemma)를 정리한다. 읽기 오버헤드(R), 갱신 오버헤드(U), 메모리 오버헤드(M) 셋을 동시에 최소화할 수는 없다. 둘을 잡으면 하나는 포기해야 한다.

| 자료구조 | R | U | M | 잘 맞는 패턴 |
|---|---|---|---|---|
| B+ tree | 낮음 | 중간 | 중간 | point + range |
| LSM tree | 중간 (컴팩션 꼬리) | 낮음 | 중간~높음 (Bloom filter, memtable, 여러 SSTable, 컴팩션 여유 공간) | 쓰기 위주 + range |
| Hash index | 낮음 (point) | 낮음 | 중간 | point만 |
| Inverted index | 중간 | 높음 (rebuild/reindex) | 높음 | 전문 검색, 집합 멤버십 |
| Columnar (Parquet 같은) | 낮음 (scan/agg) | 매우 높음 (immutable) | 낮음 (압축이 잘 됨) | 분석 scan/agg |
| Bitmap index | 낮음 (set ops) | 높음 | 낮음 (저-카디널리티일 때) | 저-카디널리티 필터링 |

접근 패턴은 다음 정도로 분류해 두면 충분하다.

- point lookup. 키로 한 건. 모든 인덱스가 가능하다.
- range scan. 정렬된 구간. B-tree, LSM.
- full scan, aggregation. 컬럼 단위 통계. columnar.
- search, matching. 텍스트, 다차원. inverted index, R-tree, vector index.
- append-only log. 추가만. Kafka, WAL.

데이터의 모양은 다음 정도로 분류한다.

- Key-Value (Redis, DynamoDB, Riak)
- Wide-column (Cassandra, HBase, BigTable)
- Document (MongoDB, Couch)
- Relational (Postgres, MySQL, Spanner)
- Graph (Neo4j, Dgraph)
- Search (Elasticsearch, Solr)
- Blob (S3, GCS)
- Log/Stream (Kafka, Pulsar)
- Columnar OLAP (ClickHouse, Druid, BigQuery, DuckDB)

### 2.4 실패(Failure): 어떻게 깨지는가

실패 모델은 컴포넌트가 어떤 장애를 견딘다고 주장하는지 확인하는 데서 시작한다.

| 모델 | 의미 |
|---|---|
| Crash-stop | 노드는 멈출 뿐, 거짓 메시지를 보내지 않는다 |
| Crash-recovery | 멈췄다가 깨어나서 상태를 복구한다 |
| Omission | 메시지 분실이 가능하다 |
| Byzantine | 임의의 거짓 동작이 가능하다 |

거의 모든 상용 데이터 시스템은 crash-recovery + omission을 가정한다. Byzantine까지 가정하면 비용이 너무 크다.

실제로 가장 위험한 건 독립 장애 가정이 깨질 때다.

- 같은 랙(rack)의 노드는 동시 손실이 가능하다.
- 같은 AZ는 정전이나 네트워크로 함께 죽는다.
- 같은 리전은 대규모 장애로 함께 죽는다.
- 같은 소프트웨어 버전은 같은 버그로 함께 죽는다. 특히 무서운 경우.

영향 범위는 컴포넌트 하나가 죽었을 때 다른 무엇이 영향을 받는가의 문제다.

- Redis가 죽으면 캐시 미스(cache miss)로 origin이 폭격당한다. 썬더링 허드(thundering herd).
- DB primary가 죽으면 쓰기가 정지된다. 그 동안 큐가 적재되고, 복구 시 부하가 폭주한다.

복구 시간은 두 숫자로 정리한다.

- RPO (Recovery Point Objective). 잃을 수 있는 데이터의 양을 시간 단위로.
- RTO (Recovery Time Objective). 복구까지 걸리는 시간.

이 두 숫자가 가용성 SLA의 실체다. "five nines"는 마케팅이고, RPO/RTO가 엔지니어링이다.

Harvest와 Yield (Fox & Brewer, HotOS 1999)는 CAP의 0/1 선택을 완화한 관점이다.

- Yield. 응답한 요청의 비율. 전통적 가용성.
- Harvest. 응답에 반영된 데이터의 비율. 예를 들어 검색이 95%의 샤드(shard)만 반환했다면 Harvest가 95%.

장애 시 "거절"이 아니라 "부분 응답"으로 우아한 성능 저하(graceful degradation)를 설계할 수 있다. 검색 엔진과 추천 시스템이 이 모델을 자주 쓴다.

백프레셔는 어디서 막히고 어디로 흐르는가의 문제다. 분산 시스템이 직접 죽는 일은 드물다. 거의 항상 백프레셔가 무너지고 연쇄 장애(cascade failure)로 죽는다. 컴포넌트마다 1차 병목이 다르고, 그 병목이 차오를 때 압력이 어디로 흘러가는지가 곧 영향 범위다.

| 컴포넌트 | 1차 백프레셔 지점 | 붕괴 양상 | 압력의 출구 |
|---|---|---|---|
| Redis | 메모리 압박, 단일 스레드 CPU | OOM, eviction 폭주, command timeout | client → origin DB 직격 |
| Postgres | 커넥션 풀(connection pool), 락 경합, WAL 디스크 | 커넥션 거부, lock wait timeout | 상류 큐 적재 |
| Kafka | 파티션 처리량, consumer lag, retention | lag 증가, retention 초과 시 영구 손실 | 상류 producer 지연 또는 disk full |
| S3 | per-prefix RPS, 5xx throttle | 지수 백오프(exponential backoff) 안 하면 cascade | client retry storm |
| Elasticsearch | heap, fielddata, 샤드 재분배 | OOM, GC stop-the-world, query rejection | 색인 파이프라인 적재 |
| CDN | 캐시 미스로 origin RPS 폭주 | hit ratio 붕괴 시 origin 폭격 | origin 직접 부하 |
| Spanner | split throughput, hot row | tablet split, latency spike | commit 대기열 |

설계할 때 던질 질문은 단순하다. 이 컴포넌트의 큐가 차오르면, 압력은 어디로 흘러가는가. 이 질문에 답할 수 있어야 격리(isolation), 격벽(bulkhead), 회로 차단기(circuit breaker), 백프레셔 인식 큐 같은 방어 패턴을 어디에 둘지 결정할 수 있다. 백프레셔가 어디로 흐르는지 모르면 작은 장애가 시스템 전체로 번진다.

---

## 3. 시스템 설계 이론, 오해 없이 쓰기

CAP을 "셋 중 하나 포기"로만 기억하면 실무 판단에 도움이 안 된다. 중요한 이론일수록 무엇을 말하고 무엇을 말하지 않는지를 정확히 구분해야 한다.

### 3.1 CAP — Brewer 2000, Gilbert–Lynch 2002

정확한 명제는 이렇다. Gilbert와 Lynch 2002에 따르면, 비동기 네트워크에서는 다음 셋을 동시에 보장하는 분산 데이터 객체가 존재하지 않는다.

- Consistency. 원자적(atomic), 선형화 가능한 일관성.
- Availability. 죽지 않은 노드는 모든 요청에 응답한다.
- Partition tolerance(파티션 내성). 임의의 메시지 손실 상황에서도 시스템이 정상 동작을 시도한다.

흔한 오해 셋을 짚는다.

첫째, "셋 중 둘만 고르라"는 표현은 정확하지 않다. P는 현실이지 선택지가 아니다. 실제 선택은 파티션이 발생했을 때 C와 A 중 하나다. 파티션이 없을 때는 둘 다 가능하다.

둘째, "C는 일반적인 의미의 일관성"이 아니다. CAP의 C는 선형화 가능성이다. 스냅샷 격리나 결과적 일관성 같은 다른 의미의 "일관성"은 CAP의 영역 밖이다.

셋째, "A는 SLA 가용성"도 아니다. CAP의 A는 살아있는 모든 노드가 응답해야 한다. 일부 노드가 응답하지 않아도 SLA를 만족하는 시스템은 CAP의 A를 위반하지만 실용적으로는 가용하다.

CAP은 강력한 단순화지만 실무 도구로는 거칠다. 다음의 PACELC가 더 유용하다.

### 3.2 PACELC — Abadi 2012

Abadi, "Consistency Tradeoffs in Modern Distributed Database System Design", IEEE Computer 2012의 표현은 이렇다.

> If Partition (P), choose between Availability (A) and Consistency (C); Else (E), choose between Latency (L) and Consistency (C).

CAP의 진짜 확장이다. 핵심은 다음 한 문장이다. 파티션이 없을 때도 일관성과 지연은 트레이드된다. 일관성을 강하게 잡으려면 더 많은 노드의 합의를 기다려야 하기 때문이다.

| 시스템 | PACELC | 의미 |
|---|---|---|
| Dynamo, Cassandra, Riak | PA / EL | 파티션 시 A 우선, 평상시 지연 우선 (결과적 일관성) |
| Spanner, FaunaDB | PC / EC | 항상 C 우선 (지연 비용 감수) |
| Postgres (단일 노드) | — | 파티션 무관 |
| MongoDB (default-ish) | 설정 의존 | readConcern/writeConcern/readPreference에 따라 PA/EC처럼도, 더 강한 C 우선처럼도 동작한다 |
| BigTable, HBase | PC / EC | C 우선 |

실무에서는 "이 DB는 일관성이 강한가"가 아니라 "평상시에 어느 정도의 지연을 받아들일 수 있는가"가 진짜 질문이다. p99 1ms 예산이라면 리전 간 합의는 불가능하니 EL 시스템밖에 못 쓴다.

### 3.3 RUM Conjecture — Athanassoulis et al., EDBT 2016

"Designing Access Methods: The RUM Conjecture", EDBT 2016에서 정리한 내용이다.

> Read overhead, Update overhead, Memory overhead 셋을 동시에 최소화하는 접근 방법은 없다.

증명된 정리는 아니지만 강력한 설계 가이드다. 새 인덱스나 저장 엔진을 만나면 첫 질문은 늘 같다. 어떤 두 가지에 강하고, 어디로 비용을 떠넘기는가.

- B+ tree (Postgres, MySQL). R이 좋고 U/M은 중간. 정렬된 트리를 유지하는 비용이 든다.
- LSM tree (RocksDB, Cassandra, ScyllaDB). U가 좋지만 비용이 사라지는 것은 아니다. 읽기 증폭(read amplification), 공간 증폭(space amplification), 컴팩션 비용으로 이동한다.
- Hash table. R도 좋고(point) U도 좋지만 range scan은 불가능하다.
- Fractal tree, Bε-tree. B-tree와 LSM 사이의 균형이다. TokuDB.
- Learned index (Kraska et al. 2018). M을 줄이려고 모델로 인덱스를 대체한다. 분포가 안정적이어야 동작한다.

### 3.4 Little's Law — Little 1961

$$
L = \lambda W
$$

$L$은 시스템 내 평균 요청 수, $\lambda$는 도착률, $W$는 평균 잔류 시간이다. 가장 단순하지만 가장 자주 쓰는 식이다. 처리량 추산의 기본 산수.

활용 예시.

- DB 커넥션 풀 100개, 평균 쿼리 10ms면 최대 10,000 qps.
- 목표 5,000 qps, p50 4ms면 평균 동시 in-flight가 20개. 풀이 100이면 충분하고 10이면 부족하다.
- Kafka consumer N대에서 메시지당 처리가 50ms면 처리량은 N / 0.05다.

큐잉 이론의 M/M/1에서 평균 잔류 시간은

$$
W = \frac{1}{\mu - \lambda}
$$

이고 사용률은

$$
\rho = \frac{\lambda}{\mu}
$$

다. $\rho$가 1에 가까워지면 $W$는 쌍곡선처럼 치솟는다. "DB CPU가 80%를 넘어가면 지연이 무너진다"는 말의 수학적 근거다.

### 3.5 USL — Gunther 2007

$$
C(N) = \frac{N}{1 + \alpha(N - 1) + \beta N(N - 1)}
$$

$\alpha$는 경합(공유 자원 직렬화 비율), $\beta$는 노드 간 간섭(동기화 cross-talk)이다. $\beta > 0$이면 최적 노드 수가 존재한다.

$$
N^* = \sqrt{\frac{1 - \alpha}{\beta}}
$$

그 이상에서는 처리량이 오히려 감소한다. 역행 스케일링(retrograde scaling)이라 부른다.

실무에서는 부하 테스트 결과 $(N, throughput)$ 점들을 USL에 피팅하면 $\alpha$, $\beta$가 나온다. $\beta$가 0에 가깝지 않으면 아키텍처에 본질적인 노드 간 간섭이 있다는 신호다. 분산 락(distributed lock), 글로벌 카운터, 캐시 무효화 폭주 같은 것들. 이런 경우 노드를 더 투입하는 방식으로는 해결되지 않는다.

### 3.6 Tail at Scale — Dean & Barroso, CACM 2013

한 문장으로 정리하면 이렇다. 팬아웃 아키텍처에서는 시스템 지연이 평균이 아니라 꼬리에 좌우된다.

간단한 계산을 보자. 컴포넌트 하나의 p99이 10ms일 때, 100개로 팬아웃한 요청의 시스템 지연은 그중 하나라도 10ms 이상 걸릴 확률에 좌우된다.

$$
1 - 0.99^{100} \approx 0.63
$$

p99이었던 게 p50이 된다.

논문에서 제시한 완화 기법은 다음과 같다.

- Hedged requests. 두 복제본에 동시 요청하고 빠른 쪽을 쓴다.
- Tied requests. 두 복제본에 보내되 한쪽이 시작하면 다른 쪽을 취소한다.
- Micro-partitioning. 파티션을 더 잘게 쪼개 hot spot을 흩뿌린다.
- Selective replication. 인기 데이터의 복제를 늘려 부하를 분산한다.

마이크로서비스, 검색, 추천처럼 팬아웃 패턴이 있는 곳에서는 컴포넌트의 p50 최적화가 거의 무의미하다. p99과 p999가 진짜 지표다.

### 3.7 Harvest와 Yield — Fox & Brewer, HotOS 1999

CAP의 0/1 선택을 연속적으로 본 관점이다.

- Yield. 완료된 요청의 비율. `completed / total`.
- Harvest. 응답에 반영된 데이터의 비율. `returned / available`.

검색 엔진에서 1000개 샤드 중 950개만 응답했지만 결과를 부분적으로 반환하면 Yield는 100%, Harvest는 95%다. CAP을 거스르지 않으면서도 사용자 경험을 살리는 방식이다.

적용할 수 있는지는 따로 판단해야 한다. 결제, 재고 차감, 원장 기록처럼 "모든 아니면 아무것도 아닌" 작업은 Harvest를 트레이드하기 어렵다. 검색, 추천, 통계, 로그 분석은 가능한 경우가 많다.

---

## 4. 컴포넌트 카드

각 카드는 보장 / 성능 / 워크로드 / 실패의 네 축으로 정리한다. 숫자는 벤치마크가 아니라 대략적인 order-of-magnitude다. 정확한 값은 워크로드와 하드웨어, 클라우드, 설정, 클라이언트 라이브러리에 따라 크게 달라진다. 여기서는 의사결정을 위한 자릿수 감각만 본다.

---

### Redis

- 보장. 단일 Redis primary 안에서 개별 명령은 이벤트 루프에 의해 직렬로 처리되므로, primary에 직접 붙은 클라이언트 관점에서는 단일 키 연산이 선형화 가능한 것처럼 동작한다. 다만 복제본 읽기, 페일오버, persistence 설정까지 포함하면 그 보장은 약해진다. Redis 복제는 비동기가 기본이라 복제본은 stale일 수 있고, `WAIT`도 Redis를 강한 일관성의 CP 시스템으로 바꾸지는 않는다.[^redis-replication] 내구성은 옵션이다. RDB(스냅샷), AOF(`appendfsync everysec`이면 최대 1초 손실, `always`면 매 쓰기마다 fsync로 ms 수준 비용).[^redis-persistence] 클러스터(cluster) 모드는 샤드 간(cross-shard) 트랜잭션을 지원하지 않는다.
- 성능. p50 50~200μs (single node, in-RAM). p99은 단일이면 약 1ms이고, persistence를 켜면 fsync 정책에 따라 튄다. 처리량은 core당 100k+ ops/s. 용량은 RAM이 천장이고, 클러스터로 수평 확장하면 TB급도 가능하지만 비용이 든다.
- 워크로드. point KV, 원자적 카운터(`INCR`), sorted set 리더보드, pub/sub, 집합 멤버십, 간단한 Lua. 약한 부분은 복잡 쿼리, 큰 range scan, 값에 대한 보조 인덱스(secondary index).
- 실패. master-replica 페일오버(failover)는 Sentinel/Cluster로 수초 안에 일어난다. 비동기 복제 때문에 복제 지연 window만큼 데이터 손실이 생길 수 있다. 메모리 압박이 오면 축출 정책(eviction policy, LRU/LFU)이 데이터를 조용히 버린다. 진실의 원천으로 쓰면 사고 난다.

한 줄로 정리하면, 빠른 휘발성 가속 계층. 절대 쓰면 안 되는 곳은 원장이나 주문 데이터 같은 SoR이다.

---

### MySQL

- 보장. InnoDB 기준 ACID. 기본 격리 수준은 `REPEATABLE READ`이고, 일관된 읽기는 트랜잭션의 첫 read가 만든 스냅샷을 본다. `SERIALIZABLE`도 가능하지만 락 비용 때문에 보통은 특수한 경우에만 쓴다. 단일 primary + binlog/redo log 기반 내구성이 기본이고, semi-sync replication이나 Group Replication으로 다중 노드 내구성을 보강한다.
- 성능. OLTP에서 p50 1~5ms, p99 10~50ms 자릿수. 읽기 위주 서비스에서는 읽기 복제본과 커넥션 풀 조합이 강하다. 쓰기 처리량은 단일 primary, hot index, 보조 인덱스 개수, fsync 정책에 묶인다.
- 워크로드. 전통적인 웹 OLTP, 관계형 모델, JOIN, point lookup, range scan, 보조 인덱스, foreign key. 약한 부분은 글로벌 스케일의 쓰기 위주 워크로드, 복잡한 분석 scan, 대형 테이블의 잦은 online schema change.
- 실패. 복제 지연, 페일오버 중 쓰기 중단, 스플릿 브레인(split-brain) 방지, schema migration 락이 운영 포인트다. MySQL 특유의 gap lock과 next-key lock, deadlock, long transaction이 꼬리 지연을 키울 수 있다.

웹 OLTP의 표준. 단순하고 빠른 관계형 SoR이 필요하면 여전히 강력하다. 다만 scale-out은 애플리케이션과 운영 설계가 같이 따라와야 한다.

---

### Postgres

- 보장. 트랜잭션 SERIALIZABLE이 가능하다. Cahill et al. 2008의 SSI가 MVCC 위에서 진짜 직렬화 가능성을 제공한다. 기본은 READ COMMITTED. 단일 노드 내구성은 WAL fsync로, 다중 노드 내구성은 `synchronous_commit=remote_apply` 같은 동기 복제로 확보한다. logical replication으로 cross-version, cross-DB도 가능하다.
- 성능. OLTP에서 p50 1~5ms, p99 10~50ms. 처리량은 단일 노드 기준 5k~30k tps 범위로 워크로드와 하드웨어에 따라 편차가 크다. 수직 스케일링이 기본이고, 수평 스케일링은 Citus, Patroni, sharding으로 직접 풀어야 한다.
- 워크로드. 관계, 트랜잭션, JOIN, 다양한 인덱스(B-tree, GIN, GiST, BRIN). JSONB도 지원한다. 약한 부분은 쓰기 처리량이 매우 큰 워크로드(단일 primary 한계)와 페타 스케일.
- 실패. streaming replication에 Patroni나 Pacemaker를 붙이면 자동 페일오버가 수초~분 단위로 가능하다. long-running transaction이 vacuum을 막아 bloat을 유발한다. 커넥션 폭주 시에는 fork 비용이 커지니 pgbouncer가 필수다.

SoR 후보 0순위. 수직 스케일링 천장에 닿기 전까지는 거의 항상 정답이다.

---

### Cassandra

- 보장. 튜닝 가능한 일관성(tunable consistency). `ONE/QUORUM/ALL`, $R+W>N$이면 쓰기 직후 읽기를 기대할 수 있다. 다만 hinted handoff, repair, 충돌 해결(conflict resolution) 같은 운영 현실이 끼어들기 때문에 수식 자체를 절대 보장으로 읽으면 안 된다. 기본은 결과적 일관성이고 PACELC로는 PA/EL이다. row-level만 원자적이고 multi-row 트랜잭션은 없다. LWT는 비싸니 신중하게 써야 한다.
- 성능. 쓰기는 매우 빠르다. LSM이고 log-structured라 p50이 ms 자릿수. 읽기는 QUORUM에서 p99 꼬리가 길어진다(컴팩션, repair). 수평 확장이 거의 선형에 가깝게 검증돼 있다(수백 노드). 용량은 본질적 제약이 없다.
- 워크로드. 시계열, 쓰기 위주, 기본 키 기반 접근. 파티션 키 + clustering 키 설계가 핵심이다. 약한 부분은 임시 쿼리, JOIN, 보조 인덱스, paging.
- 실패. gossip 기반이라 다수 노드 손실에도 견딘다. 파티션 시에는 AP. 운영 난이도가 높다(컴팩션, repair, tombstone).

쓰기 폭주, 키 기반 접근, 수평 확장이 필수인 경우. SQL과 비슷해 보이지만 NoSQL 마인드가 필수다.

---

### MongoDB

- 보장. 문서 단위 원자성이 기본이다. multi-document 트랜잭션도 지원하지만 트랜잭션의 read/write concern과 primary routing 조건을 이해해야 한다. default read concern이 `local`이라 롤백될 수 있는 데이터를 읽을 수 있고, 일반 replica set의 implicit default write concern은 대체로 `w: majority`다(arbiter 구성은 예외).[^mongodb-defaults] secondary read preference는 stale read를 만들 수 있으므로 `maxStalenessSeconds` 같은 옵션까지 같이 봐야 한다.[^mongodb-read-preference] 인과 일관성은 session + majority read/write concern 조합에서 의미가 있다.
- 성능. single-document CRUD는 ms 자릿수로 빠르다. 보조 인덱스와 작업 셋이 메모리에 잘 맞으면 OLTP 성능도 좋다. multi-document 트랜잭션, scatter-gather 쿼리, 큰 문서 업데이트는 p99 꼬리를 키운다.
- 워크로드. 문서 모양, 중첩 데이터, 유연한 스키마, aggregate pipeline, 이벤트/문서 저장소, product/catalog/profile 데이터. 약한 부분은 강한 관계형 제약, 복잡 JOIN, 고빈도 cross-document 트랜잭션, 무계획 schema drift.
- 실패. replica set election 중에는 쓰기가 일시 중단된다. read preference에 따라 stale secondary 읽기가 발생할 수 있다. 샤드 키를 잘못 잡으면 jumbo chunk, hot shard, scatter-gather 쿼리로 성능이 무너진다.

문서 모델이 도메인 모양과 맞을 때 생산성이 높다. 다만 "스키마가 없다"가 아니라 "스키마를 애플리케이션이 책임진다"에 가깝다.

---

### DynamoDB

- 보장. 결과적 일관성 읽기가 기본이다. 강한 일관성 읽기는 table과 LSI에서만 지원되고 결과적 일관성 읽기 대비 2배 비용이 든다.[^dynamodb-read-consistency] 트랜잭션(TransactWrite/TransactGet)은 최대 100개 action/items에 총 4MB 제한이 있고 비용은 2배다.[^dynamodb-transactions] 다중 리전 active-active(global tables)는 결과적 일관성이다.
- 성능. p99이 리전 내에서 single-digit ms. throughput은 provisioned 또는 on-demand. hot partition을 조심해야 한다. 단일 파티션 키의 천장은 약 1000 WCU / 3000 RCU.
- 워크로드. KV, 단순 보조 인덱스(GSI/LSI). 단일 키 접근 패턴이면 거의 무한에 가깝게 확장된다. 약한 부분은 임시 쿼리, 전문 검색, 분석.
- 실패. 매니지드, 3 AZ 복제, 99.999% SLA(글로벌 테이블). 추상화가 잘 돼 있어 직접 운영 부담이 거의 없다. 함정은 비용 모델이다. scan은 비싸고, hot partition은 throttle을 당한다.

AWS-native KV의 표준. 접근 패턴이 명확할 때 강력하다.

---

### Spanner

- 보장. External consistency, 즉 엄격한 직렬화 가능성을 글로벌하게 제공한다. TrueTime API(GPS + atomic clock의 시간 불확실성 $\varepsilon$)로 commit 타임스탬프를 결정한다. 다중 리전 트랜잭션이 가능하다.
- 성능. 단일 리전 읽기는 5~10ms 정도. 다중 리전 commit은 50~100ms(Paxos round + TrueTime wait). 처리량은 split 단위로 수평 확장된다. 용량은 사실상 무한이다(Google이 EB 스케일로 운영한다).
- 워크로드. 글로벌 SQL OLTP. interleaved table로 join locality를 확보한다. 약한 부분은 단일 row에 쓰기가 매우 빈번한 경우(split 안에 묶인다)와 sub-ms 수준의 latency-critical 워크로드.
- 실패. Paxos 그룹으로 리전 손실까지 견딘다(다중 리전 구성). RPO≈0, RTO는 수초~수분.

글로벌 강한 일관성 SQL이 필요한 드문 경우의 정답. 지연 비용을 감수할 수 있을 때만 의미가 있다.

---

### Kafka

- 보장. 파티션 단위 정렬 로그. 파티션 간 순서는 보장하지 않는다. 내구성은 `acks` 설정(0/1/all)과 ISR 수에 따른다. `acks=all` + `min.insync.replicas=2` + `replication.factor=3`이 표준 안전 설정. exactly-once는 idempotent producer와 트랜잭션으로 내부적으로 가능하지만, end-to-end exactly-once는 consumer 멱등성(idempotency)이 필요하다.
- 성능. producer p99이 `acks=all`, 리전 내에서 약 10ms. 처리량은 broker당 수십 MB/s ~ GB/s(메시지 크기, 배치, 압축에 따라). consumer는 pull 기반.
- 워크로드. 이벤트 로그, 스트림, 팬아웃, replay. 약한 부분은 random key 접근, 현재 상태 쿼리(KTable/materialized view로 풀어야 한다), 작은 메시지가 매우 많을 때의 metadata 오버헤드.
- 실패. ISR이 줄어들면 가용성과 일관성이 트레이드된다(`unclean.leader.election`). consumer lag이 silent failure의 주범이다. retention 만료 시 데이터는 영구 손실된다.

"현재 상태"가 아니라 "변화의 흐름"을 다루는 시스템의 척추. 저장소처럼 보이지만 시간순 로그다.

---

### S3 (호환 object storage 포함)

- 보장. 2020년 12월부터 모든 리전에서 강한 쓰기 직후 읽기 일관성을 제공한다. PUT/overwrite/delete 이후 GET/LIST가 최신 상태를 반영한다.[^s3-consistency] 11 nines 내구성 — 다중 AZ erasure coding. object는 partial update가 아니라 whole-object overwrite 단위이며, versioning을 켜면 PUT이 새 version을 만든다.
- 성능. GET first-byte 지연은 수십 ms 수준. per-prefix throughput은 최소 5,500 RPS(GET) / 3,500 RPS(PUT)이며 prefix 개수에는 제한이 없다.[^s3-performance] 용량 무제한. Glacier는 cold storage.
- 워크로드. blob, 백업, 정적 자원, Parquet 위에 Athena/Spark로 만든 데이터 레이크. 약한 부분은 hot small-key 워크로드, append(multipart upload는 됨), 진짜 random write(object 단위만).
- 실패. 매니지드, 리전 단위 outage는 드물지만 발생한다. cross-region replication(CRR)으로 DR이 가능하다.

가장 저렴한 무한 durable storage. 데이터 레이크와 백업의 기본기.

---

### Elasticsearch

- 보장. Near real-time이다. refresh interval이 기본 1초. 같은 샤드 안에서도 결과적 일관성. translog로 내구성을 확보한다. 쿼럼 스타일 쓰기(`wait_for_active_shards`). 스플릿 브레인은 zen2와 7.x 이후 완화됐다.
- 성능. 쿼리는 복잡도에 따라 10~100ms. bulk indexing 처리량은 좋다. aggregation은 cardinality와 샤드 수에 크게 좌우된다.
- 워크로드. 전문 검색, 로그와 observability, 다차원 aggregation, geo, vector. 약한 부분은 SoR(재색인이 가능해야 한다), 복잡 트랜잭션, 강한 일관성.
- 실패. 샤드 재분배가 부하 spike를 유발한다. 메모리 압박(heap, fielddata)이 가장 흔한 운영 이슈. 버전 호환성이 깐깐하다.

검색 DB이지 진실 DB가 아니다. SoR을 따로 두고 색인은 파생 뷰로 둬야 한다.

---

### CDN (CloudFront, Fastly, Cloudflare 같은)

- 보장. TTL 기반 결과적 일관성. immutable object identity(URL + ETag). stale-while-revalidate로 가용성을 우선한다.
- 성능. edge p50이 single-digit ms. 캐시 미스 시에는 origin shield를 거쳐 origin RTT.
- 워크로드. static, cacheable, immutable. API GET 응답도 적절한 TTL이면 가능하다. 약한 부분은 개인화, 실시간, write.
- 실패. stale 응답으로 우아한 성능 저하가 자연스럽다. origin 장애도 캐시 히트율(hit ratio)만큼은 영향이 없다.

사용자 가까이 있는 빠른 거짓말쟁이. 빠르지만 진실 반영은 느리다.

---

### SQLite

- 보장. ACID, single-writer 직렬화 가능(WAL 모드 포함). file-as-DB.
- 성능. 로컬 in-process, μs 자릿수. 처리량은 single-writer가 천장이다.
- 워크로드. embedded, 단일 앱, 작은~중간 크기 데이터셋, 분석/보고서(CTE, window 함수 지원이 좋다). 약한 부분은 동시 writer와 네트워크 접근. LiteFS, Cloudflare D1 같은 wrapper로 보강할 수는 있다.
- 실패. 파일 손상이 주된 실패 모드다. corruption은 매우 드물지만 발생할 수 있으니 백업이 필수다.

"단일 프로세스에서 충분"이라는 조건만 맞으면 거의 항상 옳은 선택. Hipp이 만든, 세상에서 가장 많이 배포된 DB다.

---

### DuckDB

- 보장. ACID 단일 프로세스. 컬럼 지향, vectorized execution. Parquet, CSV, Arrow를 native로 읽는다.
- 성능. 단일 노드에서 GB/s 스캔. 분석 쿼리는 초~수십 초.
- 워크로드. 임시 분석, embedded 분석, 데이터 사이언스. 약한 부분은 OLTP, 동시 writer, 분산.
- 실패. 프로세스 로컬이라 디스크 손상이 주된 실패 모드다.

"분석용 SQLite". S3 + Parquet + DuckDB는 작은 팀이 갖출 수 있는 무서운 데이터 스택이다.

---

## 5. 결정 트리: 네 단계 압축

새 워크로드나 새 컴포넌트를 고를 때 빠르게 돌려 보는 절차다.

### Step 1. 보장 먼저

"깨지면 사고로 직결되는 보장은 무엇인가?"

- 선형화 가능한 코디네이션이 필요하면 Spanner, etcd, ZooKeeper, FoundationDB가 후보다. Cassandra와 DynamoDB의 결과적 일관성은 탈락.
- multi-row 트랜잭션이 필요하면 SQL 계열, Spanner, FoundationDB가 후보다. NoSQL 다수가 탈락.
- 내구성 사고 시 데이터 손실을 허용할 수 없으면 SoR로 지정한다. Redis, Elasticsearch, CDN은 후보가 아니다.
- 부분 손실이 OK면(캐시, 검색 인덱스, 로그 분석 같은 것) 파생 뷰 후보로 자유롭게 쓴다.

이 단계에서 후보의 절반 정도가 빠진다.

### Step 2. 워크로드 모양

"이 데이터와 쿼리는 어떤 모양인가?"

- Point KV — KV store (Redis, DynamoDB, MongoDB, Postgres도 OK)
- Range scan과 정렬 — B-tree나 LSM (Postgres, Cassandra, MySQL)
- 전문 검색과 fuzzy — Elasticsearch, OpenSearch, Vespa
- 많은 행 위에 aggregation — 컬럼 지향 (ClickHouse, Druid, BigQuery, DuckDB)
- Append-only 이벤트 로그 — Kafka, Pulsar
- Blob — S3
- 그래프 탐색 — Neo4j, Dgraph, JanusGraph
- 벡터 유사도 — Faiss/HNSW, pgvector, Qdrant, Weaviate

### Step 3. 실패 모델

"어떤 장애를 견뎌야 하는가? RPO/RTO는?"

- 단일 노드면 충분 — SQLite, 단일 Postgres
- AZ 손실까지 — 다중 AZ (RDS Multi-AZ, DynamoDB, Spanner zonal config)
- 리전 손실까지 — 다중 리전 (S3 CRR, Spanner multi-region, Aurora Global, Cassandra multi-DC)
- 소프트웨어 버그 상관관계까지 — 다른 소프트웨어/버전으로 백업. 실전에서 자주 무시되지만 가장 위험한 카테고리다.

### Step 4. 지연 예산

"p99이 얼마면 사용자가 행복한가? 혹은 SLA를 충족하는가?"

이 숫자가 PACELC의 E 선택을 결정한다.

| p99 예산 | 의미 |
|---|---|
| 1ms 미만 | 로컬 메모리. 같은 host의 Redis, in-process 캐시, SQLite, embedded. 다른 host로 가면 끝. |
| 10ms 미만 | 같은 AZ 동기 복제. 보통의 OLTP. AZ 간은 빠듯하다. |
| 100ms 미만 | AZ 간 동기 OK. 리전 간 읽기 OK (쓰기는 빠듯). |
| 1s 미만 | 리전 간 동기가 가능하다(Spanner). 사용자가 체감하는 한계. |
| 1s 이상 | 분석과 batch. 일관성과 지연 부담이 거의 없다. |

이 단계에서 "리전 간 강한 일관성을 sub-10ms로" 같은 비현실적 요구가 걸러진다. 광속이 허락하지 않는다.

---

## 6. 자주 쓰는 한 줄 요약

> 컴포넌트를 처음 볼 때 던지는 질문은 단 하나다.
>
> "이건 진실로 믿을 건가, 빠르게 보여줄 건가, 이동시킬 건가, 계산할 건가?"
>
> 분류한 다음에는 이 질문이다.
>
> "이게 깨졌을 때 데이터가 사라져도 되는가?"
>
> 답이 NO이면 SoR이고, YES이면 파생/이동/연산이다. SoR이 아닌 것을 SoR로 쓰면 사고가 난다.

---

## 부록: 참고 문헌

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
- Kleppmann, M. *Designing Data-Intensive Applications*. O'Reilly, 2017. (위 이론들을 실무 관점에서 통합한 정리)
- Bailis, P., Davidson, A., Fekete, A., Ghodsi, A., Hellerstein, J., Stoica, I. *Highly Available Transactions: Virtues and Limitations*. VLDB 2014.
- Corbett, J. C. et al. *Spanner: Google's Globally Distributed Database*. OSDI 2012.
- DeCandia, G. et al. *Dynamo: Amazon's Highly Available Key-Value Store*. SOSP 2007.
- Kraska, T. et al. *The Case for Learned Index Structures*. SIGMOD 2018.
- AWS. *DynamoDB read consistency*. 공식 문서.
- AWS. *DynamoDB Transactions: How it works*. 공식 문서.
- AWS. *Amazon S3 Strong Consistency*. 공식 문서.
- Redis. *Redis replication*. 공식 문서.
- Redis. *Redis persistence*. 공식 문서.
- Oracle. *MySQL Reference Manual: InnoDB Transaction Isolation Levels*. 공식 문서.
- Oracle. *MySQL Reference Manual: InnoDB Transaction Model*. 공식 문서.
- MongoDB. *Default MongoDB Read Concerns/Write Concerns*. 공식 문서.
- MongoDB. *Read Preference*. 공식 문서.
- MongoDB. *Transactions*. 공식 문서.

---

*마지막 한 마디.* 이 문서를 가장 잘 활용하는 방법은 단순하다. 새 컴포넌트를 만나면 §4의 카드 형식으로 직접 한 장 채워 보라. 채울 수 없는 칸이 있다면 그 컴포넌트에 대해 모른다는 뜻이다. 그 빈칸이 곧 다음 공부거리다.

[^dynamodb-read-consistency]: AWS 공식 문서에 따르면 DynamoDB의 결과적 일관성 읽기는 기본값이며, 강한 일관성 읽기는 table과 LSI에서만 지원된다. 비용도 다르다. 4KB 이하 item 기준 강한 일관성 읽기는 1 RCU, 결과적 일관성 읽기는 0.5 RCU를 소비한다. GSI와 stream read는 결과적 일관성만 지원한다. <https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/HowItWorks.ReadConsistency.html>

[^dynamodb-transactions]: DynamoDB `TransactWriteItems`/`TransactGetItems`는 최대 100개 action/items를 하나의 all-or-nothing operation으로 묶을 수 있고, aggregate item size는 4MB를 넘을 수 없다. 예전 자료에는 25개 제한으로 적힌 경우가 있으나 현재 공식 문서는 100개로 안내한다. <https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/transaction-apis.html>

[^s3-consistency]: Amazon S3는 2020년 12월부터 모든 region에서 PUT, overwrite, delete 이후 GET/LIST에 대해 strong read-after-write consistency를 제공한다. <https://aws.amazon.com/s3/consistency/>

[^s3-performance]: AWS는 S3 성능이 prefix 단위로 scaling되며 prefix당 최소 3,500 PUT/s, 5,500 GET/s를 지원하고, prefix 수에는 제한이 없다고 설명한다. <https://aws.amazon.com/s3/consistency/>

[^redis-replication]: Redis 공식 문서는 Redis Open Source 복제가 기본적으로 asynchronous replication이며, `WAIT`가 acknowledged copy 수를 늘릴 수는 있지만 Redis 인스턴스 집합을 강한 일관성의 CP 시스템으로 바꾸지는 않는다고 설명한다. <https://redis.io/docs/latest/operate/oss_and_stack/management/replication/>

[^redis-persistence]: Redis 공식 문서는 AOF의 `appendfsync always`, `everysec`, `no` 정책을 구분한다. `everysec`는 빠른 편이지만 장애 시 약 1초 데이터 손실을 허용하고, `always`는 더 안전하지만 매우 느릴 수 있다. <https://redis.io/docs/latest/operate/oss_and_stack/management/persistence/>

[^mongodb-defaults]: MongoDB 공식 문서 기준으로 primary/secondary read의 기본 read concern은 `local`이며, 이 값은 majority에 기록되지 않아 rollback될 수 있는 데이터를 반환할 수 있다. MongoDB 5.0 이후 implicit default write concern은 대체로 `w: majority`지만 arbiter 구성에서는 예외가 있다. <https://www.mongodb.com/docs/upcoming/reference/mongodb-defaults/>

[^mongodb-read-preference]: MongoDB 공식 문서는 `primary`를 제외한 read preference가 stale data를 반환할 수 있다고 설명한다. secondary 계열 read preference에서는 `maxStalenessSeconds`로 지나치게 오래된 secondary를 피할 수 있다. <https://www.mongodb.com/docs/v7.0/core/read-preference/>
