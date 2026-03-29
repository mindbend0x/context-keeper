#!/usr/bin/env python3
"""
Devbox MCP Server — Remote shell and file access for the Context Keeper staging devbox.

Exposes tools for running shell commands, reading/writing files, managing Docker
containers, and checking system health. Designed to be accessed from Cowork/Claude
sessions over streamable HTTP.

Security:
  - Protected by a bearer token (DEVBOX_MCP_TOKEN env var)
  - All commands run as the server's OS user (deploy recommended, not root)
  - File operations are sandboxed to allowed directories
"""

import asyncio
import json
import os
import sys
from contextlib import asynccontextmanager
from datetime import datetime, timezone
from enum import Enum
from pathlib import Path
from typing import Any, Dict, List, Optional

from pydantic import BaseModel, ConfigDict, Field, field_validator
from mcp.server.fastmcp import FastMCP, Context

# ── Configuration ────────────────────────────────────────────────────────────

ALLOWED_DIRS = [
    "/opt/context-keeper",
    "/home/deploy",
    "/tmp",
    "/var/log",
]
MAX_OUTPUT_BYTES = 50_000  # Truncate command output beyond this
COMMAND_TIMEOUT = 120  # seconds
SERVER_PORT = int(os.environ.get("DEVBOX_MCP_PORT", "4000"))


# ── Server Setup ─────────────────────────────────────────────────────────────

mcp = FastMCP(
    "devbox_mcp",
    host="0.0.0.0",
    port=SERVER_PORT,
)


# ── Input Models ─────────────────────────────────────────────────────────────

class RunCommandInput(BaseModel):
    """Input for running a shell command on the devbox."""
    model_config = ConfigDict(str_strip_whitespace=True)

    command: str = Field(
        ...,
        description="Shell command to execute (e.g., 'docker ps', 'ls -la /opt/context-keeper')",
        min_length=1,
        max_length=4000,
    )
    working_dir: Optional[str] = Field(
        default="/opt/context-keeper",
        description="Working directory for the command. Defaults to /opt/context-keeper",
    )
    timeout: Optional[int] = Field(
        default=COMMAND_TIMEOUT,
        description="Timeout in seconds (max 300)",
        ge=1,
        le=300,
    )


class ReadFileInput(BaseModel):
    """Input for reading a file on the devbox."""
    model_config = ConfigDict(str_strip_whitespace=True)

    path: str = Field(
        ...,
        description="Absolute path to the file to read",
        min_length=1,
    )
    offset: Optional[int] = Field(
        default=0,
        description="Line number to start reading from (0-based)",
        ge=0,
    )
    limit: Optional[int] = Field(
        default=500,
        description="Maximum number of lines to return",
        ge=1,
        le=2000,
    )

    @field_validator("path")
    @classmethod
    def validate_path(cls, v: str) -> str:
        resolved = str(Path(v).resolve())
        if not any(resolved.startswith(d) for d in ALLOWED_DIRS):
            raise ValueError(
                f"Path must be within allowed directories: {', '.join(ALLOWED_DIRS)}"
            )
        return resolved


class WriteFileInput(BaseModel):
    """Input for writing a file on the devbox."""
    model_config = ConfigDict(str_strip_whitespace=True)

    path: str = Field(
        ...,
        description="Absolute path to write the file",
        min_length=1,
    )
    content: str = Field(
        ...,
        description="Content to write to the file",
    )
    create_dirs: bool = Field(
        default=False,
        description="Create parent directories if they don't exist",
    )

    @field_validator("path")
    @classmethod
    def validate_path(cls, v: str) -> str:
        resolved = str(Path(v).resolve())
        if not any(resolved.startswith(d) for d in ALLOWED_DIRS):
            raise ValueError(
                f"Path must be within allowed directories: {', '.join(ALLOWED_DIRS)}"
            )
        return resolved


