#!/usr/bin/env bash
# Ubuntu 24.04 클라우드 이미지로 VirtualBox box를 만든다. VM을 띄우지 않는다(시제품, 2026-11).
#
#   sudo box/build.sh <amd64|arm64> <출력 폴더>
#
# 같은 아키텍처의 Linux에서 실행한다(GitHub Actions의 ubuntu-24.04, ubuntu-24.04-arm).
# 1. 공식 클라우드 이미지를 받아 SHA-256을 확인하고 64 GiB raw 디스크로 늘린다.
# 2. 루프 장치로 마운트하고 chroot에서 customize.sh를 실행한다. CPU가 같은 아키텍처라 에뮬레이션이 없다.
# 3. VMDK(streamOptimized)로 바꾸고 template/<아키텍처>/의 box.ovf로 묶는다. VirtualBox가 필요 없다.
#
# 필요한 것: qemu-utils, cloud-guest-utils(growpart), gdisk(sgdisk), pigz, e2fsprogs, zerofree
set -euo pipefail

ARCH=${1:?아키텍처(amd64 또는 arm64)}
OUT=$(realpath -m "${2:?출력 폴더}")
HERE=$(cd "$(dirname "$0")" && pwd)
SERIES=noble
BASE=https://cloud-images.ubuntu.com/$SERIES/current
IMG=$SERIES-server-cloudimg-$ARCH.img
DISK_BYTES=68719476736   # 지금 box와 같은 64 GiB(template의 ovf:capacity)
TPL=$HERE/template/$ARCH
WORK=$OUT/work
MNT=$WORK/mnt
LOOP=

[ "$(id -u)" = 0 ] || { echo "root로 실행한다(sudo)" >&2; exit 2; }
[ "$(dpkg --print-architecture)" = "$ARCH" ] || { echo "호스트 아키텍처가 $(dpkg --print-architecture)다. $ARCH 호스트에서 실행한다" >&2; exit 2; }
[ -f "$TPL/box.ovf" ] || { echo "$TPL/box.ovf가 없다" >&2; exit 2; }

mkdir -p "$WORK" "$MNT" "$OUT/box"
: > "$OUT/timings.tsv"
T0=$SECONDS; LAST=$SECONDS; CUR=
step() {  # 앞 단계의 걸린 시간을 기록하고 다음 단계를 시작한다
  if [ -n "$CUR" ]; then printf '%s\t%s\n' "$CUR" "$((SECONDS - LAST))" >> "$OUT/timings.tsv"; echo "::endgroup::"; fi
  CUR=$1; LAST=$SECONDS
  [ -n "$CUR" ] && echo "::group::$CUR"
  return 0
}

cleanup() {
  set +e
  [ -e "$MNT/usr/sbin/policy-rc.d" ] && rm -f "$MNT/usr/sbin/policy-rc.d"
  for m in var/cache/apt/archives run sys proc dev/pts dev boot/efi boot ""; do mountpoint -q "$MNT/$m" && umount -l "$MNT/$m"; done
  [ -n "$LOOP" ] && losetup -d "$LOOP"
}
trap cleanup EXIT

step "클라우드 이미지 받기"
curl -fsSL -o "$WORK/$IMG" "$BASE/$IMG"
curl -fsSL "$BASE/SHA256SUMS" | grep " \*$IMG\$" | sed "s| \*| $WORK/|" | sha256sum -c -
SERIAL=$(curl -fsSL "$BASE/unpacked/build-info.txt" | sed -n 's/^serial=//p')
echo "serial=$SERIAL"

step "raw 디스크로 바꾸고 64 GiB로 늘리기"
qemu-img convert -f qcow2 -O raw "$WORK/$IMG" "$WORK/disk.raw"
rm -f "$WORK/$IMG"
truncate -s "$DISK_BYTES" "$WORK/disk.raw"
sgdisk -e "$WORK/disk.raw"   # 늘린 끝으로 GPT 백업 헤더를 옮긴다
LOOP=$(losetup -fP --show "$WORK/disk.raw")
growpart "$LOOP" 1
partx -u "$LOOP"
e2fsck -pf "${LOOP}p1" || [ $? -le 1 ]
resize2fs "${LOOP}p1"
lsblk -o NAME,SIZE,FSTYPE,LABEL "$LOOP"

