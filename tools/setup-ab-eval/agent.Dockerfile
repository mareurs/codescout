FROM swebench/sweb.eval.x86_64.burntsushi_1776_ripgrep-2576@sha256:49be14a10abc3651effb761cb15f8439c54525a89c457e2d8db58b7d794aa76b
USER root
# Remove original Git objects, shell history and precompiled test artifacts.
RUN rm -rf /testbed/.git /testbed/target /root/.bashrc /root/.bash_history \
 && useradd --non-unique --uid 1000 --gid 1000 --create-home --shell /bin/bash agent \
 && cd /testbed && git init -q \
 && git config user.email diagnostic@example.invalid \
 && git config user.name Diagnostic \
 && git add . && git commit -qm 'Benchmark base source' \
 && chown -R 1000:1000 /testbed /usr/local/cargo
ENV HOME=/home/agent CARGO_NET_OFFLINE=true RUSTFLAGS=-Awarnings
USER 1000:1000
WORKDIR /testbed
# Warm only the original visible tests, without hidden-test or reference mounts.
RUN cargo test --offline --package ripgrep --test integration -- regression --color never