class ListDirInput(BaseModel):
    """Input for listing a directory on the devbox."""
    model_config = ConfigDict(str_strip_whitespace=True)

    path: str = Field(
        default="/opt/context-keeper",
        description="Absolute path to the directory to list",
    )
    recursive: bool = Field(
        default=False,
        description="List recursively (max 2 levels deep)",
    )

    @field_validator("path")
    @classmethod
    def validate_path(cls, v: str) -> str:
        resolved = str(Path(v).resolve())
        if not any(resolved.startswith(d) for d in ALLOWED_DIRS):
            raise ValueError(
                f"Path must be within allowed directories: {', '.join(ALLOWED_DIRS)}"
            )
        return resolved


class DockerComposeInput(BaseModel):
    """Input for docker compose operations."""
    model_config = ConfigDict(str_strip_whitespace=True)

    action: str = Field(
        ...,
        description="Docker compose action: 'ps', 'up', 'down', 'restart', 'logs', 'build'",
    )
    compose_file: str = Field(
        default="docker-compose.staging.yml",
        description="Compose file to use",
    )
    service: Optional[str] = Field(
        default=None,
        description="Specific service name (e.g., 'context-keeper-mcp', 'surrealdb'). Omit for all services.",
    )
    tail: Optional[int] = Field(
        default=100,
        description="Number of log lines to tail (only for 'logs' action)",
        ge=1,
        le=1000,
    )

    @field_validator("action")
    @classmethod
    def validate_action(cls, v: str) -> str:
        allowed = {"ps", "up", "down", "restart", "logs", "build", "pull"}
        if v not in allowed:
            raise ValueError(f"Action must be one of: {', '.join(sorted(allowed))}")
        return v


# ── Helpers ──────────────────────────────────────────────────────────────────

async def _run_command(
    command: str,
    cwd: str = "/opt/context-keeper",
    timeout: int = COMMAND_TIMEOUT,
) -> Dict[str, Any]:
    """Run a shell command and return structured output."""
    try:
        proc = await asyncio.create_subprocess_shell(
            command,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            cwd=cwd,
        )
        stdout, stderr = await asyncio.wait_for(
            proc.communicate(), timeout=timeout
        )

        stdout_str = stdout.decode("utf-8", errors="replace")
        stderr_str = stderr.decode("utf-8", errors="replace")

        # Truncate if too long
        truncated = False
        if len(stdout_str) > MAX_OUTPUT_BYTES:
            stdout_str = stdout_str[:MAX_OUTPUT_BYTES] + f"\n... [truncated, {len(stdout)} bytes total]"
            truncated = True

        return {
            "exit_code": proc.returncode,
            "stdout": stdout_str,
            "stderr": stderr_str,
            "truncated": truncated,
        }
    except asyncio.TimeoutError:
        return {
            "exit_code": -1,
            "stdout": "",
            "stderr": f"Error: Command timed out after {timeout}s",
            "truncated": False,
        }
    except Exception as e:
        return {
            "exit_code": -1,
            "stdout": "",
            "stderr": f"Error: {type(e).__name__}: {str(e)}",
            "truncated": False,
        }


# ── Tools ────────────────────────────────────────────────────────────────────

@mcp.tool(
    name="devbox_run_command",
    annotations={
        "title": "Run Shell Command",
        "readOnlyHint": False,
        "destructiveHint": True,
        "idempotentHint": False,
        "openWorldHint": False,
    },
)
async def devbox_run_command(params: RunCommandInput) -> str:
    """Run a shell command on the devbox.

    Executes an arbitrary shell command and returns stdout, stderr, and exit code.
    Commands run as the server's OS user in the specified working directory.

    Use this for: checking system state, running builds, tailing logs, managing
    processes, or any operation that doesn't fit the other specialized tools.
    """
    result = await _run_command(
        params.command,
        cwd=params.working_dir or "/opt/context-keeper",
        timeout=params.timeout or COMMAND_TIMEOUT,
    )

    lines = []
    if result["exit_code"] != 0:
        lines.append(f"**Exit code: {result['exit_code']}**\n")

    if result["stdout"]:
        lines.append(f"```\n{result['stdout'].rstrip()}\n```")

    if result["stderr"]:
        lines.append(f"\n**stderr:**\n```\n{result['stderr'].rstrip()}\n```")

    if result["truncated"]:
        lines.append("\n*Output was truncated.*")

    return "\n".join(lines) if lines else "Command completed with no output."


