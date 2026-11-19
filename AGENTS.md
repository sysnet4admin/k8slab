# AGENTS.md

k8slab은 강의와 책의 Vagrantfile을 그대로 읽어 VirtualBox에 쿠버네티스 실습 환경을 만드는 도구다. 베이그런트(Vagrant)가
하던 일(VM과 네트워크 생성, 프로비저닝 스크립트 실행)만 한다. Rust로 작성하고 Windows와 macOS용 실행 파일 하나로 배포한다.

## 범위

- 대상은 sysnet4admin의 쿠버네티스 강의 저장소 4개, 『요즘 개발자를 위한 쿠버네티스/도커』(2판) 저장소, 연구 저장소
  (`sysnet4admin/Research`)의 시험 클러스터다. 목록은 `tools/sync-fixtures.sh`의 `REPOS`에 있다. 앤서블 강의와 책은
  대상이 아니다.
- 공개하지 않은 Vagrantfile은 시험 자료로 복사하지 않고 `tools/check-dir.sh <폴더>`로 그 자리에서 비교한다.
- Ubuntu box(`sysnet4admin/Ubuntu-k8s`)만 지원한다. CentOS box는 지원하지 않는다. 다른 OS를 더한다면 Rocky나 Fedora이고
  게스트 OS마다 다른 부분(고정 IP 설정)만 따로 둔다.
- 하이퍼바이저는 VirtualBox만 지원한다.
- 명령은 강의 자료가 사용하는 `up`, `halt`, `destroy`부터 만들고 `ssh`, `status`를 더한다. 동사는 베이그런트와 같게 둔다.
  수강생이 영상의 `vagrant up`에서 프로그램 이름만 바꾸면 되게 하기 위해서다.

## 설정 파일

- k8slab의 설정 파일은 k8slabfile이다. 베이그런트와 Vagrantfile은 중장기적으로 신뢰하기 어려워 언젠가 없앨 대상이다.
  Vagrantfile 해석은 저장소들이 k8slabfile로 옮겨 가는 동안만 사용하는 호환 기능이다.
- 실습 폴더마다 `k8slabfile` 하나로 관리한다(2026-11-19 저자 판정). 내용은 TOML 문법으로 작성한다. 다른 이름이나 별도
  설정 파일을 두지 않는다.
- 폴더에 k8slabfile과 Vagrantfile이 모두 있으면 k8slabfile을 읽는다. 둘 중 1개만 있으면 그 파일을 읽는다. 옮기기가 끝난
  폴더에서는 Vagrantfile을 지운다.
- Vagrantfile 해석기는 키우지 않는다. Ruby 함수처럼 해석기가 다루지 않는 문법을 사용하는 프로젝트는 k8slabfile로 옮긴다.
- 내부 모델은 베이그런트의 개념(provider, forwarded_port 등)이 아니라 k8slab의 개념(노드, IP, SSH 포트, 디스크, 프로비저닝)으로
  둔다. Vagrantfile은 이 모델로 바꿔서 읽는다. 그래야 Vagrantfile 해석을 지워도 나머지가 그대로 남는다.

## Vagrantfile 해석(호환용)

- `src/vagrantfile/`는 Ruby 전체가 아니라 대상 Vagrantfile이 사용하는 문법만 해석한다(변수, 문자열 치환, 범위 잘라내기,
  `each` 반복, `if`, 해시, `` `uname -m` ``).
- 정답은 `tools/oracle.rb`다. Vagrantfile을 실제 Ruby로 실행하되 베이그런트 대신 설정 호출만 기록한다. `tests/fixtures.rs`가
  Rust 해석 결과와 정답을 Vagrantfile마다, arm64와 x86_64 각각 비교한다.
- 대상 Vagrantfile이 바뀌면 `tools/sync-fixtures.sh`로 시험 자료를 다시 만들고 `cargo test`를 실행한다. 새 문법이 나오면
  `tools/oracle.rb`와 `src/vagrantfile/`를 같은 규칙으로 함께 수정한다. 한쪽만 수정하지 않는다.
- define 없이 `config.vm`에 바로 설정하면 베이그런트처럼 "default" VM 하나로 본다. define과 함께 사용하는 경우는 지원하지 않는다.

## 확인

```bash
cargo test
tools/sync-fixtures.sh   # 대상 저장소에서 Vagrantfile과 정답을 다시 만든다(ruby, git 필요)
cargo run -- plan -f <Vagrantfile> --arch arm64
```

## 그 밖의 규칙

- 이 저장소는 공개 저장소다. 비공개 저장소 이름과 로컬 절대 경로를 파일과 커밋 메시지에 넣지 않는다.
- 문서와 코드 주석은 한국어로 작성한다. 커밋 메시지는 한국어로 작성하고 AI 어트리뷰션 트레일러를 넣지 않는다.
- 강의 영상에 나오는 명령과 결과가 달라지면 안 된다. 실습 저장소의 동작을 바꾸는 우회는 k8slab 안에서 처리한다.
