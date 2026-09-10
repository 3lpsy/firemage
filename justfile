set dotenv-load := true
set dotenv-path := ".env"
set dotenv-required := true
set positional-arguments := true
export CARGO_TARGET_DIR := env("FIREMAGE_CARGO_TARGET_DIR", justfile_directory() / "target")

default:
    @just --list

# Run a focused Cargo command with the configured dependency proxy.
cargo *args:
    bash ci/internal/build/cargo.sh "$@"

docker-cargo *args:
    bash ci/internal/build/docker.sh recipe dev cargo "$@"

check:
    bash ci/internal/build/cargo.sh check --workspace --all-targets

build:
    bash ci/internal/build/cargo.sh build --workspace

release: ui-build
    FIREMAGE_WEBUI_EMBED_DIR="$PWD/dist/webui" bash ci/internal/build/cargo.sh build --locked --release -p firemage

test: test-crates test-webui test-doc

fmt:
    cargo fmt --all

lint:
    bash ci/internal/build/cargo.sh clippy --workspace --all-targets -- -D warnings

fmt-check:
    cargo fmt --all -- --check

clippy: lint

test-crates:
    bash ci/internal/runner/test-packages.sh crates

test-doc:
    bash ci/internal/build/cargo.sh test --workspace --doc

build-release: release

check-all: fmt-check clippy check ui-check test-crates test-webui test-doc test-infra-command test-ci test-docker test-fj

verify: check-all

run *args:
    bash ci/internal/build/cargo.sh run -p firemage -- "$@"

# Run the selected prebuilt binary without invoking Cargo.
test-cli-binary:
    FIREMAGE_TEST_BINARY="${FIREMAGE_TEST_BINARY:-$PWD/dist/firemage}" python3 crates/tests-infra/scripts/test-cli.py

# Real CLI, HTTPS and terminal login. Needs Python 3 and OpenSSL, no KVM.
test-cli: build
    python3 crates/tests-infra/scripts/test-cli.py

# Build the shared Rust/chef toolchain from the configured base image.
docker-toolchain:
    bash ci/internal/build/docker.sh toolchain

# Build dependencies with cargo-chef, reuse target/podman, export dist/firemage.
docker-build:
    bash ci/internal/build/docker.sh recipe dev build

docker-release:
    bash ci/internal/build/docker.sh recipe release release

docker-check:
    bash ci/internal/build/docker.sh recipe dev check

docker-test:
    bash ci/internal/build/docker.sh recipe dev test

docker-test-cli:
    bash ci/internal/build/docker.sh recipe dev test-cli

docker-fmt:
    bash ci/internal/build/docker.sh recipe dev fmt

docker-lint:
    bash ci/internal/build/docker.sh recipe dev lint

docker-verify:
    bash ci/internal/build/docker.sh recipe dev verify

# Run Firemage in the development container, forwarding application arguments.
docker-run *args:
    bash ci/internal/build/docker.sh recipe dev run "$@"

# Validate container command contracts without a container daemon.
test-docker:
    python3 ci/internal/tests/test-docker.py
    python3 ci/internal/tests/test-cargo-config.py

# Forgejo guest entrypoints. CI creates an empty .env and supplies variables.
ci-sync-workdir:
    bash ci/internal/runner/ci-workdir.sh

ci-setup:
    bash ci/internal/runner/ci-setup.sh

ci-host-check:
    bash ci/internal/runner/ci-host-check.sh

ci-task task="verify":
    bash ci/internal/task/ci-task.sh "$1"

# Dispatch against an explicit revision using the authenticated Forgejo CLI.
ci-dispatch task="verify" ref="master" release="":
    bash ci/internal/task/ci-dispatch.sh "$1" "$2" "$3"

ci-warm ref="master":
    just fj-dispatch flanforge.yml "$1"

ci-runs workflow="ci-task.yml" job="":
    just fj-runs "$1" "$2"

# Validate CI argument handling without allocating a guest.
test-ci:
    python3 ci/internal/tests/test-ci.py
    python3 ci/internal/tests/test-ci-report.py
    python3 ci/internal/tests/test-release.py

# Build and publish the shared toolchain using the engine's registry credentials.
ci-toolchain:
    bash ci/internal/build/toolchain.sh publish

toolchain-dispatch ref="master":
    just fj-dispatch toolchain.yml "$1"

# Forgejo helpers follow LiftFG's file/ref/JSON and file/job/watermark conventions.
fj-dispatch workflow ref="master" inputs="{}":
    @bash ci/fj.sh dispatch "$1" "$2" "$3"

fj-runs workflow job="":
    @bash ci/fj.sh runs "$1" "$2"

fj-run-latest workflow:
    @bash ci/fj.sh run-latest "$1"

fj-run-wait workflow job since poll="30" tries="240":
    @bash ci/fj.sh run-wait "$1" "$2" "$3" "$4" "$5"

fj-ci-issues state="open":
    @bash ci/fj.sh issues "$1"

