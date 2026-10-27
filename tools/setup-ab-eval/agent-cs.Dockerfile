# sab agent image: the one image all three arms (bare, cs, full) of a task run in.
#
# FROM the task image (build arg BASE). It adds, under /opt/sab/bin (first on PATH):
#   codescout      built here from the manifest's codescout SHA with the daily features
#   rust-analyzer  the rustup component of the builder toolchain (pinned below)
#   node           for the codescout-companion hooks
# and python3 only when the task image lacks it. Nothing else of the task image changes: the
# user, workdir and /testbed are the base's. The `bare` arm runs in this same image, so the
# bytes an agent can read are the same in every arm; only CLI flags and mounts differ (arms.py).
#
# The index is NOT built here: a build cannot join the study's internal network. It is built at
# prepare time by arms.prepare_index, which runs `codescout index -p /testbed` in a container of
# this image on the study network, records the wall time in index-cost.json, and commits the
# result as the run image.
#
# Provenance (ruling R3): adapted from claude-plugins
# effort-steering/spikes/s5/images/codescout.Dockerfile at 223b56abecae (last changed 40b0586),
# source sha256 f8046b6a5f06ea51c4e51e4318fa49f5125b52734883f1b4718142e812cb65cd
# (`git show 223b56abecae:effort-steering/spikes/s5/images/codescout.Dockerfile | sha256sum`;
# recorded under "adapted" in PROVENANCE.json):
#   kept     node copied from the official node image (pinned binary, no extra apt repo);
#            mold, because codescout's .cargo/config.toml links with -fuse-ld=mold;
#            RUSTUP_TOOLCHAIN set so rustup ignores rust-toolchain.toml's windows-gnu target;
#            RUSTC_WRAPPER="" so config.toml's sccache wrapper is not needed.
#   changed  rust:1.97.1 (Debian 13, glibc 2.41) -> rust:1.97.1-bookworm (Debian 12, glibc 2.36),
#            node:26-trixie-slim -> node:26-bookworm-slim: the host binary (glibc 2.44) and a
#            trixie build do not run on a glibc 2.36 task image; a build on the task image's own
#            Debian release does. The toolchain lives only in the builder stage; the task
#            image keeps its own toolchain.
#   dropped  the agent user, CARGO_HOME=/cargo and the read-only cargo mount (the task image has
#            its own user and offline cargo setup).
#
# Build (the codescout source is a clean checkout of the manifest SHA, .git included so the
# binary bakes that SHA and git_dirty=false):
#   docker build -f agent-cs.Dockerfile --build-arg BASE=<task image> \
#     --build-context codescout_src=<checkout> -t <tag> <empty dir>

ARG BASE=reliable-agent-eval/transfer-final-ruff-15309:20261027
ARG RUST_BUILDER=rust:1.97.1-bookworm@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97
ARG NODE_IMAGE=node:26-bookworm-slim@sha256:28147155d47dc31abe0a46e0d12b59b2fb3d0a2dcc5dc65fce76a074180b16fe

FROM ${NODE_IMAGE} AS node

FROM ${RUST_BUILDER} AS codescout
ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_TOOLCHAIN=1.97.1-x86_64-unknown-linux-gnu \
    RUSTC_WRAPPER=""
RUN apt-get update \
 && apt-get install -y --no-install-recommends mold \
 && rm -rf /var/lib/apt/lists/*
RUN rustup component add rust-analyzer
# glibc shim (found by the first build here): ort's prebuilt onnxruntime static library, pulled
# in by the default `local-embed` feature, references three glibc 2.38 symbols,
# __isoc23_strtol/strtoll/strtoull (the link errors named exactly these); bookworm has 2.36, so
# the link fails. The shim defines four functions: those three plus __isoc23_strtoul, the
# remaining member of the family, unreferenced today. Each forwards to the C99
# functions (C23 only adds the 0b prefix), keeping the daily feature set (R11) on a glibc that
# the task image can run (R3). The study configures only HTTP embedders, so ort never runs.
COPY <<'EOF' /opt/isoc23-shim.c
#include <stdlib.h>
long __isoc23_strtol(const char *s, char **e, int b) { return strtol(s, e, b); }
long long __isoc23_strtoll(const char *s, char **e, int b) { return strtoll(s, e, b); }
unsigned long __isoc23_strtoul(const char *s, char **e, int b) { return strtoul(s, e, b); }
unsigned long long __isoc23_strtoull(const char *s, char **e, int b) { return strtoull(s, e, b); }
EOF
RUN cc -O2 -fPIC -c /opt/isoc23-shim.c -o /opt/isoc23-shim.o
# RUSTFLAGS replaces config.toml's target rustflags, so its mold link-arg is restated here.
ENV RUSTFLAGS="-C link-arg=-fuse-ld=mold -C link-arg=/opt/isoc23-shim.o"
COPY --from=codescout_src . /src
WORKDIR /src
# The `cargo rb` alias: `build --release --features server-stack`, default features on (R11).
RUN cargo build --release --locked --features server-stack \
 && install -d /out/bin /out/lib \
 && install -m 0755 target/release/codescout /out/bin/codescout \
 && install -m 0755 "$(rustup which rust-analyzer)" /out/bin/rust-analyzer \
 && cp -a "$(rustc --print sysroot)"/lib/*.so* /out/lib/ \
 && /out/bin/codescout version > /out/codescout-version.json \
 && /out/bin/rust-analyzer --version > /out/rust-analyzer-version.txt \
 && sha256sum /out/bin/codescout /out/bin/rust-analyzer > /out/sha256.txt

FROM ${BASE}
ARG AGENT_USER=1000:1000
USER root
COPY --from=codescout /out/bin/ /opt/sab/bin/
# rust-analyzer (the rustup component) links the toolchain's librustc_driver and libLLVM through
# RUNPATH $ORIGIN/../lib, so those libraries ship next to it in /opt/sab/lib (found by the
# second build here: the binary alone fails to load).
COPY --from=codescout /out/lib/ /opt/sab/lib/
COPY --from=codescout /out/codescout-version.json /out/rust-analyzer-version.txt /out/sha256.txt /opt/sab/
COPY --from=node /usr/local/bin/node /opt/sab/bin/node
RUN command -v python3 >/dev/null \
 || (apt-get update && apt-get install -y --no-install-recommends python3 \
     && rm -rf /var/lib/apt/lists/*)
ENV PATH=/opt/sab/bin:${PATH}
RUN codescout --version && rust-analyzer --version && node --version && python3 --version
USER ${AGENT_USER}
