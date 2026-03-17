# Remove build artifacts, test cache and state files
clean:
    #cargo clean
    rm -rf tests/cache
    rm -f tests/state.json tests/state.*.json

# Run tests with stdout visible, then diff last two states
test:
    cargo test -- --nocapture

# Upload latest state file to minio
sync:
    #!/usr/bin/env bash
    set -euo pipefail
    latest=$(ls -1 tests/state.*.json 2>/dev/null | sort | tail -n1)
    if [ -z "$latest" ]; then
        echo "No state files found"
        exit 1
    fi
    echo "Uploading $latest"
    #minio-mc cp "$latest" bbq/brutal-csv-test/

empty-state:
    minio-mc ls --json bbq/brutal-csv-test/examples | jq -sc '{"Error": [.[].key]}'

