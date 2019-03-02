# Profiling

apt-get install google-perftools libgoogle-perftools-dev

Follow Usage from https://crates.io/crates/cpuprofiler

Run editor or headless with --enable_profiler
google-pprof --no_strip_temp ../target/debug/editor profile
google-pprof --no_strip_temp ../target/release/headless profile
top30 --cum
