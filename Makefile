.PHONY: dev build release test pytest check clippy fmt clean

dev:
	maturin develop

build:
	maturin build

release:
	maturin build --release

test:
	cargo test --lib
	cargo test --lib -p pyinferencemanager-core

pytest:
	pytest tests/python/ -v

test-integration:
	OLLAMA_AVAILABLE=1 cargo test --features integration_tests

check:
	cargo check --all

clippy:
	cargo clippy --all -- -D warnings

fmt:
	cargo fmt --all

clean:
	cargo clean
	rm -rf target dist *.egg-info