fj-issue-for-run run:
    @bash ci/fj.sh issue-for-run "$1"

fj-issue number:
    @bash ci/fj.sh issue "$1"

fj-issue-artifacts number dest="":
    @bash ci/fj.sh issue-artifacts "$1" "$2"

fj-issue-close number comment:
    @bash ci/fj.sh issue-close "$1" "$2"

_fj-api method path:
    @bash ci/fj.sh api "$1" "$2"

# Check Forgejo helper behavior without contacting the service.
test-fj:
    python3 ci/internal/tests/test-fj.py

# Forgejo container tiers follow LiftFG's tagged CI entrypoints.
ci-check:
    bash ci/internal/task/ci-tag.sh check

ci-clippy:
    bash ci/internal/task/ci-tag.sh clippy

ci-test-crates:
    bash ci/internal/task/ci-tag.sh test-crates

ci-tagged-release:
    bash ci/internal/task/ci-tag.sh release

ci-build task="build" ref="master":
    #!/usr/bin/env bash
    set -euo pipefail
    case "$1" in build|build-release|check-all) ;; *) echo 'Invalid build task' >&2; exit 2 ;; esac
    inputs="$(jq -nc --arg task "$1" '{task: $task}')"
    just fj-dispatch build.yml "$2" "$inputs"

ci-release tag="":
    #!/usr/bin/env bash
    set -euo pipefail
    tag="$1"
    if [[ -z "$tag" ]]; then
        version="$(sed -n '/^\[workspace.package\]/,/^\[/p' Cargo.toml | sed -n 's/^version = "\([^"]*\)"/\1/p' | head -n1)"
        tag="v$version"
    fi
    [[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]
    just fj-dispatch release.yml "$tag"

ci-test-tier tier="check-all":
    bash ci/docker/testing/run-tests.sh "$1"

ci-release-download tag="" infra="no":
    bash ci/internal/release/download-release.sh "$1" "$2"

# Container equivalents retain the native tier names used by LiftFG.
docker-build-release: docker-release

docker-check-all: docker-verify

docker-clippy: docker-lint

docker-test-crates:
    bash ci/internal/build/docker.sh recipe dev test-crates

docker-test-doc:
    bash ci/internal/build/docker.sh recipe dev test-doc

docker-fmt-check:
    bash ci/internal/build/docker.sh recipe dev fmt-check

docker-ci-check: docker-check

docker-ci-test-crates: docker-test-crates

docker-ci-clippy: docker-clippy

docker-ci-check-all: docker-check-all

docker-ci-build-release: docker-build-release

# Real infrastructure tests run on a prepared FlanForge host.
test-compose:
    bash ci/internal/tests/test-compose.sh

test-firecracker:
    sudo -n python3 crates/tests-infra/scripts/test-firecracker.py

test-vm:
    sudo -n env FIREMAGE_TEST_BINARY="${FIREMAGE_TEST_BINARY:-$PWD/dist/firemage}" FIREMAGE_CI_RESULTS_DIR="${FIREMAGE_CI_RESULTS_DIR:-$PWD/target/vm-results}" python3 crates/tests-infra/scripts/test-vm.py

# Render UI proposals for review before implementing components.
design-render:
    bash ci/internal/design/render.sh

# Development binaries embed the opt-in infrastructure harness.
build-dev:
    bash ci/internal/build/cargo.sh build -p firemage --features tests-infra

build-dev-release: ui-build
    FIREMAGE_WEBUI_EMBED_DIR="$PWD/dist/webui" bash ci/internal/build/cargo.sh build --locked --release -p firemage --features tests-infra

tests-infra *args:
    bash ci/internal/build/cargo.sh run -p firemage --features tests-infra -- tests-infra "$@"

ui-build:
    bash ci/internal/webui/build.sh

ui-check:
    bash ci/internal/build/cargo.sh check -p firemage-webui-app --target wasm32-unknown-unknown

test-webui:
    bash ci/internal/runner/test-packages.sh webui

test-infra-command:
    bash ci/internal/build/cargo.sh test -p firemage --features tests-infra --test infra_command

test-e2e: ui-build
    bash ci/internal/runner/run-webui-e2e.sh

ci-test-webui:
    bash ci/internal/task/ci-tag.sh test-webui

ci-test-e2e:
    bash ci/internal/task/ci-tag.sh test-e2e

ci-dev-release:
    just fj-dispatch dev-release.yml master

docker-ui-build:
    bash ci/internal/build/docker.sh recipe release ui-build

docker-ui-check:
    bash ci/internal/build/docker.sh recipe dev ui-check

docker-test-webui:
    bash ci/internal/build/docker.sh recipe dev test-webui

docker-test-e2e:
    bash ci/internal/build/docker.sh recipe dev test-e2e

docker-test-infra-command:
    bash ci/internal/build/docker.sh recipe dev test-infra-command

docker-build-dev:
    bash ci/internal/build/docker.sh recipe dev build-dev

docker-build-dev-release:
    bash ci/internal/build/docker.sh recipe release build-dev-release
