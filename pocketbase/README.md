# PocketBase Backend for KanbanBOMs

PocketBase provides the shared state backend for the KanbanBOMs app. No custom backend code—collections and REST API are created via migrations.

## Quick Start

### 1. Download PocketBase

```bash
cd pocketbase
# Linux x64
curl -L -o pocketbase.zip https://github.com/pocketbase/pocketbase/releases/download/v0.22.0/pocketbase_0.22.0_linux_amd64.zip
unzip -o pocketbase.zip
chmod +x pocketbase
```

Or download from: https://pocketbase.io/docs/

### 2. Run

```bash
./pocketbase serve
```

- **Admin UI**: http://localhost:8090/_/
- **API base**: http://localhost:8090/api/

On first run, create a superuser (email + password) when prompted.

### 3. Collections

Migrations run automatically. The following collections are created:

| Collection             | Purpose                          |
|------------------------|----------------------------------|
| boms                   | Parts/assemblies                 |
| bom_entries            | BOM structure (assembly→component) |
| requests               | Kitting requests                 |
| request_entries        | Assemblies in a request          |
| bom_revisions          | BOM entry snapshots              |
| part_master_categories | Part Master category tabs (shared) |

All API rules are empty for internal-network open access.

### 4. Internal Network

To serve on all interfaces (for LAN access):

```bash
./pocketbase serve --http=0.0.0.0:8090
```

Clients set API URL to `http://<server-ip>:8090/api/`.
