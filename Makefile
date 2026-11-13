# The gate's steps, run by fleet's pre-commit skill. CI runs the same commands.
# The Python steps need a .venv with the package installed: python3 -m venv .venv && .venv/bin/pip install -e .
.PHONY: format lint typecheck test

format:
	cargo fmt
	ruff format .

lint:
	cargo clippy --all-targets --all-features -- -D warnings
	ruff check .

typecheck:
	cargo check --all-targets --all-features
	ty check

test:
	cargo test --all-features
	python3 -m unittest
