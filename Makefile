# Daily driver: `make` = fast dogfood build; ~/.cargo/bin/ff-tower symlinks
# to target/dogfood/ff-tower, so `ff tower` is live the moment it links.
# `make release` is the honest fat-LTO build.

.PHONY: build release test fmt fmt-check lint install clean

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

# Point ~/.cargo/bin/ff-tower at the dogfood binary. That is the whole
# install: fufu's `ff-<name>` dispatch searches PATH, so `ff tower` finds it
# with nothing else wired. Idempotent; rerun after a move.
install: build
	@mkdir -p $(HOME)/.cargo/bin
	ln -sfn $(CURDIR)/target/dogfood/ff-tower $(HOME)/.cargo/bin/ff-tower
	@echo "linked $(HOME)/.cargo/bin/ff-tower -> $(CURDIR)/target/dogfood/ff-tower"

clean:
	cargo clean
