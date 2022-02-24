#!/bin/bash
#:
#: name = "falcon-build"
#: variety = "basic"
#: target = "helios"
#: rust_toolchain = "nightly-2021-11-24"
#: output_rules = [
#:   "/work/debug/*",
#:   "/work/release/*",
#: ]
#: access_repos = [
#:   "oxidecomputer/p9fs",
#:   "oxidecomputer/ispf",
#:   "oxidecomputer/dendrite",
#: ]
#:

set -o errexit
set -o pipefail
set -o xtrace

cargo --version
rustc --version

banner build
ptime -m cargo build --features falcon
ptime -m cargo build --features falcon --release

for x in debug release
do
    mkdir -p /work/$x
    cp target/$x/propolis-server /work/$x/propolis-server
done
