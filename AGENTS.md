# AGENTS.md

k8slab은 강의와 책의 Vagrantfile을 그대로 읽어 VirtualBox에 쿠버네티스 실습 환경을 만드는 도구다. 베이그런트(Vagrant)가
하던 일(VM과 네트워크 생성, 프로비저닝 스크립트 실행)만 한다. Rust로 작성하고 Windows와 macOS용 실행 파일 하나로 배포한다.

## 범위

- 대상은 sysnet4admin의 쿠버네티스 강의 저장소 4개, 『요즘 개발자를 위한 쿠버네티스/도커』(2판) 저장소, 연구 저장소
  (`sysnet4admin/Research`)의 시험 클러스터다. 목록은 `tools/sync-fixtures.sh`의 `REPOS`에 있다. 앤서블 강의와 책은
  대상이 아니다.
- 공개하지 않은 Vagrantfile은 시험 자료로 복사하지 않고 `tools/check-dir.sh <폴더>`로 그 자리에서 비교한다.
- Ubuntu box(`sysnet4admin/Ubuntu-k8s`)만 지원한다. CentOS box는 지원하지 않는다. 다른 OS가 필요해지면 그때 Rocky나
  Fedora를 더한다.
- 하이퍼바이저는 VirtualBox만 지원한다.
- 하위 명령과 옵션은 베이그런트와 같게 둔다. 수강생은 영상의 `vagrant up`에서, 연구 하네스는 스크립트의 `vagrant`에서
  프로그램 이름만 바꾸면 되게 하기 위해서다.

## 현재 상태와 다음 할 일 (2026-11-20)

### 옮길지는 베이그런트의 추이를 보고 정한다

