#!/usr/bin/env bash
# build.sh가 클라우드 이미지 안(chroot)에서 실행한다.
# Packer와 bento로 만든 sysnet4admin/Ubuntu-k8s 1.0.0과 같은 설정을 넣는다. 항목마다 원래 어디서 하던 일인지 적었다.
set -euxo pipefail
export DEBIAN_FRONTEND=noninteractive
ARCH=$(dpkg --print-architecture)

## 패키지 ##
# bento update_ubuntu.sh: 자동 업데이트를 끄고 전체 업그레이드
systemctl disable apt-daily.timer apt-daily-upgrade.timer
systemctl mask apt-daily.service apt-daily-upgrade.service
apt-get -y purge unattended-upgrades ubuntu-release-upgrader-core
apt-get -y update
apt-get -y dist-upgrade -o Dpkg::Options::=--force-confnew
# bento cleanup_ubuntu.sh: linux-firmware는 설치하되 파일은 풀지 않는다(VM에는 필요 없고 약 800 MB다)
printf '%s\n' '#BENTO-BEGIN' 'path-exclude=/lib/firmware/*' 'path-exclude=/usr/lib/firmware/*' \
  'path-exclude=/usr/share/doc/linux-firmware/*' '#BENTO-END' >> /etc/dpkg/dpkg.cfg.d/excludes
# 클라우드 이미지의 커널 패키지는 linux-virtual이다. 지금 box(설치 프로그램)와 같은 linux-generic으로 맞춘다.
apt-get -y install linux-generic
apt-get -y purge linux-virtual linux-image-virtual linux-headers-virtual || true
rm -rf /lib/firmware/* /usr/lib/firmware/*
# chroot에서는 apt가 실행 중인 커널을 알 수 없어 이미지에 있던 커널을 지우지 않는다. 가장 새 커널만 남긴다.
NEW=$(ls /boot | sed -n 's/^vmlinuz-\(.*\)-generic$/\1/p' | sort -V | tail -1)
OLD=$(dpkg -l | awk '/^ii/ { print $2 }' | grep -E '^linux-(image|modules|modules-extra|headers)-[0-9]' | grep -v -- "$NEW" || true)
[ -z "$OLD" ] || apt-get -y purge $OLD
# custom_post_hoon.sh: 실습 도구
apt-get -y install zip jq bpytop stress psmisc net-tools bash-completion sshpass nfs-common curl git vim
curl -fsSL -o /usr/bin/yq "https://github.com/mikefarah/yq/releases/latest/download/yq_linux_${ARCH}"
chmod +x /usr/bin/yq

## 계정 ##
# autoinstall identity와 custom_post_hoon.sh: vagrant/vagrant, root/vagrant
useradd -m -s /bin/bash vagrant
for g in adm cdrom sudo dip plugdev lxd; do getent group "$g" >/dev/null && usermod -aG "$g" vagrant; done
echo 'vagrant:vagrant' | chpasswd
echo 'root:vagrant' | chpasswd
# bento _common/vagrant.sh: 베이그런트 공개 키(RSA, ED25519)
install -d -m 700 -o vagrant -g vagrant /home/vagrant/.ssh
curl -fsSL https://raw.githubusercontent.com/hashicorp/vagrant/main/keys/vagrant.pub > /home/vagrant/.ssh/authorized_keys
chown vagrant:vagrant /home/vagrant/.ssh/authorized_keys
chmod 600 /home/vagrant/.ssh/authorized_keys
# bento sudoers_ubuntu.sh
sed -i -e '/Defaults\s\+env_reset/a Defaults\texempt_group=sudo' /etc/sudoers
echo 'vagrant ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/99_vagrant
chmod 440 /etc/sudoers.d/99_vagrant

## SSH ##
# 클라우드 이미지는 비밀번호 로그인을 막는다(60-cloudimg-settings.conf). 지금 box처럼 허용한다(실습 스크립트가 sshpass를 사용).
rm -f /etc/ssh/sshd_config.d/60-cloudimg-settings.conf
echo 'PasswordAuthentication yes' > /etc/ssh/sshd_config.d/50-box.conf
# bento sshd.sh, custom_post_hoon.sh
sed -i 's/^#\?PermitRootLogin .*/PermitRootLogin yes/' /etc/ssh/sshd_config
printf 'UseDNS no\nGSSAPIAuthentication no\n' >> /etc/ssh/sshd_config
ln -sf /lib/systemd/system/ssh.service /etc/systemd/system/sshd.service
# 호스트 키는 VM마다 첫 부팅에서 만든다. 클라우드 이미지에서는 cloud-init이 하던 일인데 아래에서 cloud-init을 끈다.
cat > /etc/systemd/system/ssh-host-keys.service <<'EOF'
[Unit]
Description=Generate SSH host keys if missing
Before=ssh.service
ConditionPathExistsGlob=!/etc/ssh/ssh_host_*_key

