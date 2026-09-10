ARG TOOLCHAIN
FROM ${TOOLCHAIN} AS planner
WORKDIR /workspace
COPY . .
RUN cargo chef prepare --recipe-path /recipe.json

FROM ${TOOLCHAIN} AS dependencies
WORKDIR /workspace
ARG PROFILE=dev
ENV CARGO_TARGET_DIR=/build-target
COPY --from=planner /recipe.json /opt/firemage/recipe.json

# Keep the cooked artifacts outside the runtime cache mount so it can be seeded.
RUN case "$PROFILE" in \
      dev) cargo chef cook --locked --workspace --all-targets --recipe-path /opt/firemage/recipe.json ;; \
      release) cargo chef cook --locked --release -p firemage --recipe-path /opt/firemage/recipe.json ;; \
      *) exit 1 ;; \
    esac \
    && mv /build-target /opt/firemage/target \
    && chmod -R a+rX /opt/firemage/target \
    && { sha256sum /opt/firemage/recipe.json; rustc -vV; printf '%s\n' "$PROFILE"; } | sha256sum > /opt/firemage/cache-key \
    && printf '%s\n' "$PROFILE" > /opt/firemage/profile \
    && chmod -R a+rwX "$CARGO_HOME"

COPY ci/build/container-entry.sh /opt/firemage/container-entry.sh
ENTRYPOINT ["bash", "/opt/firemage/container-entry.sh"]
CMD ["build"]
