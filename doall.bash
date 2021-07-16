#!/usr/bin/env bash

set -euo pipefail

for x in /usr/local/mail/brennan/Maildir/cur/* ; do
cargo run --release --bin bmail -- $x
done