k8slab을 만든다고 강의와 책을 당장 모두 옮기는 것이 아니다. 베이그런트의 추이를 보고 정한다(저자 판정). 볼 것은 새 릴리스(MIT
릴리스, Ruby 3.4.11 이상), macOS 26.7 이상에서 `vagrant up`이 실패하는 문제(hashicorp/vagrant#13845), 다음 VirtualBox 버전 지원,
베이그런트에 들어 있는 Ruby 3.3의 지원 종료(2027-03-31)다. 다만 저자는 옮길 가능성이 높다고 본다. 베이그런트에 대한 신뢰가 이미
한 번 꺾였고 기존에도 베이그런트의 사용자 경험이 좋지 않았기 때문이다. 수강생 질문과 답변을 분석해 이 판단에 보탠다.

### 된 것

- Vagrantfile 해석기: 대상 Vagrantfile 45개를 arm64, x86_64 각각 Ruby 실행 결과와 같게 읽는다(90건)
- box 빌드(`box/`): amd64 2분 23초, arm64 2분 54초, 각 약 670 MB. arm64 box로 starter.kit 3~7장 전체 실습과 재시작을 확인했다
- 워크플로: `ci.yml`(리눅스, macOS, Windows 시험), `release.yml`(실행 파일), `box.yml`(publish 없이 시험 실행 통과)

### 다음 순서

1. k8slabfile 형식(TOML 문법)과 `convert`(Vagrantfile을 k8slabfile로), `convert --check`(k8slabfile과 Vagrantfile이 같은 결과를 내는지)
2. `up`: box 받기와 캐시, 한 번 가져와 연결 복제, 대역마다 호스트 전용 네트워크(192.168.1.0/24, 192.168.2.0/24), NAT와 SSH 포워딩
   (포트가 겹치면 베이그런트의 auto_correct처럼 바꾸고 알린다), 대역 충돌 검사, 내장 SSH, 게스트 고정 IP(netplan, MAC으로 찾기),
   프로비저닝(셸의 path, inline, args, env, reboot와 파일 복사)
3. `halt`, `destroy`, `status`, `ssh`(`-c`), `snapshot save/restore/list`
4. `v0.1.0` 릴리스와 Windows 확인(저자가 Windows PC에서 확인)

`up`의 VirtualBox 동작은 `tools/reference/k8s-lab.sh`를 따른다. 베이그런트 없이 VM 4대, 고정 IP, MetalLB L2 접속, 재시작까지
macOS(Apple Silicon)에서 확인한 셸 시제품이다. 확인한 것은 이렇다: box를 한 번 가져와 스냅숏을 만들고 연결 복제한다, macOS
VirtualBox 7은 `hostonlynet`(Linux는 `hostonlyif`)을 사용한다, NIC2의 MAC을 IP마다 고정하고 netplan이 MAC으로 찾는다,
`clonevm --groups`는 반영되지 않아 `modifyvm --groups`로 지정한다, `ssh`가 반복문의 표준 입력을 읽지 않게 한다.

### 알아 둘 것

- arm64에서는 `if ARCH == "arm64"` 분기 때문에 VMware 설정만 남는 VM이 12대 있다(prom 강의의 harbor 실습 등). VirtualBox 설정이
  없으면 VMware 설정의 CPU와 메모리를 사용한다.
- define 없이 box만 지정한 Vagrantfile(2판 책 2.1.3)은 "default" VM이다. 포워딩 설정이 없으므로 베이그런트처럼 SSH를 2222번에서
  시작해 빈 포트를 찾는다.
- Windows에서 호스트 전용 어댑터를 만들 때는 관리자 권한(UAC)이 필요하다.

### 저자가 할 일

- box 게시용 secret 4개(`R2_ACCOUNT_ID`, `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`, `VG_BOX_TOKEN`). 방법은 `.github/workflows/box.yml`
  맨 위에 있다.
- box 신규 버전(1.1.0)의 첫 게시는 모든 수강생과 독자가 받으므로 amd64 box를 x86 기기에서, 2판 책 실습과 다른 강의 저장소를
  확인한 뒤 정한다.

## 만드는 범위

- 지금 목적(강의와 책, 연구 시험 클러스터)에 필요한 것만 만든다. 새 목적이 생기면 그때 이 저장소에 기능을 더한다. 미리
  일반화하지 않는다(2026-11-19 저자 판정).
- 첫 버전의 명령은 `up`, `halt`, `destroy`, `ssh`(`-c` 포함), `status`, `snapshot save/restore/list`, `convert`(Vagrantfile을
  k8slabfile로 바꾼다)다.

## 설정 파일

- k8slab의 설정 파일은 k8slabfile이다. 베이그런트와 Vagrantfile은 중장기적으로 신뢰하기 어려워 언젠가 없앨 대상이다.
  Vagrantfile 해석은 저장소들이 k8slabfile로 옮겨 가는 동안만 사용하는 호환 기능이다.
- 실습 폴더마다 `k8slabfile` 하나로 관리한다(2026-11-19 저자 판정). 내용은 TOML 문법으로 작성한다. 다른 이름이나 별도
  설정 파일을 두지 않는다.
- 폴더에 k8slabfile과 Vagrantfile이 모두 있으면 k8slabfile을 먼저 읽는다. 둘 중 1개만 있으면 그 파일을 읽는다.
- 지금 있는 Vagrantfile은 따로 정하지 않으면 그대로 둔다. 강의 영상이 Vagrantfile 기준이라 지우면 수강생이 혼란스러울 수
  있고 영상을 더 찍을지도 정해지지 않았다(2026-11-19 저자 판정). 언제 지울지는 저장소마다 상황에 따라 정한다.
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

## 구성

| 위치 | 내용 |
|---|---|
| `src/` | k8slab 명령과 Vagrantfile 해석기 |
| `tools/` | 정답 생성기(`oracle.rb`), 시험 자료 스크립트, `up` 구현의 참고인 셸 시제품(`reference/k8s-lab.sh`) |
| `box/` | `sysnet4admin/Ubuntu-k8s` box 빌드와 게시. 클라우드 이미지를 chroot로 설정하고 VM을 띄우지 않는다(`box/README.md`) |
| `.github/workflows/ci.yml` | 리눅스, macOS, Windows에서 시험하고 정답이 oracle과 같은지 확인한다 |
| `.github/workflows/release.yml` | `v*` 태그에서 macOS(arm64, x86_64)와 Windows 실행 파일을 Releases에 올린다 |
| `.github/workflows/box.yml` | box를 빌드한다. `publish`를 선택하면 R2와 vg-box/k8s에 올리고 카탈로그에 신규 버전을 더한다 |

box를 받는 곳(`box.kuberneteslab.dev`의 R2, `vg-box/k8s`의 Releases와 Pages)은 이 저장소 밖에 있다. 이 주소는 강의 저장소와
책에 이미 들어가 있어서 바꾸지 않는다.

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
