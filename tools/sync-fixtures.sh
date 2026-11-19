#!/usr/bin/env bash
# 강의, 책, 연구(공개) 저장소에서 Ubuntu box를 사용하는 Vagrantfile을 받아 tests/fixtures/를 다시 만든다.
# 정답(plan.<arch>.json)은 tools/oracle.rb(실제 Ruby)로 만든다.
#
#   tools/sync-fixtures.sh
#
# Vagrantfile이 바뀌면 이 스크립트를 실행하고 cargo test로 k8slab 해석기가 따라가는지 확인한다.
set -euo pipefail
cd "$(dirname "$0")/.."

REPOS=(
  sysnet4admin/_Lecture_k8s_starter.kit
  sysnet4admin/_Lecture_k8s_learning.kit
  SeongJuMoon/_Lecture_prom_learning.kit
  SeongJuMoon/_Lecture_graf_learning.kit
  internal-k8s/_Book_k8sInfra
  sysnet4admin/Research
)

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
rm -rf tests/fixtures
mkdir -p tests/fixtures

count=0
for repo in "${REPOS[@]}"; do
  name=${repo#*/}
  git clone -q --depth 1 "https://github.com/$repo" "$tmp/$name"
  echo "$repo $(git -C "$tmp/$name" rev-parse --short HEAD)" >> "$tmp/sources"
  # .archived와 책의 옛 백업 폴더(8x., 9x.)는 뺀다
  while IFS= read -r f; do
    rel=${f#"$tmp/"}
    dir=tests/fixtures/$(dirname "$rel")
    mkdir -p "$dir"
    cp "$f" "$dir/Vagrantfile"
    for arch in arm64 x86_64; do
      ruby tools/oracle.rb "$f" "$arch" > "$dir/plan.$arch.json"
    done
    count=$((count + 1))
  done < <(grep -rl --include=Vagrantfile 'sysnet4admin/Ubuntu-k8s' "$tmp/$name" \
             | grep -v -E '/(\.archived|\.git)/|/[89][0-9]\.[^/]*/' | sort)
done

mv "$tmp/sources" tests/fixtures/SOURCES
echo "Vagrantfile ${count}개를 받았다(출처와 커밋은 tests/fixtures/SOURCES)"
