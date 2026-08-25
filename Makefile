CORE_PACKAGES := -p cleaner-core -p cleaner-macos -p cleaner-cli

.PHONY: format format-check test clippy gui-check script-check check verify prepush ci run cli-dry-run package-macos notarize-macos generate-cask verify-macos-release

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
	bash -n scripts/macos/generate_cask.sh
	bash -n scripts/macos/test_generate_cask.sh
	bash -n scripts/macos/test_package_contract.sh
	bash -n scripts/macos/verify_release.sh
	bash -n scripts/macos/test_verify_release.sh
	bash scripts/macos/test_generate_cask.sh
	bash scripts/macos/test_package_contract.sh
	bash scripts/macos/test_verify_release.sh

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

generate-cask:
	bash scripts/macos/generate_cask.sh

verify-macos-release:
	bash scripts/macos/verify_release.sh
