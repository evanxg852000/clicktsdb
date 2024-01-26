

docker-build:
	@docker build . 

docker-up:
	docker compose -f docker-compose.yml up -d --remove-orphans --wait --build

docker-down:
	docker compose -f docker-compose.yml down
