ARG BASE_IMAGE=docker.io/library/fedora:44
FROM ${BASE_IMAGE}

USER root
RUN dnf -y install --setopt=install_weak_deps=False \
        rust cargo rustfmt clippy just gcc gcc-c++ make pkgconf-pkg-config \
        python3 nodejs jq curl openssl ca-certificates git tar gzip util-linux \
        findutils diffutils shellcheck chromium chromedriver rust-std-static-wasm32-unknown-unknown lld \
    && dnf clean all

ARG DEPENDENCY_INDEX
ARG CHEF_VERSION=0.1.78
ARG NEXTEST_VERSION=0.9.140
ARG WASM_BINDGEN_VERSION=0.2.125
ENV CARGO_HOME=/opt/firemage/cargo RUSTC_WRAPPER=""
ENV PATH=/usr/local/bin:${PATH}
COPY ci/build/cargo.sh /opt/firemage/cargo-proxy.sh
COPY ci/build/configure-cargo.py /opt/firemage/configure-cargo.py

# Persist source replacement so cargo-chef's Cargo subprocesses use the proxy too.
RUN python3 /opt/firemage/configure-cargo.py \
    && FIREMAGE_DEPENDENCY_INDEX="$DEPENDENCY_INDEX" bash /opt/firemage/cargo-proxy.sh \
        install --locked --version "$CHEF_VERSION" --root /usr/local cargo-chef \
    && FIREMAGE_DEPENDENCY_INDEX="$DEPENDENCY_INDEX" bash /opt/firemage/cargo-proxy.sh \
        install --locked --version "$NEXTEST_VERSION" --root /usr/local cargo-nextest \
    && FIREMAGE_DEPENDENCY_INDEX="$DEPENDENCY_INDEX" bash /opt/firemage/cargo-proxy.sh \
        install --locked --version "$WASM_BINDGEN_VERSION" --root /usr/local wasm-bindgen-cli \
    && cargo --version && rustc --version && cargo chef --version \
    && cargo fmt --version && cargo clippy --version && just --version \
    && python3 --version && node --version && openssl version && cc --version \
    && command -v flock \
    && chmod -R a+rwX "$CARGO_HOME"

# Check non-root access within the builder; nested OCI execution is not required.
COPY ci/build/toolchain-check.sh /opt/firemage/toolchain-check.sh
RUN setpriv --reuid=65534 --regid=65534 --clear-groups \
    bash /opt/firemage/toolchain-check.sh

WORKDIR /workspace
