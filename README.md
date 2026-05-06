# ports-cli

Fast local port and process inspection for dev servers.

## Install

```bash
cargo install --git https://github.com/winterrx/ports-cli ports-cli
```

## Usage

```bash
ports
ports --all
ports ps
ports 3000
whoisonport 3000
ports kill 3000
ports kill all
ports kill-all --yes
ports watch
```

## Example

```text
$ ports

┌─────────────────────────────────────┐
│  Port Whisperer                     │
│  listening to your ports...         │
└─────────────────────────────────────┘

╭───────┬──────────┬───────┬──────────────┬───────────┬────────┬────────────╮
│ PORT  ┆ PROCESS  ┆ PID   ┆ PROJECT      ┆ FRAMEWORK ┆ UPTIME ┆ STATUS     │
╞═══════╪══════════╪═══════╪══════════════╪═══════════╪════════╪════════════╡
│ :3000 ┆ node     ┆ 41230 ┆ dashboard    ┆ Next.js   ┆ 12m    ┆ healthy    │
│ :5173 ┆ bun      ┆ 41882 ┆ marketing    ┆ Vite      ┆ 4m     ┆ healthy    │
╰───────┴──────────┴───────┴──────────────┴───────────┴────────┴────────────╯
```

## Notes

- `ports` shows the common dev-facing view.
- `ports --all` shows all listening ports.
- `ports ps` shows active dev processes.
- `ports kill all` kills every shown dev listener after confirmation.
- `ports kill-all --yes` skips the prompt; add `--all` to include every listener.
- `whoisonport <port>` is a shortcut for detailed lookup.
