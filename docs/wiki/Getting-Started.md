# Getting Started

## Docker Compose

```yaml
services:
  mirrorproxy:
    image: kudang/mirrorproxy:latest
    restart: unless-stopped
    ports: ["3000:3000"]
    volumes: ["mirrorproxy-data:/data"]
volumes:
  mirrorproxy-data:
```

Run `docker compose up -d`, check `http://127.0.0.1:3000/healthz`, then sign in
at `/admin`. If no initial password was supplied, read the generated password
from the container log. Keep the `/data` volume during upgrades; it contains
SQLite, cache data, and writable GeoIP databases.

For source builds, run `bash scripts/fetch-geoip.sh` followed by
`cargo run -p mirrorproxy-server -- serve`.

[中文](Getting-Started-zh-CN) · [Full deployment guide](Deployment)