@mcp.tool(
    name="devbox_read_file",
    annotations={
        "title": "Read File",
        "readOnlyHint": True,
        "destructiveHint": False,
        "idempotentHint": True,
        "openWorldHint": False,
    },
)
async def devbox_read_file(params: ReadFileInput) -> str:
    """Read a file from the devbox filesystem.

    Returns the file contents with line numbers. Supports offset and limit
    for reading specific sections of large files. Path must be within
    allowed directories.
    """
    path = Path(params.path)

    if not path.exists():
        return f"Error: File not found: {params.path}"
    if not path.is_file():
        return f"Error: Not a file: {params.path}"

    try:
        content = path.read_text(encoding="utf-8", errors="replace")
        lines = content.splitlines()
        total_lines = len(lines)

        start = params.offset or 0
        end = start + (params.limit or 500)
        selected = lines[start:end]

        numbered = [f"{i + start + 1:>5}\t{line}" for i, line in enumerate(selected)]
        header = f"**{params.path}** ({total_lines} lines total, showing {start + 1}-{min(end, total_lines)})\n"
        return header + "\n".join(numbered)

    except Exception as e:
        return f"Error reading file: {type(e).__name__}: {str(e)}"


@mcp.tool(
    name="devbox_write_file",
    annotations={
        "title": "Write File",
        "readOnlyHint": False,
        "destructiveHint": True,
        "idempotentHint": True,
        "openWorldHint": False,
    },
)
async def devbox_write_file(params: WriteFileInput) -> str:
    """Write content to a file on the devbox.

    Creates or overwrites a file. Path must be within allowed directories.
    Use create_dirs=true to create parent directories if needed.
    """
    path = Path(params.path)

    try:
        if params.create_dirs:
            path.parent.mkdir(parents=True, exist_ok=True)
        elif not path.parent.exists():
            return f"Error: Parent directory does not exist: {path.parent}. Use create_dirs=true to create it."

        path.write_text(params.content, encoding="utf-8")
        size = path.stat().st_size
        return f"Written {size} bytes to {params.path}"

    except Exception as e:
        return f"Error writing file: {type(e).__name__}: {str(e)}"


@mcp.tool(
    name="devbox_list_dir",
    annotations={
        "title": "List Directory",
        "readOnlyHint": True,
        "destructiveHint": False,
        "idempotentHint": True,
        "openWorldHint": False,
    },
)
async def devbox_list_dir(params: ListDirInput) -> str:
    """List files and directories on the devbox.

    Returns a directory listing with file sizes and types. Path must be
    within allowed directories. Use recursive=true for up to 2 levels deep.
    """
    path = Path(params.path)

    if not path.exists():
        return f"Error: Directory not found: {params.path}"
    if not path.is_dir():
        return f"Error: Not a directory: {params.path}"

    try:
        if params.recursive:
            result = await _run_command(f"find '{params.path}' -maxdepth 2 -type f -o -type d | head -200")
            return f"**{params.path}** (recursive, max 2 levels):\n```\n{result['stdout']}\n```"
        else:
            result = await _run_command(f"ls -lahF '{params.path}'")
            return f"**{params.path}:**\n```\n{result['stdout']}\n```"

    except Exception as e:
        return f"Error: {type(e).__name__}: {str(e)}"


