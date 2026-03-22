cargo clippy --workspace --all-targets --all-features -- -D warnings > clippy_out.txt 2>&1
cargo test --workspace > test_out.txt 2>&1
