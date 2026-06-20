---
name: install-eidetic-mcp
description: Install and configure the Eidetic MCP server. Use when user needs to set up Eidetic from npm or source, register it with an MCP client/agent, choose a storage backend, configure Memwal/Sui account settings, or troubleshoot Eidetic MCP startup/configuration.
---

# Install Eidetic

## Workflow

1. Choose the install path:
   - For normal users, prefer `npm install -g eidetic-mcp` or `npx eidetic-mcp`.
   - For source installs, use `cargo install --path .` from the Eidetic repo.
   - For local validation during development, use `cargo run -- ...`.
2. Register Eidetic with the target agent using `eidetic setup <agent>` after install.
3. Choose and configure storage. Prefer `memwal` when the user wants remote semantic memory; otherwise use the default `sqlite`.
4. Verify with a non-destructive command such as `eidetic --help`, `eidetic setup <agent>` dry inspection when possible, or `cargo check` in source workflows.

## Agent Setup

Use the built-in setup command when possible:

```bash
eidetic setup <agent>
```

Supported agent names are documented in `README.md`. Current expected names include `claude`, `claude-desktop`, `cursor`, `pi`, `vscode`, `opencode`, `gemini-cli`, and `codex`.

Warn before changing a real MCP client config if the user did not clearly ask for it. The setup command writes or updates client config files.

## Storage Backends

Eidetic supports `--storage-backend` and `EIDETIC_STORAGE_BACKEND`.

### Memwal

Use Memwal for remote semantic memory with a local SQLite exact CRUD index:

```bash
eidetic --storage-backend memwal serve
```

Common Memwal environment configuration:

```bash
export EIDETIC_STORAGE_BACKEND=memwal
export EIDETIC_MEMWAL_REGISTRY_ID=0x...
export EIDETIC_MEMWAL_NAMESPACE=eidetic
eidetic serve
```

Memwal reads Sui account data from `~/.sui/sui_config` by default. Use `--sui-config-dir` or `EIDETIC_SUI_CONFIG_DIR` to override it.

Available Memwal config inputs:

- `--memwal-account-id` / `EIDETIC_MEMWAL_ACCOUNT_ID`
- `--memwal-registry-id` / `EIDETIC_MEMWAL_REGISTRY_ID`
- `--memwal-server-url` / `EIDETIC_MEMWAL_SERVER_URL`
- `--memwal-relayer-config-url` / `EIDETIC_MEMWAL_RELAYER_CONFIG_URL`
- `--memwal-namespace` / `EIDETIC_MEMWAL_NAMESPACE`
- `--memwal-delegate-label` / `EIDETIC_MEMWAL_DELEGATE_LABEL`
- `--sui-config-dir` / `EIDETIC_SUI_CONFIG_DIR`

For MCP account operations, use:

- `mem_sui_accounts` to list usable Sui accounts without exposing private keys.
- `mem_select_sui_account` to choose the active account for the current server process.
- `mem_memwal_config` to show redacted backend/account configuration.

Account selection is process-local. Eidetic does not mutate `~/.sui/client.yaml` or persist selected Memwal account state across restarts.

### SQLite

SQLite is the default local backend:

```bash
eidetic --storage-backend sqlite serve
```

Use `--storage-path` or `EIDETIC_STORAGE_PATH` for a custom directory.

### File

Use file storage for JSON files that are easy to inspect:

```bash
eidetic --storage-backend file serve
```

Prefer SQLite for normal local use.
