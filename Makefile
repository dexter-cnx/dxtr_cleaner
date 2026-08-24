CORE_PACKAGES := -p cleaner-core -p cleaner-macos -p cleaner-cli

.PHONY: format format-check test clippy gui-check script-check check verify prepush ci run cli-dry-run package-macos notarize-macos

format:
	cargo fmt --all

format-check:
	cargo fmt --all -- --check

test:
	cargo test $(CORE_PACKAGES)

clippy:
	cargo clippy $(CORE_PACKAGES) --all-targets -- -D warnings

gui-check:
	cargo check -p cleaner-gui

script-check:
	bash -n scripts/macos/package.sh
	bash -n scripts/macos/notarize.sh

verify: format-check test clippy gui-check script-check

check: verify

prepush: format verify

ci: verify

run:
	cargo run -p cleaner-gui

cli-dry-run:
	cargo run -p cleaner-cli -- scan --category dev

package-macos:
	bash scripts/macos/package.sh

notarize-macos:
	bash scripts/macos/notarize.sh
