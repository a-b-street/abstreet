#!/bin/bash
# Generates a scenario from external JSON data, for
# https://github.com/a-b-street/abstreet/issues/861.
#
# Along with actdev_scenario.sh, this should eventually be expressed as part of
# the Rust pipeline directly somehow, so that --scenario uses the original data
# (or a cached S3 version, rather)

set -e

wget https://github.com/spstreets/OD2017/releases/download/1/all_trips.json
cargo run --release --bin cli -- import-scenario --map=data/system/br/sao_paulo/maps/sao_miguel_paulista.bin --input=all_trips.json
# Cancel 80% of all driving trips, so the scenario doesn't gridlock
cargo run --release --bin cli -- augment-scenario --input-scenario=data/system/br/sao_paulo/scenarios/sao_miguel_paulista/Full.bin --scenario-modifiers='[{"ChangeMode":{"pct_ppl":80,"departure_filter":[0,864000000],"from_modes":["Drive"],"to_mode":null}}]' --delete-cancelled-trips
