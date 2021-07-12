#!/usr/bin/env bash

set -euo pipefail

for x in /usr/local/mail/brennan/Maildir/cur/* ; do
cargo run --bin bmail -- $x
done
