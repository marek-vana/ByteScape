#!/bin/zsh
set -euo pipefail
cd -- "${0:A:h}"
if [[ -x target/release/bytescape ]]; then
  exec target/release/bytescape "$@"
fi
task_cargo="${CARGO_HOME:-$HOME/.cargo}/bin/cargo"
if [[ ! -x "$task_cargo" ]]; then
  task_cargo="$(command -v cargo)"
fi
exec "$task_cargo" run --release -- "$@"