[Service]
Type=oneshot
ExecStart=/usr/bin/ssh-keygen -A

[Install]
WantedBy=ssh.service multi-user.target
EOF
systemctl enable ssh-host-keys.service

## 부팅과 네트워크 ##
# bento networking_ubuntu.sh: eth0 이름과 DHCP
sed -i 's/^GRUB_CMDLINE_LINUX="\(.*\)"/GRUB_CMDLINE_LINUX="net.ifnames=0 biosdevname=0 \1"/' /etc/default/grub
cat > /etc/netplan/01-netcfg.yaml <<'EOF'
network:
  version: 2
  ethernets:
    eth0:
      dhcp4: true
EOF
chmod 600 /etc/netplan/01-netcfg.yaml
# 클라우드 이미지는 initrd 없이 먼저 부팅해 보고 실패하면 다시 부팅한다. VirtualBox 디스크 드라이버는 모듈이라 처음부터 initrd를 쓴다.
rm -f /etc/default/grub.d/40-force-partuuid.cfg
update-grub
# cloud-init은 데이터 소스가 없는 VirtualBox에서 할 일이 없다. 지금 box도 설치 뒤 꺼져 있다.
touch /etc/cloud/cloud-init.disabled

## 시스템 설정 ##
# custom_post_hoon.sh
ln -sf /usr/share/zoneinfo/Asia/Seoul /etc/localtime
echo Asia/Seoul > /etc/timezone
cat > /etc/apt/apt.conf.d/20auto-upgrades <<'EOF'
APT::Periodic::AutocleanInterval "0";
APT::Periodic::Unattended-Upgrade "1";
APT::Periodic::Download-Upgradeable-Packages "0";
APT::Periodic::Update-Package-Lists "0";
EOF
cat > /etc/sysctl.d/99-local-network.conf <<'EOF'
net.ipv6.conf.all.disable_ipv6 = 1
net.ipv6.conf.default.disable_ipv6 = 1
EOF
echo vagrant > /etc/hostname
printf '127.0.0.1 localhost\n127.0.1.1 vagrant\n' > /etc/hosts

## 정리 ##
apt-get -y autoremove --purge
apt-get clean
rm -rf /var/lib/apt/lists/*
# bento cleanup_ubuntu.sh
rm -rf /usr/share/doc/*
find /var/cache -type f -delete
rm -f /var/lib/systemd/random-seed
# 크기 확인용: 큰 폴더 목록
du -x -d 3 -m / 2>/dev/null | sort -n | tail -25
rm -f /etc/ssh/ssh_host_*
: > /etc/machine-id   # VM마다 첫 부팅에서 새로 만든다(/var/lib/dbus/machine-id는 이 파일을 가리키는 링크)
find /var/log -type f -exec truncate -s 0 {} +
rm -rf /tmp/* /var/tmp/* /root/.bash_history
{
  echo "kernel=$(ls /boot | sed -n 's/^vmlinuz-//p' | sort -V | tail -1)"
  echo "packages=$(dpkg -l | grep -c '^ii')"
} > /etc/box-build-info
