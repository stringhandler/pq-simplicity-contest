# syntax=docker/dockerfile:1
#
# The contest runner and the SHRINCS signer, in one image.
#
# Both come from tx-manifest-wallet at the rev runner/Cargo.toml pins for tx-manifest-lib,
# fetched from GitHub, so the image matches what CI scores with:
#
#   docker build -t pq-contest-runner .
#
# Run it with the contest repository mounted at /contest:
#
#   docker run --rm -v "$PWD:/contest" pq-contest-runner test --out results
#   docker run --rm -v "$PWD:/contest" pq-contest-runner site --results results --out public

FROM rust:1-bookworm AS build
RUN apt-get update \
 && apt-get install -y --no-install-recommends g++ make git libssl-dev pkg-config \
 && rm -rf /var/lib/apt/lists/*

# The signer, from tx-manifest-wallet at the runner's pinned rev. It fetches shrincs-cpp
# at its own pinned commit.
COPY runner/Cargo.toml /contest/runner/Cargo.toml
RUN rev=$(sed -n 's/^tx-manifest-lib = .*rev = "\([0-9a-f]\{40\}\)".*/\1/p' /contest/runner/Cargo.toml) \
 && test -n "$rev" \
 && git init -q /txmanifest-wallet \
 && git -C /txmanifest-wallet fetch -q --depth 1 https://github.com/stringhandler/txmanifest-wallet "$rev" \
 && git -C /txmanifest-wallet checkout -q FETCH_HEAD \
 && make -C /txmanifest-wallet/contrib/shrincs-signer

COPY runner /contest/runner
RUN cargo build --release --locked --manifest-path /contest/runner/Cargo.toml

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates libssl3 git \
 && rm -rf /var/lib/apt/lists/* \
 && git config --system --add safe.directory '*'
# git dates each entry from the commit that added it. The mounted repository is owned by
# the host user, so git has to be told to trust it.
COPY --from=build /txmanifest-wallet/contrib/shrincs-signer/bin/shrincs-signer /usr/local/bin/
COPY --from=build /contest/runner/target/release/runner /usr/local/bin/
ENV SHRINCS_SIGNER=/usr/local/bin/shrincs-signer
WORKDIR /contest
ENTRYPOINT ["runner"]