step "마운트"
part() { lsblk -lnpo NAME,LABEL "$LOOP" | awk -v l="$1" '$2 == l { print $1 }'; }
ROOT=$(part cloudimg-rootfs); BOOT=$(part BOOT); EFI=$(part UEFI)
[ -n "$ROOT" ] || { echo "루트 파티션(cloudimg-rootfs)을 찾지 못했다" >&2; exit 1; }
mount "$ROOT" "$MNT"
[ -n "$BOOT" ] && mount "$BOOT" "$MNT/boot"
[ -n "$EFI" ] && mount "$EFI" "$MNT/boot/efi"
mount --bind /dev "$MNT/dev"
mount --bind /dev/pts "$MNT/dev/pts"
mount -t proc proc "$MNT/proc"
mount -t sysfs sysfs "$MNT/sys"
mount -t tmpfs tmpfs "$MNT/run"
# apt가 받는 .deb(1 GB 가까이)를 이미지 밖에 둔다. 이미지 안에 받았다 지우면 그 블록이 디스크 파일에 남는다.
mkdir -p "$WORK/apt-archives/partial"
mount --bind "$WORK/apt-archives" "$MNT/var/cache/apt/archives"
# 이미지의 /etc/resolv.conf는 /run/systemd/resolve/stub-resolv.conf를 가리킨다. chroot는 호스트와 네트워크를 함께 쓰므로 호스트 설정을 넣는다.
mkdir -p "$MNT/run/systemd/resolve"
cat /etc/resolv.conf > "$MNT/run/systemd/resolve/stub-resolv.conf"
# chroot 안에서 패키지가 서비스를 시작하지 않게 한다
printf '#!/bin/sh\nexit 101\n' > "$MNT/usr/sbin/policy-rc.d"; chmod +x "$MNT/usr/sbin/policy-rc.d"

step "chroot에서 설정"
install -m 755 "$HERE/customize.sh" "$MNT/tmp/customize.sh"
chroot "$MNT" /usr/bin/env -i PATH=/usr/sbin:/usr/bin:/sbin:/bin HOME=/root LANG=C.UTF-8 /bin/bash /tmp/customize.sh
rm -f "$MNT/tmp/customize.sh" "$MNT/usr/sbin/policy-rc.d"
cp "$MNT/etc/box-build-info" "$OUT/box-build-info" 2>/dev/null || true

step "빈 블록 정리와 마운트 해제"
# chroot 안에서 패키지 설치가 띄운 프로세스(gpg-agent 등)가 남아 있으면 /sys를 해제하지 못한다(target is busy)
for p in /proc/[0-9]*; do
  if [ "$(readlink "$p/root" 2>/dev/null)" = "$MNT" ]; then
    echo "chroot 프로세스 종료: ${p#/proc/} $(tr '\0' ' ' < "$p/cmdline" 2>/dev/null)"
    kill "${p#/proc/}" 2>/dev/null || true
  fi
done
sleep 2
for m in var/cache/apt/archives run sys proc dev/pts dev; do umount -R "$MNT/$m" 2>/dev/null || umount -lR "$MNT/$m"; done
alloc() { echo "disk.raw 할당 $(du -B1M "$WORK/disk.raw" | cut -f1) MiB(겉보기 $(du -B1M --apparent-size "$WORK/disk.raw" | cut -f1) MiB) $1"; }
alloc "fstrim 전"
echo "루프 장치 discard_max_bytes=$(cat "/sys/block/${LOOP#/dev/}/queue/discard_max_bytes")"
fstrim -v "$MNT"; [ -n "$EFI" ] && fstrim -v "$MNT/boot/efi" || true; [ -n "$BOOT" ] && fstrim -v "$MNT/boot" || true
sync; alloc "fstrim 뒤"
for m in boot/efi boot ""; do mountpoint -q "$MNT/$m" && umount "$MNT/$m"; done
e2fsck -pf "$ROOT" || [ $? -le 1 ]
# fstrim이 루프 장치에서 구멍을 내지 못하는 경우에 대비해 빈 블록 중 데이터가 남은 것을 0으로 채운다(bento minimize.sh의 dd와 같은 목적)
zerofree -v "$ROOT"; [ -n "$BOOT" ] && zerofree -v "$BOOT" || true
sync; alloc "zerofree 뒤"
losetup -d "$LOOP"; LOOP=

step "VMDK로 바꾸기"
DISK=$(grep -o 'ovf:href="[^"]*"' "$TPL/box.ovf" | head -1 | cut -d'"' -f2)
qemu-img convert -f raw -O vmdk -o subformat=streamOptimized "$WORK/disk.raw" "$OUT/box/$DISK"
rm -f "$WORK/disk.raw"

step "box로 묶기"
cp "$TPL/box.ovf" "$TPL/Vagrantfile" "$TPL/metadata.json" "$OUT/box/"
BOX=ubuntu-24.04-$ARCH.virtualbox.box
tar -C "$OUT/box" -cf - box.ovf Vagrantfile metadata.json "$DISK" | pigz -9 > "$OUT/$BOX"
(cd "$OUT" && sha256sum "$BOX" > SHA256SUMS)
step ""

{
  echo "## $ARCH box (클라우드 이미지 $SERIAL)"
  echo
  echo "| 단계 | 초 |"
  echo "|---|---|"
  while IFS=$'\t' read -r name sec; do echo "| $name | $sec |"; done < "$OUT/timings.tsv"
  echo "| 합계 | $((SECONDS - T0)) |"
  echo
  echo "box: $(stat -c %s "$OUT/$BOX") bytes, VMDK: $(stat -c %s "$OUT/box/$DISK") bytes"
  echo
  echo '```'
  cat "$OUT/SHA256SUMS"
  [ -f "$OUT/box-build-info" ] && cat "$OUT/box-build-info"
  echo '```'
} > "$OUT/summary.md"
cat "$OUT/summary.md"
