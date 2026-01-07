.PHONY: test ci

test:
	docker compose run --rm tests

ci:
	docker compose run --rm ci