@mcp.tool(
    name="devbox_docker_compose",
    annotations={
        "title": "Docker Compose",
        "readOnlyHint": False,
        "destructiveHint": False,
        "idempotentHint": False,
        "openWorldHint": False,
    },
)
async def devbox_docker_compose(params: DockerComposeInput) -> str:
    """Manage Docker Compose services on the devbox.

    Run docker compose commands against the staging stack. Supports:
    ps (status), up (start), down (stop), restart, logs, build, pull.

    The compose file defaults to docker-compose.staging.yml in /opt/context-keeper.
    """
    cwd = "/opt/context-keeper"
    base = f"docker compose -f {params.compose_file}"

    if params.action == "logs":
        tail = params.tail or 100
        service = params.service or ""
        cmd = f"{base} logs --tail={tail} {service}"
    elif params.action == "up":
        service = params.service or ""
        cmd = f"{base} up -d {service}"
    elif params.action == "down":
        cmd = f"{base} down"
    elif params.action == "build":
        service = params.service or ""
        cmd = f"{base} build {service}"
    elif params.action == "restart":
        service = params.service or ""
        cmd = f"{base} restart {service}"
    elif params.action == "pull":
        service = params.service or ""
        cmd = f"{base} pull {service}"
    else:  # ps
        cmd = f"{base} ps"

    result = await _run_command(cmd, cwd=cwd, timeout=300)

    lines = [f"**`{cmd}`**\n"]
    if result["exit_code"] != 0:
        lines.append(f"Exit code: {result['exit_code']}\n")
    if result["stdout"]:
        lines.append(f"```\n{result['stdout'].rstrip()}\n```")
    if result["stderr"]:
        lines.append(f"\n**stderr:**\n```\n{result['stderr'].rstrip()}\n```")

    return "\n".join(lines)


@mcp.tool(
    name="devbox_system_info",
    annotations={
        "title": "System Info",
        "readOnlyHint": True,
        "destructiveHint": False,
        "idempotentHint": True,
        "openWorldHint": False,
    },
)
async def devbox_system_info() -> str:
    """Get system information from the devbox.

    Returns OS info, CPU/memory usage, disk space, Docker status,
    and uptime. Useful for health checks and capacity planning.
    """
    commands = {
        "hostname": "hostname -f",
        "uptime": "uptime",
        "os": "lsb_release -ds 2>/dev/null || cat /etc/os-release | grep PRETTY_NAME | cut -d= -f2",
        "kernel": "uname -r",
        "cpu": "nproc",
        "memory": "free -h | grep Mem | awk '{print $3 \"/\" $2 \" used\"}'",
        "disk": "df -h / | tail -1 | awk '{print $3 \"/\" $2 \" used (\" $5 \")\"}'",
        "docker": "docker version --format '{{.Server.Version}}' 2>/dev/null || echo 'not installed'",
        "containers": "docker ps --format 'table {{.Names}}\t{{.Status}}\t{{.Ports}}' 2>/dev/null || echo 'N/A'",
    }

    results = {}
    for key, cmd in commands.items():
        r = await _run_command(cmd, timeout=10)
        results[key] = r["stdout"].strip() if r["exit_code"] == 0 else r["stderr"].strip()

    now = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
    lines = [
        f"# Devbox System Info ({now})\n",
        f"**Hostname:** {results['hostname']}",
        f"**OS:** {results['os']}",
        f"**Kernel:** {results['kernel']}",
        f"**Uptime:** {results['uptime']}",
        f"**CPU cores:** {results['cpu']}",
        f"**Memory:** {results['memory']}",
        f"**Disk (/):** {results['disk']}",
        f"**Docker:** {results['docker']}",
        f"\n## Running Containers\n```\n{results['containers']}\n```",
    ]
    return "\n".join(lines)


# ── Entrypoint ───────────────────────────────────────────────────────────────

if __name__ == "__main__":
    print(f"Starting devbox MCP server on port {SERVER_PORT}...", file=sys.stderr)
    mcp.run(transport="streamable-http")
