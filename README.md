# Eidetic 🧠

**Eidetic** is an Agent-Agnostic Memory MCP Server. It provides a structured, long-term memory system via the Model Context Protocol (MCP).

Eidetic enables **memory portability**: a memory created by one agent (e.g. Claude Code) can be instantly recalled by another (e.g. Cursor). 

Agents can automatically store, recall, deduplicate, and review project knowledge—such as architectural decisions, user preferences, and bug resolutions—so they never have to start from scratch.

---

## Installation

You can install Eidetic in two ways:

### 1. Via NPM (Recommended)
You can install the pre-compiled binary wrapper globally using npm:

```bash
npm install -g eidetic-mcp
```

Or run it directly using `npx`:
```bash
npx eidetic-mcp
```

### 2. Via Cargo (From Source)
If you prefer to build from source, you can use Cargo:

```bash
git clone https://github.com/mcxross/eidetic.git
cd eidetic
cargo install --path .
```

---

## Agent Integration

Eidetic features a built-in auto-setup command to easily inject itself into the configuration files of popular coding agents.

Run the following command to register Eidetic with your agent of choice:

```bash
eidetic setup <agent>
```

### Supported Agents
*   `claude` - Claude Code CLI
*   `claude-desktop` - Claude Desktop App
*   `cursor` (or `pi`) - Cursor IDE
*   `vscode` - Visual Studio Code
*   `opencode` - OpenCode
*   `gemini-cli` - Gemini CLI
*   `codex` - OpenAI Codex CLI

**Example**:
```bash
eidetic setup cursor
```
This will automatically parse your `~/.cursor/mcp.json` file, safely merge Eidetic as an available MCP server, and point the agent to the exact Eidetic executable path.

---

## Features

### MCP Server
Start the MCP server manually (usually handled automatically by your agent):
```bash
eidetic serve
```
By default, the server uses a SQLite database to store memories locally.

### TUI Storage Inspector
Eidetic comes with a built-in Terminal UI to easily inspect, search, review, and delete the memories stored by your agents.

```bash
eidetic tui
```
**Controls:**
*   `Tab` - Switch views (Projects, Observations, Sessions, Search)
*   `Enter` - View details for an observation
*   `d` - Soft-delete an observation
*   `D` - Hard-delete an observation
*   `r` - Mark an observation as reviewed
*   `/` - Open search

---

## License
Apache 2.0
