#!/bin/bash

set -euxo pipefail

# build wasm fdw package using docker
docker build -t wasm-builder .
docker create --name temp-container wasm-builder
docker cp temp-container:/*.wasm ./target/wasm32-unknown-unknown/release/
docker rm temp-container

# copy wasm to db container
db_container=`docker ps --format "{{.Names}}" | grep supabase_db_`
docker cp target/wasm32-unknown-unknown/release/*.wasm ${db_container}:/