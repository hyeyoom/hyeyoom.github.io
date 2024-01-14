# scp (secure copy)

# 기본 사용법

```bash
# file 전송
scp [옵션] [파일경로] [계정@서버주소:대상경로]
# directory 전송
scp -r [디렉토리] [계정@서버주소:대상경로]
```

실제 예제

```bash
scp app.jar ec2-user@awsdomain:/home/ec2-user/
```

반대로 서버에서 내 로컬로 파일을 가져오고 싶다면, 다음과 같이 하면 된다.

```bash
scp ec2-user@awsdomain:/home/ec2-user/app.jar .
```

# 옵션

- `-r`: 디렉토리 전송
- `-i`: 인증키 파일 지정
- `-P`: 포트 지정