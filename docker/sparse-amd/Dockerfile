# syntax=docker/dockerfile:1.7
#
# TEI for AMD ROCm — based on the upstream Dockerfile-amd from PR #860
# (huggingface/text-embeddings-inference, branch fa-varlen). That PR is the
# canonical reference for the AMD build path. We mirror its base image and
# install steps, with two deliberate departures:
#
# 1. Skip the flash-attention build. The upstream Dockerfile builds
#    ROCm/flash-attention from source pinned at gfx942 (MI300). The fork
#    does NOT support gfx1100 (RDNA3 / RX 7000). PR #853 wires a PyTorch
#    `F.scaled_dot_product_attention` fallback that engages when flash-attn
#    is absent, so leaving it out is functionally correct on RDNA3 — just
#    slower than MI300 would be.
# 2. Single-stage build instead of cargo-chef. We rebuild fewer times so
#    layer caching for Rust deps isn't worth the multi-stage complexity yet.

FROM rocm/pytorch:rocm7.1_ubuntu22.04_py3.10_pytorch_release_2.8.0 AS builder

# Specific PR-merge SHA — required-amd.txt etc. landed post-v1.9.3.
ARG TEI_REF=1588129f932125a780ab97ccb300e7774b02d230
ENV DEBIAN_FRONTEND=noninteractive \
    CARGO_HOME=/root/.cargo \
    PATH=/root/.cargo/bin:$PATH

RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential git cmake ninja-build python3-dev \
        curl ca-certificates pkg-config protobuf-compiler libssl-dev \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --default-toolchain stable --profile minimal

WORKDIR /usr/src
RUN git init tei \
    && cd tei \
    && git fetch --depth 1 https://github.com/huggingface/text-embeddings-inference.git ${TEI_REF} \
    && git checkout FETCH_HEAD

WORKDIR /usr/src/tei

# Python backend install — mirrors upstream Dockerfile-amd. Their Makefile
# handles install correctly on this specific base image without the pip
# workarounds (setuptools<70, --no-build-isolation, etc.) my earlier attempts
# needed against rocm7.2.3+pytorch2.9.1. The Makefile reads requirements.txt;
# copy requirements-amd.txt into that name first.
RUN cp backends/python/server/requirements-amd.txt \
       backends/python/server/requirements.txt \
    && cd backends/python/server && make install

# requirements-amd.txt pins numpy==1.26.4, accelerate==0.33.0 (which wants
# numpy<2), and downgrades scipy/sklearn — but the rocm/pytorch base ships
# numpy 2.x + scipy 1.15 already, and the downgrade leaves scipy/sklearn
# linked against the wrong numpy ABI (scipy.interpolate._fitpack_impl
# TypeError at import). Restore the base versions; also add three deps
# missing from rocm/pytorch's slim env that transformers transitively needs.
RUN pip install --no-build-isolation --force-reinstall \
        numpy scipy scikit-learn \
    && pip install --no-build-isolation \
        more_itertools psutil "backports.tarfile"

# Rust router — same features as upstream Dockerfile-amd's http variant.
RUN cargo build --release \
        --no-default-features \
        --features python,http \
        --bin text-embeddings-router \
    && mv target/release/text-embeddings-router /usr/local/bin/ \
    && rm -rf target

# ── Runtime ───────────────────────────────────────────────────────────────
# Same base — Python backend imports torch/rocm at startup, needs the full
# ROCm runtime. Could trim by copying just /opt/venv to a leaner base later.
FROM rocm/pytorch:rocm7.1_ubuntu22.04_py3.10_pytorch_release_2.8.0

ENV PATH=/usr/local/bin:$PATH \
    HUGGINGFACE_HUB_CACHE=/data

COPY --from=builder /usr/local/bin/text-embeddings-router /usr/local/bin/
COPY --from=builder /usr/src/tei /usr/src/tei
COPY --from=builder /opt/venv /opt/venv

WORKDIR /usr/src/tei

EXPOSE 80

ENTRYPOINT ["text-embeddings-router"]
CMD ["--model-id", "prithivida/Splade_PP_en_v1", \
     "--pooling", "splade", \
     "--dtype", "float16", \
     "--auto-truncate", \
     "--hostname", "0.0.0.0", \
     "--port", "80"]
