# Profiling

apt-get install google-perftools libgoogle-perftools-dev

Follow Usage from https://crates.io/crates/cpuprofiler

google-pprof --web ../target/debug/editor profile
google-pprof --web ../target/release/headless profile
