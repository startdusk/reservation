.PHONY: codeline
codeline:
	@tokei .
	@

.PHONY: initdb
initdb:
	@docker run \
    -e POSTGRES_USER=postgres \
    -e POSTGRES_PASSWORD=password \
    -e POSTGRES_DB=reservation \
    -p 5432:5432 \
    -d \
    --name=reservation \
    postgres -N 1000

.PHONY: test 
test: 
	@cargo nextest run

.PHONY: fmt
fmt:
	@cargo clippy
