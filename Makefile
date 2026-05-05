.PHONY: hooks fmt clippy test bench

hooks:
	git config core.hooksPath .githooks
	chmod +x .githooks/pre-commit
	@echo "git hooks installed (.githooks/)"

fmt:
	cargo fmt --all

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

test:
	cargo test --workspace --all-features

bench:
	cargo bench --workspace
