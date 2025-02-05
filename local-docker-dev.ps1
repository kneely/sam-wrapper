# Stop on errors
$ErrorActionPreference = "Stop"

# Build wasm fdw package using docker
docker build -t wasm-builder .
docker create --name temp-container wasm-builder
New-Item -ItemType Directory -Force -Path .\target\wasm32-unknown-unknown\release
docker cp "temp-container:/*.wasm" ".\target\wasm32-unknown-unknown\release\"
docker rm temp-container

# Copy wasm to db container
$db_container = docker ps --format "{{.Names}}" | Select-String "supabase_db_"
docker cp ".\target\wasm32-unknown-unknown\release\*.wasm" "${db_container}:/" 