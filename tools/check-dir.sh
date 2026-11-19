#!/usr/bin/env bash
# 지정한 폴더 아래의 Vagrantfile마다 k8slab 해석 결과가 tools/oracle.rb(실제 Ruby)와 같은지 확인한다.
# 파일을 복사하지 않으므로 공개할 수 없는 Vagrantfile을 확인할 때 쓴다.
#
#   tools/check-dir.sh <폴더>
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
dir=${1:?폴더를 지정한다}

cargo build --quiet --manifest-path "$here/Cargo.toml"
bin=$here/target/debug/k8slab
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

same=0; diff=0; fail=0
while IFS= read -r f; do
  for arch in arm64 x86_64; do
    if ! ruby "$here/tools/oracle.rb" "$f" "$arch" > "$tmp/o.json" 2> "$tmp/o.err"; then
      echo "정답 생성 실패 [$arch] $f: $(grep -m1 -oE '[^:]+\(RuntimeError\)|[A-Za-z]+Error.*' "$tmp/o.err" | head -1)"
      fail=$((fail + 1)); continue
    fi
    if ! "$bin" plan -f "$f" --arch "$arch" > "$tmp/k.json" 2> "$tmp/k.err"; then
      echo "k8slab 실패 [$arch] $f: $(head -1 "$tmp/k.err")"
      fail=$((fail + 1)); continue
    fi
    if ruby -rjson -e 'exit(JSON.parse(File.read(ARGV[0])) == JSON.parse(File.read(ARGV[1])) ? 0 : 1)' "$tmp/o.json" "$tmp/k.json"; then
      same=$((same + 1))
    else
      echo "다름 [$arch] $f"
      diff=$((diff + 1))
    fi
  done
done < <(find "$dir" -name Vagrantfile -not -path '*/.git/*' -not -path '*/.vagrant/*' | sort)

echo "같음 ${same}건, 다름 ${diff}건, 실패 ${fail}건"
