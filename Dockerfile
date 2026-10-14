# syntax=docker/dockerfile:1
#
# The contest runner and the SHRINCS signer, in one image.
#
# The runner fetches tx-manifest-lib at the rev pinned in runner/Cargo.toml. The signer is
# built from tx-manifest-wallet, which comes in as a named build context: check out the same
# commit there.
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

# Only the signer's sources: none of a local signer build.
COPY --from=txmanifest contrib/shrincs-signer/Makefile contrib/shrincs-signer/shrincs-signer.cpp \
     /txmanifest-wallet/contrib/shrincs-signer/

# The signer fetches shrincs-cpp at its pinned commit.
RUN make -C /txmanifest-wallet/contrib/shrincs-signer

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
