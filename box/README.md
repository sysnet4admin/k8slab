# box 만들기

Ubuntu 24.04 공식 클라우드 이미지를 받아 chroot로 설정하고 VirtualBox box로 묶는다. VM을 띄우지 않으므로 KVM이 없는 GitHub
Actions arm64 러너에서도 에뮬레이션 없이 amd64와 같은 방식으로 만들 수 있다. Packer와 VirtualBox도 필요 없다.

게시된 `sysnet4admin/Ubuntu-k8s` 1.0.0은 Packer와 bento로 만들었다([IaC의 bento](https://github.com/sysnet4admin/IaC/tree/main/bento)).
arm64는 GitHub Actions에서 VM으로 설치하면 3시간 19분이 걸려서 로컬 Mac에서 빌드했다. 이 폴더의 방식은 amd64와 arm64를
모두 Actions에서 3분 안에 만든다. 2026-11에 IaC의 시제품 브랜치에서 이 저장소로 옮겼다.

| 파일 | 하는 일 |
|---|---|
| `build.sh` | 이미지 받기, SHA-256 확인, 64 GiB로 늘리기, 마운트, chroot, VMDK 변환, box로 묶기 |
| `customize.sh` | chroot 안에서 1.0.0과 같은 설정을 넣는다. 항목마다 bento의 어느 스크립트가 하던 일인지 적었다 |
| `template/<아키텍처>/` | 1.0.0 box의 `box.ovf`, `Vagrantfile`, `metadata.json`. 디스크 파일 이름과 크기(64 GiB)를 그대로 사용한다 |
| `publish_catalog.py` | 카탈로그(JSON)에 신규 버전을 더한다. 기존 버전은 남긴다 |
| `../.github/workflows/box.yml` | amd64, arm64를 각 아키텍처 러너에서 만든다. `publish`를 선택하면 R2와 vg-box/k8s Releases에 올리고 카탈로그 2곳(R2, Pages)에 신규 버전을 더한다 |

## 1.0.0과 비교

| 항목 | 1.0.0(설치 프로그램) | 이 방식(클라우드 이미지) |
|---|---|---|
| 디스크 | LVM 루트 30.5 GB, `/boot` 2 GB, EFI 1 GB | 클라우드 이미지의 파티션(루트, `/boot`, EFI)에서 루트만 디스크 끝까지 늘린다. LVM 없음 |
| 커널 패키지 | linux-generic | 이미지의 linux-virtual을 linux-generic으로 바꾼다 |
| SSH 호스트 키 | 빌드할 때 만든 키를 모든 VM이 함께 사용 | VM마다 첫 부팅에서 만든다 |
| cloud-init | 설치 뒤 꺼짐 | 꺼 둔다 |

## 시험 결과 (2026-11-19)

GitHub Actions에서 amd64와 arm64를 동시에 만들었다(클라우드 이미지 serial 20261017 기준).

| 항목 | arm64(`ubuntu-24.04-arm`) | amd64(`ubuntu-24.04`) |
|---|---|---|
| `build.sh` 전체 | 2분 54초 | 2분 23초 |
| 그중 chroot 설정 | 86초 | 53초 |
| box 크기 | 674,726,156 bytes | 668,561,500 bytes |
| 1.0.0 box 크기 | 968,788,037 bytes | 973,781,504 bytes(VMDK) |

같은 arm64를 VM으로 설치하면 3시간 19분이 걸렸다. 커널은 6.8.0-149-generic, Ubuntu는 24.04.5다.

arm64 box를 Apple Silicon Mac(macOS 26.5.2, VirtualBox 7.2.8, 베이그런트(Vagrant) 2.4.9)에서 확인했다. amd64 box는 아직 x86 기기에서
확인하지 않았다.

- 부팅 9.4초, 실패한 서비스 없음. `eth0`, `eth1`, 호스트 이름, 시간대(Asia/Seoul), IPv6 비활성화, cloud-init 비활성화가 1.0.0과 같다.
- SSH: root와 비밀번호 로그인 허용, `UseDNS no`, `GSSAPIAuthentication no`. 호스트 키는 VM마다 첫 부팅에서 만들어졌다.
- `_Lecture_k8s_starter.kit` ch2/2.4 Vagrantfile로 `vagrant up`: 4분 14초에 노드 4대가 클러스터에 붙었다. 확인 시점에 w3-k8s는 붙은 지
  13초라 NotReady였고 다시 확인하지 않았다. box 이름만 시험용으로 바꾸고 나머지는 그대로다.
- VirtualBox만으로 같은 구성을 만들었다(4분 2초, 노드 4대 Ready). ch3/3.4의 MetalLB를 적용했다. 192.168.1.11에 호스트에서 접속되고 노드 사이에 root 비밀번호(sshpass)로 접속된다.
- 베이그런트가 "No guest additions were detected" 안내를 보여 준다. 1.0.0 arm64 box에도 게스트 확장이 없어서 같은 안내가 나온다.

### starter.kit 전체 실습 (2026-11-19, arm64)

`vagrant up`(4분 16초, 노드 4대 Ready)으로 만든 클러스터에서 `_Lecture_k8s_starter.kit`의 3장부터 7장까지 `.cmd` 순서대로 실행했다.
대화형 명령은 같은 결과를 내는 방법으로 바꿨다(`kubectl edit`는 `KUBE_EDITOR`로, `kubectl exec -it`는 명령을 넘겨서).

| 실습 | 결과 |
|---|---|
| 3.1~3.5 파드, NodePort, 디플로이먼트, MetalLB, 정리 | 통과. 호스트에서 NodePort 6곳과 192.168.1.11 접속 |
| 5.1 오브젝트 삭제와 복구 | 통과 |
| 5.2 워커의 kubelet, containerd 중지 | 통과. NotReady 노드를 피해 배치되고 containerd를 멈춘 동안 ContainerCreating |
| 5.3 컨트롤 플레인의 kubelet, containerd 중지 | 통과(아래 참고) |
| 6.1 `kubectl edit`로 replicas 변경 | 통과 |
| 6.2 NFS 볼륨 | 통과. 파드를 지운 뒤 새 파드에서 이전 기록이 보이고 시간대는 +0900 |
| 7.1 kubeadm 업그레이드(1.35.0에서 1.35.1) | 통과 |
| 7.2 앤서블 업그레이드(1.35.2) | 통과. root 비밀번호로 노드 4대에 키를 복사하고 플레이북 실패 0 |
| 7.3 단축어(`k`) | 통과 |
| B.001, A.004 | 통과 |
| A.003 etcdctl | 실패. 받는 etcdctl이 x86-64용이라 arm64에서 실행되지 않는다. box와 관계없다 |
| A.005 Polaris | 실패. 최신 릴리스에 `dashboard.yaml`이 없어 404다. box와 관계없다 |

5.3에서 kubelet을 멈춘 뒤 `kubectl delete pod kube-scheduler-cp-k8s`는 kubelet이 삭제를 마무리하지 못해 끝나지 않는다. 쿠버네티스
동작이라 box와 관계없고 `--wait=false`로 이어서 실행했다. 실습을 마친 뒤 `vagrant halt`와 `vagrant up`(1분 22초)으로 다시 켜도
노드 4대가 Ready이고 정상이 아닌 파드가 0개이며 NFS 볼륨과 192.168.1.11 접속이 유지됐다.

### 시험하며 수정한 것

- chroot 안에서 패키지 설치가 띄운 프로세스가 남아 `/sys`를 해제하지 못했다. 해제 전에 그 프로세스를 끝낸다.
- `linux-generic`이 펌웨어 약 800 MB를 함께 설치했다. bento처럼 dpkg 제외 규칙으로 파일을 풀지 않는다.
- apt가 받은 `.deb`를 이미지 안에 받았다 지우면 그 블록이 디스크 파일에 남아 box가 1.7 GB였다. apt 캐시를 이미지 밖 폴더에
  연결해서 675 MB가 됐다. `fstrim`은 루프 장치에서 동작한다(할당 3,246 MiB에서 2,195 MiB). `zerofree`는 크기를 더 줄이지 않았다.

### 남은 것

- amd64 box를 x86 기기(Intel Mac, Windows)에서 확인
- 2판 책 실습과 다른 강의 저장소(learning.kit, prom, graf)에서 확인(starter.kit는 위에서 확인)
- 클라우드 이미지를 `current`가 아니라 날짜(serial)로 고정할지

## 직접 실행

같은 아키텍처의 Ubuntu에서 실행한다.

```bash
sudo apt-get install -y qemu-utils cloud-guest-utils gdisk pigz zerofree
sudo box/build.sh arm64 out
```

## 게시

Actions에서 `Box` 워크플로를 손으로 실행한다. `version`에 신규 버전(예: 1.1.0)을 넣고 `publish`를 선택하면 아래 순서로
진행한다. `publish`를 선택하지 않으면 box는 아티팩트로만 남고 바뀔 카탈로그를 실행 요약에 보여 준다.

1. R2 `files/sysnet4admin/Ubuntu-k8s/<버전>/`에 box 2개를 올리고 크기를 확인한다
2. `vg-box/k8s`에 Release `Ubuntu-k8s-<버전>`을 만들고 box 2개와 SHA256SUMS를 올린다
3. R2 카탈로그(`sysnet4admin/Ubuntu-k8s`)와 Pages 카탈로그(`vg-box/k8s`의 `sysnet4admin/Ubuntu-k8s.json`)에 신규 버전을 더한다
4. `box.kuberneteslab.dev`로 카탈로그와 box 파일을 받아 확인한다

기존 버전은 카탈로그에 남긴다. `box_version`으로 버전을 고정한 Vagrantfile이 계속 동작하게 하기 위해서다.
