# Local engine on Linux (https://chat.reframe.dev/ in Chrome)

`optchat-local` runs an OptChat engine on your own Linux machine and serves it at
**https://chat.reframe.dev/** in your normal Chrome (or opens the native app against it).

The engine is the reframe app `@dj/test`, branch `optchat`, run by a local reframe hypervisor with
`REFRAME_LOCAL=1`. Turns use your own `claude` and `codex` logins on this machine, never API keys.

```
Chrome ── https://chat.reframe.dev (/etc/hosts → 127.0.0.1:443) ─┐
native app ── http://127.0.0.1:8080 ─────────────────────────────┼─ caddy ── hypervisor 127.0.0.1:8000
                                                                 │   (adds x-reframe-router-route:
Chrome, no root setup ── https://chat.reframe.dev:8443 ──────────┘    dj--test--optchat--default)
```

## What you need

| Thing | Default path | Override |
|---|---|---|
| Patched reframe runtime (stock Deno can't run it; it needs `Reframe.blake3`) | `~/.reframe/runtime/reframe` | `REFRAME_RUNTIME` |
| reframe repo checkout or worktree (`hub/@reframe/cli/main.ts`, `deno.json`) | `~/.reframe/hypervisor-src` | `REFRAME_HYPERVISOR_SRC` |
| Caddy 2.11 | `~/.reframe/local/bin/caddy` | `OPTCHAT_CADDY` |
| `claude` logged in, `codex` logged in (ChatGPT's bundled codex at `/usr/lib/chatgpt/resources/codex`), `sqlite3` | on `PATH` | |
| Google Chrome | `google-chrome` | |
| Native app (only for `optchat-local app`) | `/usr/bin/optchat` | `OPTCHAT_APP` |

Install the script:

```bash
install -m 0755 shells/linux/local-engine/optchat-local ~/.local/bin/optchat-local
```

## One-time setup

```bash
optchat-local setup        # no sudo: a local CA + a chat.reframe.dev cert; Chrome trusts the CA (~/.pki/nssdb)
sudo optchat-local root    # /etc/hosts: 127.0.0.1 chat.reframe.dev; caddy may bind :443; CA in the system store
```

- `setup` makes `~/.reframe/local/tls/ca.{crt,key}` and `chat.reframe.dev.{crt,key}` (it keeps an existing key),
  then adds the CA to your user's NSS database. If `certutil` is missing it unpacks `libnss3-tools` for your user
  only. Restart Chrome afterwards if it was open.
- `.dev` is HSTS-preloaded, so Chrome needs a certificate it trusts: there is no "proceed anyway".
- `root` is the only step that needs sudo. It adds the hosts line (tagged `# optchat-local`), gives caddy
  `cap_net_bind_service` (`setcap`; repeat after replacing the caddy binary) and adds the CA to the system store,
  so the native app, `curl` and `deno` trust it too.
- Undo: delete the `# optchat-local` line in `/etc/hosts`, `sudo setcap -r ~/.reframe/local/bin/caddy`, and
  `certutil -D -d sql:$HOME/.pki/nssdb -n "OptChat Local CA (<user>)"`.
- While `chat.reframe.dev` points at 127.0.0.1, the real public host is unreachable from this machine.

## Every day

```bash
optchat-local            # start the engine and open https://chat.reframe.dev/ in Chrome; Ctrl-C stops it
optchat-local app        # start the engine and open the native app; closing it stops the engine
optchat-local serve      # engine only
optchat-local status     # ports, DNS, :443 permission, and whether the page answers
optchat-local stop       # stop a stray engine (anything on :8000/:8080/:8443/:443)
optchat-local logs       # follow ~/.reframe/local/logs/{hypervisor,caddy}.log
optchat-local config     # write and validate ~/.reframe/local/Caddyfile.optchat-local
```

The first page load starts the engine: it creates `~/.reframe/data/workspaces/` with `engine.db`, `admin-token`
and `local-client-token`. A fresh engine has no workspace: open https://chat.reframe.dev/engine, sign in with the
contents of `admin-token` and create one.

Until `sudo optchat-local root` has run, `optchat-local` opens `http://127.0.0.1:8080/` instead (loopback pages
may be plain http) and still serves https on `:8443` for a Chrome started with
`--host-resolver-rules='MAP chat.reframe.dev:443 127.0.0.1:8443'`.

## Troubleshooting

- **"another OptChat process ... is writing this chat"**: an old hypervisor still holds the writer lock:
  `optchat-local stop`.
- **Chats stuck at "0 steps"**: the snap `codex` is too old; the script puts `/usr/lib/chatgpt/resources` first
  on the engine's `PATH`.
- **`NET::ERR_CERT_AUTHORITY_INVALID`**: run `optchat-local setup` again and fully restart Chrome.
- **`ERR_CONNECTION_REFUSED` on https://chat.reframe.dev/**: caddy can't bind :443 (`optchat-local status`); run
  `sudo optchat-local root`, then restart the engine.
