# k8slab

강의와 책의 Vagrantfile을 그대로 읽어 VirtualBox에 쿠버네티스 실습 환경을 만드는 도구입니다. 베이그런트(Vagrant) 없이
실행 파일 하나로 동작하고 Windows와 macOS를 지원합니다.

```bash
k8slab up        # vagrant up과 같은 일
k8slab halt
k8slab destroy
```

## 상태

개발 중입니다. 지금은 Vagrantfile 해석기만 있습니다.

```bash
k8slab plan -f Vagrantfile --arch arm64   # 해석한 실행 계획을 JSON으로 보여 줍니다
```

강의 저장소 4개, 『요즘 개발자를 위한 쿠버네티스/도커』(2판) 저장소, 연구 저장소(`sysnet4admin/Research`)에서 Ubuntu box를
사용하는 Vagrantfile 45개를 모두 베이그런트와 같게 읽는지 시험합니다. 시험의 정답은 Vagrantfile을 실제 Ruby로 실행해서 만듭니다(`tools/oracle.rb`).

## box

실습 VM이 사용하는 `sysnet4admin/Ubuntu-k8s` box도 이 저장소에서 만듭니다. GitHub Actions에서 amd64와 arm64를 VM 없이
3분 안에 만듭니다(`box/README.md`).

## 지원 범위

- box: `sysnet4admin/Ubuntu-k8s`
- 하이퍼바이저: VirtualBox
- 호스트: Windows, macOS(Apple Silicon, Intel)
