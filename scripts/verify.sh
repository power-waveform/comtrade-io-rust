#!/usr/bin/env bash
# 提交前完整测试门禁。与 .github/workflows/ci.yml 各 job 复用同一组命令，
# 保证"本地过了 CI 也过"。用法：bash scripts/verify.sh
set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> 1/9 cargo fmt（两遍收敛）"
cargo fmt
cargo fmt
cargo fmt --check

echo "==> 2/9 cargo clippy（-D warnings）"
cargo clippy --all-targets -- -D warnings

echo "==> 3/9 cargo build --no-default-features"
cargo build --no-default-features

echo "==> 4/9 cargo test"
cargo test

echo "==> 5/9 cargo test --features gbk-builtin"
cargo test --features gbk-builtin

echo "==> 6/9 cargo doc --no-deps（-D warnings）"
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps

echo "==> 7/9 cargo build --examples"
cargo build --examples

echo "==> 8/9 cargo publish --dry-run（守体积 < 10MiB）"
cargo publish --dry-run --allow-dirty

echo "==> 9/9 验证打包内容（tests/ examples/ docs/ tasks/ .github/ scripts/ 不在列表）"
cargo package --list --allow-dirty > /tmp/pkglist.txt
if grep -qE "tests/|examples/|docs/|tasks/|CLAUDE\.md|rustfmt\.toml|\.github/|scripts/" /tmp/pkglist.txt; then
  echo "ERROR: 发布包混入应排除文件！"
  grep -E "tests/|examples/|docs/|tasks/|CLAUDE\.md|rustfmt\.toml|\.github/|scripts/" /tmp/pkglist.txt
  exit 1
fi

echo
echo "==> 全部通过 ✅"
