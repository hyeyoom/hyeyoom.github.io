# 벨레이디의 모순

벨레이디의 모순은 저장할 수 있는 프레임의 크기를 늘리면 오히려 페이지 폴트가 증가하는 현상을 의미한다. 페이지 교체 알고리즘으로 FIFO를 사용할 때 주로 발생한다.

다음과 같이 페이지를 요청한다고 가정해보자.

```python
page_references = [3, 2, 1, 0, 3, 2, 4, 3, 2, 1, 0, 4]
```

그리고 FIFO의 사이즈를 3에서 4로 바꾸면 페이지 폴트 수가 감소해야할 것 같지만 역설적으로 증가한다.

다음은 벨레이디의 모순을 시뮬레이션 할 수 있는 간단한 코드이다.  

```python
from collections import deque


class Fifo:

    def __init__(self, init_size: int):
        self.queue = deque(maxlen=init_size)
        self.count_of_page_fault = 0
        self.init_size = init_size

    def request_page(self, page_num: int):
        if page_num not in self.queue:
            self.queue.append(page_num)
            self.count_of_page_fault += 1

    def bulk(self, page_references: [int]) -> int:
        for page in page_references:
            self.request_page(page)
        return self.count_of_page_fault


def simulate_beladys_anomaly():
    page_references = [3, 2, 1, 0, 3, 2, 4, 3, 2, 1, 0, 4]

    for n in range(3, 6):
        fifo = Fifo(n)
        page_faults = fifo.bulk(page_references)
        print(f"Size of Fifo: {fifo.init_size}, Page Faults: {page_faults}")


if __name__ == "__main__":
    simulate_beladys_anomaly()
```

결과는 다음과 같다.  

```bash
Size of Fifo: 3, Page Faults: 9
Size of Fifo: 4, Page Faults: 10
Size of Fifo: 5, Page Faults: 5
```

뭐.. 뭐노..  

그런데 FIFO 같은 단순한 알고리즘보다는 LRU, LFU, 2Q와 같은 고오급 알고리즘을 사용하면 폴트가 감소하는 경향을 보이니 FIFO를 버리고 최소 LRU 같은 친구들을 사용하자.  