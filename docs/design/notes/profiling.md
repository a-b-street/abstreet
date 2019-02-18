# Profiling

apt-get install google-perftools libgoogle-perftools-dev

Follow Usage from https://crates.io/crates/cpuprofiler

Run editor with --enable_profiler
google-pprof --web ../target/debug/editor profile
google-pprof --web ../target/release/headless profile

Or run without --web and do 'top30 --cum'
