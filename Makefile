# Daily driver: `make` = fast dogfood build. With cargo's target dir shared
# machine-wide (`[build] target-dir` in ~/.cargo/config.toml) and its dogfood/
# on PATH, `atc` is live the moment it links; the build is the install.
# `make release` is the honest fat-LTO build.

.PHONY: build release test fmt fmt-check lint clean

build:
	cargo build --profile dogfood

release:
	cargo build --release

test:
	cargo test --workspace

fmt:
	cargo fmt --all
	pnpm --dir web format

fmt-check:
	cargo fmt --all --check
	pnpm --dir web format-check

lint:
	cargo clippy --workspace --all-targets -- -D warnings
	pnpm --dir web lint

# Only this workspace's own crates: the target dir is shared with every
# other workspace on the machine, so a bare `cargo clean` would take theirs.
clean:
	cargo clean -p atc-core -p atc-cli -p atc-serve -p atc-testsupport
