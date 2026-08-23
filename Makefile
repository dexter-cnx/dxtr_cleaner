.PHONY: format format-check test check ci run cli-dry-run

format:
	cargo fmt --all

format-check:
	cargo fmt --all -- --check

test:
	cargo test -p cleaner-core -p cleaner-macos -p cleaner-cli

check:
	cargo check --workspace

ci: format-check test

run:
	cargo run -p cleaner-gui

cli-dry-run:
	cargo run -p cleaner-cli -- scan --category dev
