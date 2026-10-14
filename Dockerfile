# syntax=docker/dockerfile:1
#
# The contest runner and the SHRINCS signer, in one image.
#
# tx-manifest-wallet comes in as a named build context, so the runner builds against a
# local checkout of it rather than a pinned revision:
#
#   docker build --build-context txmanifest=../txmanifest-wallet -t pq-contest-runner .
#
# Run it with the contest repository mounted at /contest:
#
#   docker run --rm -v "$PWD:/contest" pq-contest-runner test --out results
#   docker run --rm -v "$PWD:/contest" pq-contest-runner site --results results --out public

FROM rust:1-bookworm AS build
RUN apt-get update \
 && apt-get install -y --no-install-recommends g++ make git libssl-dev pkg-config \
 && rm -rf /var/lib/apt/lists/*

# Only the sources the build needs: no target/, and none of a local signer build.
COPY --from=txmanifest Cargo.toml Cargo.lock /txmanifest-wallet/
COPY --from=txmanifest txmanifest_lib /txmanifest-wallet/txmanifest_lib
COPY --from=txmanifest txmanifest_wallet /txmanifest-wallet/txmanifest_wallet
COPY --from=txmanifest contrib/shrincs-signer/Makefile contrib/shrincs-signer/shrincs-signer.cpp \
     /txmanifest-wallet/contrib/shrincs-signer/

# The signer fetches shrincs-cpp at its pinned commit.
RUN make -C /txmanifest-wallet/contrib/shrincs-signer

# runner/Cargo.toml finds the wallet at ../../txmanifest-wallet, which is /txmanifest-wallet.
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
