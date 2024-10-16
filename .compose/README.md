[//]: # (DO NOT EDIT!)
[//]: # (This file was provisioned by OpenTofu)
[//]: # (File origin: https://github.com/aetheric-oss/tofu-github/tree/main/src/modules/vars/templates/rust/svc/.compose/README.md)

# Aetheric Realm Docker Compose

This directory contains all files needed to run the full Aetheric Realm stack.
The `services.yaml` files can be used as includes in your own project's `compose.yaml` file.

## Usage

### Profiles

All services have been given profiles for easy selection of services to start.
The following profiles are available:

#### `realm-db`

Only start the database backend (including init containers)

#### `realm-gis`

Only start the gis backend (including init containers)

#### `realm`

Starts all `svc-*` services including their backend dependencies (db, gis, cache, queue)

#### `realm-tools`

Starts additional tools which are not required for the `realm` stack to function, but are nice to have for development purposes.
Currently, the tools profile will start:
  - A reverse proxy (Traefik) so that all services are exposed on their URI path on (localhost)[http://localhost]
  - A log watcher (dozzle) wich can be accessed on (localhost/logs)[http://localhost/logs] if the reverse proxy is running as well, or (localhost:8081)[http://localhost:8081]

### Example commands

#### Start all realm services following log output
```
docker compose --profile realm up
```

#### Start all realm services in the background
```
docker compose --profile realm up -d
```

#### Start all realm tools in the background
```
docker compose --profile realm-tools up -d
```

## Example `compose.yaml` files

The files found in this directory are just there to be used as includes in your own `compose.yaml` file.
Below you can find some examples about the usage of these files.

### Aetheric Realm services

`compose.yaml`
```yaml
---
include:
  - .compose/aetheric-gis/services.yaml
  - .compose/aetheric-db/services.yaml
  - .compose/aetheric-cache/services.yaml
  - .compose/aetheric-queue/services.yaml
  - .compose/aetheric-svc/services.yaml
```

example commands for this configuration:

Start stack:
```bash
docker compose --profile realm up -d
```

Stop stack:
```bash
docker compose --profile realm down
```

### Full Realm services stack including tools

`compose.yaml`
```yaml
---
include:
  - .compose/aetheric-gis/services.yaml
  - .compose/aetheric-db/services.yaml
  - .compose/aetheric-cache/services.yaml
  - .compose/aetheric-queue/services.yaml
  - .compose/aetheric-svc/services.yaml
  - .compose/dozzle/services.yaml
  - .compose/reverse-proxy/services.yaml
```

example commands for this configuration:

Start stack:
```bash
docker compose --profile realm up -d
docker compose --profile realm-tools up -d
```

Stop stack:
```bash
docker compose --profile realm-tools down
docker compose --profile realm down
```

### Aetheric Realm Single service only

This example shows how to create a compose file for the svc-storage service only.

`compose.yaml`
```yaml
---
include:
  - .compose/aetheric-db/services.yaml

configs:
  log4rs:
    file: log4rs.yaml

services:
  svc-storage:
    extends:
      file: .compose/aetheric-svc/svc.yaml
      service: storage
```

example commands for this configuration:

Start stack:
`docker compose svc-storage up -d`

Stop stack:
`docker compose --profile realm down`

Providing the `realm` profile is needed in order to stop all dependencies (eg. db) as well.
If you don't provide the profile, only the svc-storage service will be terminated
