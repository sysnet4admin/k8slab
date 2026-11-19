#!/bin/sh
# 베이그런트 없이 VirtualBox만으로 쉽게 시작하는 쿠버네티스(_Lecture_k8s_starter.kit ch2) 실습 환경을 만든다. 시제품(2026-11-17).
# Vagrantfile과 같은 결과를 목표로 한다: VM 4대, 호스트 전용 네트워크 192.168.1.0/24, NAT SSH 포워딩 60010, 60101~,
# 실습 저장소의 프로비저닝 스크립트 3개를 같은 순서와 인자로 실행.
#
#   ./k8s-lab.sh up        만들고 프로비저닝
#   ./k8s-lab.sh status    VM 상태
#   ./k8s-lab.sh halt      VM 종료(다시 시작할 때는 up)
#   ./k8s-lab.sh destroy   VM 삭제(호스트 전용 네트워크와 box는 남긴다)
#
# macOS(Apple Silicon, Intel)와 Linux에서 같은 동작을 하도록 POSIX sh로만 작성했다. bash 3.2나 zsh 전용 문법을 사용하지 않는다.
# 반드시 ./k8s-lab.sh로 실행한다(bash k8s-lab.sh처럼 셸을 지정하면 첫 줄이 무시된다).

set -eu

## 설정 (Vagrantfile의 configuration variables와 같은 값) ##
N=3
K8S_V='1.35.0-1.1'
CTRD_V='2.2.2-1~ubuntu.24.04~noble'
BOX_NAME='sysnet4admin/Ubuntu-k8s'
BOX_VERSION="${K8S_LAB_BOX_VERSION:-1.0.0}"   # 다른 box를 시험할 때는 K8S_LAB_BOX_DIR(풀어 둔 box 폴더)과 함께 지정
BOX_CATALOG="https://box.kuberneteslab.dev/$BOX_NAME"
LAB_TAG='k8s-lab'
NET_PREFIX='192.168.1'
## /설정 ##

K8S_SHORT=$(printf '%s' "$K8S_V" | cut -c1-6)   # 1.35.0
K8S_MINOR=$(printf '%s' "$K8S_V" | cut -c1-4)   # 1.35
CTRD_SHORT=$(printf '%s' "$CTRD_V" | cut -c1-3) # 2.2
GROUP="/k8s-U$K8S_SHORT-ctrd-$CTRD_SHORT($LAB_TAG)"
WORK="${K8S_LAB_HOME:-$HOME/.k8s-lab}"

case "$(uname -m)" in
  arm64|aarch64) ARCH=arm64; KIT_DIR='ch2/2.4' ;;
  *)             ARCH=amd64; KIT_DIR='ch2/2.3' ;;
esac
KIT_RAW="https://raw.githubusercontent.com/sysnet4admin/_Lecture_k8s_starter.kit/main/$KIT_DIR"
BASE_VM="Ubuntu-k8s-$BOX_VERSION-$ARCH-base($LAB_TAG)"

