$ErrorActionPreference = "Continue"
cargo build -p lucidity-host 2>&1 | Tee-Object -FilePath "build_output.txt"