log()  { printf '\n==> %s\n' "$*"; }
die()  { printf '\n[오류] %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "$1 명령이 없습니다. $2"; }

need VBoxManage "VirtualBox를 설치하세요."
need curl  ""
need tar   ""
need ssh   ""
need scp   ""

vm_exists()  { VBoxManage showvminfo "$1" >/dev/null 2>&1; }
vm_running() { VBoxManage list runningvms | grep -q "^\"$1\""; }

# 노드 목록을 "이름 IP SSH포트 CPU 메모리 역할" 한 줄씩 출력한다(배열 없이 POSIX sh로 반복하기 위해)
nodes() {
  printf 'cp-k8s %s.10 60010 2 2048 CP\n' "$NET_PREFIX"
  i=1
  while [ "$i" -le "$N" ]; do
    printf 'w%s-k8s %s.10%s 6010%s 1 1024 W\n' "$i" "$NET_PREFIX" "$i" "$i"
    i=$((i + 1))
  done
}
vm_name() { printf '%s-%s(%s)' "$1" "$K8S_SHORT" "$LAB_TAG"; }

# 호스트 전용 네트워크의 MAC. VirtualBox 접두사 080027 뒤에 IP 끝자리를 붙여 노드마다 고정한다
mac_of() {
  last=$(printf '%s' "$1" | awk -F. '{print $4}')
  printf '0800271A%04d' "$last"
}
mac_colon() { printf '%s' "$1" | sed 's/../&:/g; s/:$//' | tr 'A-F' 'a-f'; }

## box ##
box_dir() {
  if [ -n "${K8S_LAB_BOX_DIR:-}" ]; then printf '%s' "$K8S_LAB_BOX_DIR"; return; fi
  vg="$HOME/.vagrant.d/boxes/$(printf '%s' "$BOX_NAME" | sed 's#/#-VAGRANTSLASH-#')/$BOX_VERSION/$ARCH/virtualbox"
  if [ -f "$vg/box.ovf" ]; then printf '%s' "$vg"; return; fi   # 베이그런트로 이미 받은 box를 다시 사용
  printf '%s' "$WORK/boxes/$(basename "$BOX_NAME")/$BOX_VERSION/$ARCH"
}

fetch_box() {
  dir=$(box_dir)
  if [ -f "$dir/box.ovf" ]; then log "box 사용: $dir"; return; fi
  mkdir -p "$dir"
  url="https://box.kuberneteslab.dev/files/$BOX_NAME/$BOX_VERSION/$(basename "$BOX_NAME")-$BOX_VERSION-virtualbox-$ARCH.box"
  log "box 받기: $url"
  curl -fL --retry 3 -o "$dir/box.tar" "$url"
  # 카탈로그에서 이 아키텍처의 SHA-256을 찾는다(jq 없이)
  want=$(curl -fsSL "$BOX_CATALOG" | awk -v a="\"$ARCH\"" '
    /"architecture"/ { hit = index($0, a) > 0 }
    hit && /"checksum"/ && !/checksum_type/ { gsub(/[",]/, "", $2); print $2; exit }' FS=': *')
  if command -v sha256sum >/dev/null 2>&1; then got=$(sha256sum "$dir/box.tar" | awk '{print $1}')
  else got=$(shasum -a 256 "$dir/box.tar" | awk '{print $1}'); fi
  [ -n "$want" ] && [ "$want" = "$got" ] || die "SHA-256이 다릅니다(카탈로그 $want, 받은 파일 $got)"
  tar -xf "$dir/box.tar" -C "$dir" && rm -f "$dir/box.tar"
}

## SSH ##
ssh_keys() {
  kd="$WORK/ssh"; mkdir -p "$kd"
  for k in vagrant.key.ed25519 vagrant.key.rsa; do
    if [ ! -f "$kd/$k" ]; then
      if [ -f "$HOME/.vagrant.d/insecure_private_keys/$k" ]; then cp "$HOME/.vagrant.d/insecure_private_keys/$k" "$kd/$k"
      else curl -fsSL -o "$kd/$k" "https://raw.githubusercontent.com/hashicorp/vagrant/main/keys/$k"; fi
      chmod 600 "$kd/$k"
    fi
  done
}
SSH_OPTS="-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR -o ConnectTimeout=5 -o IdentitiesOnly=yes"
gssh() { port=$1; shift; ssh $SSH_OPTS -i "$WORK/ssh/vagrant.key.ed25519" -i "$WORK/ssh/vagrant.key.rsa" -p "$port" vagrant@127.0.0.1 "$@" </dev/null; }
gscp() { port=$1; shift; scp -q $SSH_OPTS -i "$WORK/ssh/vagrant.key.ed25519" -i "$WORK/ssh/vagrant.key.rsa" -P "$port" "$@"; }

wait_ssh() {
  printf '    SSH 대기(127.0.0.1:%s)' "$1"
  t=0
  until gssh "$1" true 2>/dev/null; do
    t=$((t + 5)); [ "$t" -le 300 ] || die "5분이 지나도 SSH가 열리지 않습니다(포트 $1)"
    printf '.'; sleep 5
  done
  printf ' 연결됨\n'
}

## 호스트 전용 네트워크 ##
# macOS의 VirtualBox 7부터는 hostonlynet, Linux는 hostonlyif를 사용한다
hostonly_nic_args() {
  if [ "$(uname -s)" = Darwin ]; then
    name=$(VBoxManage list hostonlynets | awk -v p="$NET_PREFIX.0" '
      /^Name:/ { n = $2 } /^NetworkMask:/ { m = $2 } /^LowerIP:/ && $2 == p && m == "255.255.255.0" { print n; exit }')
    if [ -z "$name" ]; then
      name="$LAB_TAG-net"
      log "호스트 전용 네트워크 만들기: $name ($NET_PREFIX.0/24)" >&2
      VBoxManage hostonlynet add --name "$name" --netmask 255.255.255.0 \
        --lower-ip "$NET_PREFIX.0" --upper-ip "$NET_PREFIX.0" --enable >/dev/null
    fi
    printf -- '--nic2 hostonlynet --host-only-net2 %s' "$name"
  else
    name=$(VBoxManage list hostonlyifs | awk -v ip="$NET_PREFIX.1" '/^Name:/ { n = $2 } /^IPAddress:/ && $2 == ip { print n; exit }')
    if [ -z "$name" ]; then
      name=$(VBoxManage hostonlyif create | sed -n "s/.*'\(.*\)'.*/\1/p")
      VBoxManage hostonlyif ipconfig "$name" --ip "$NET_PREFIX.1" --netmask 255.255.255.0
    fi
    printf -- '--nic2 hostonly --hostonlyadapter2 %s' "$name"
  fi
}

## VM ##
ensure_base() {
  vm_exists "$BASE_VM" && return
  log "기본 VM 가져오기: $BASE_VM"
  VBoxManage import "$(box_dir)/box.ovf" --vsys 0 --vmname "$BASE_VM" --group "/$LAB_TAG-base" >/dev/null
  VBoxManage snapshot "$BASE_VM" take base >/dev/null
}

create_vm() {  # 이름 IP 포트 CPU 메모리
  vm=$(vm_name "$1")
  if vm_exists "$vm"; then log "$vm 이미 있음"; return; fi
  log "$vm 만들기(연결 복제)"
  VBoxManage clonevm "$BASE_VM" --snapshot base --options link,keepallmacs --name "$vm" --register >/dev/null
  # shellcheck disable=SC2046
  # clonevm의 --groups는 반영되지 않아서(VirtualBox 7.2.8) 그룹은 여기서 지정한다
  VBoxManage modifyvm "$vm" --groups "$GROUP" --cpus "$4" --memory "$5" \
    --nic1 nat --natpf1 "ssh,tcp,,$3,,22" \
    $NIC2_ARGS --nictype2 82540EM --macaddress2 "$(mac_of "$2")" --cableconnected2 on
}

prepare_guest() {  # 호스트명 IP 포트
  mac=$(mac_colon "$(mac_of "$2")")
  gssh "$3" "sudo hostnamectl set-hostname $1 && printf '%s\n' \
    'network:' '  version: 2' '  ethernets:' '    lab:' '      match:' '        macaddress: \"$mac\"' \
    '      addresses: [$2/24]' | sudo tee /etc/netplan/60-$LAB_TAG.yaml >/dev/null && \
    sudo chmod 600 /etc/netplan/60-$LAB_TAG.yaml && sudo netplan apply"
}

provision() {  # 포트 스크립트 [인자...]
  port=$1; script=$2; shift 2
  [ -f "$WORK/kit/$script" ] || curl -fsSL -o "$WORK/kit/$script" "$KIT_RAW/$script"
  gscp "$port" "$WORK/kit/$script" "vagrant@127.0.0.1:/tmp/$script"
  log "  $script $*"
  # 베이그런트의 셸 프로비저너처럼 root로, 작업 위치는 /home/vagrant에서 실행한다
  gssh "$port" "cd /home/vagrant && sudo -H bash /tmp/$script $*"
}

check_ports() {
  nodes | while read -r h ip port c m r; do
    vm_exists "$(vm_name "$h")" && continue
    if command -v nc >/dev/null 2>&1 && nc -z 127.0.0.1 "$port" 2>/dev/null; then
      die "127.0.0.1:$port를 이미 다른 프로그램이 사용합니다. 베이그런트 실습 VM이 실행 중이면 먼저 종료하세요."
    fi
  done
}

cmd_up() {
  check_ports
  mkdir -p "$WORK/kit"
  fetch_box
  ssh_keys
  ensure_base
  NIC2_ARGS=$(hostonly_nic_args)
  nodes | while read -r h ip port c m r; do create_vm "$h" "$ip" "$port" "$c" "$m"; done
  # 베이그런트처럼 컨트롤 플레인부터 1대씩 올리고 프로비저닝한다
  nodes | while read -r h ip port c m r; do
    vm=$(vm_name "$h")
    if ! vm_running "$vm"; then
      log "$vm 시작"
      VBoxManage startvm "$vm" --type headless >/dev/null
      wait_ssh "$port"
    fi
    if [ -f "$WORK/done-$vm" ]; then log "$vm 이미 프로비저닝됨"; continue; fi
    prepare_guest "$h" "$ip" "$port"
    provision "$port" k8s_env_build.sh "$N" "$K8S_MINOR"
    provision "$port" k8s_pkg_cfg.sh "$K8S_V" "$CTRD_V" "$r"
    if [ "$r" = CP ]; then provision "$port" controlplane_node.sh "$K8S_SHORT"
    else provision "$port" worker_nodes.sh; fi
    touch "$WORK/done-$vm"
  done
  log "완료. 타비 설정은 강의 저장소의 config.yaml을 그대로 사용하면 됩니다(127.0.0.1:60010, 60101~)."
  gssh 60010 "sudo kubectl --kubeconfig /etc/kubernetes/admin.conf get nodes -o wide" || true
}

cmd_status() {
  nodes | while read -r h ip port c m r; do
    vm=$(vm_name "$h")
    if ! vm_exists "$vm"; then printf '%-28s 없음\n' "$vm"
    elif vm_running "$vm"; then printf '%-28s 실행 중  %s  ssh 127.0.0.1:%s\n' "$vm" "$ip" "$port"
    else printf '%-28s 종료됨\n' "$vm"; fi
  done
}

cmd_halt() {
  # 워커부터 종료하고 컨트롤 플레인을 마지막에 종료한다
  nodes | sort -r | while read -r h ip port c m r; do
    vm=$(vm_name "$h")
    vm_running "$vm" || continue
    log "$vm 종료"
    VBoxManage controlvm "$vm" acpipowerbutton
    t=0
    while vm_running "$vm"; do
      t=$((t + 2)); [ "$t" -le 120 ] || { VBoxManage controlvm "$vm" poweroff >/dev/null 2>&1; break; }
      sleep 2
    done
  done
}

cmd_destroy() {
  nodes | while read -r h ip port c m r; do
    vm=$(vm_name "$h")
    vm_exists "$vm" || continue
    log "$vm 삭제"
    vm_running "$vm" && VBoxManage controlvm "$vm" poweroff >/dev/null 2>&1 || true
    sleep 2
    VBoxManage unregistervm "$vm" --delete >/dev/null
    rm -f "$WORK/done-$vm"
  done
  log "VM을 지웠습니다. 기본 VM($BASE_VM)과 box는 다음 up에서 다시 사용하려고 남겨 둡니다."
}

case "${1:-}" in
  up)      cmd_up ;;
  status)  cmd_status ;;
  halt)    cmd_halt ;;
  destroy) cmd_destroy ;;
  *) printf '사용법: ./k8s-lab.sh up | status | halt | destroy\n'; exit 1 ;;
esac
