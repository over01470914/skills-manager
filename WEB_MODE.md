# Skills Manager Web mode

Web mode serves the existing React interface from a headless Rust process. It uses
the same library database and core functions as the desktop app. The web process
must run on the machine whose skills and agent directories you want to manage.

## Build

```sh
npm ci
npm run build
cd src-tauri
cargo build --release --bin skills-manager-web --bin skills-manager-cli
```

Keep `dist/` in the repository and both binaries in the same `target/release`
directory. Alternatively set `SM_WEB_DIST` and `SM_CLI_PATH` to their absolute
paths. On startup, Web mode publishes that CLI to `~/.skills-manager/bin` so
deployed bundle entry skills can invoke the same commands. If publishing fails,
the server reports the error instead of serving a bundle that agents cannot run.

## Run locally

From the repository root:

```sh
./src-tauri/target/release/skills-manager-web
```

Open <http://127.0.0.1:1420/>. The process listens on loopback by default.
`SM_WEB_PORT` changes the port. `SM_SKILLS_ROOT` can select an existing
`skills` directory; otherwise the configured Skills Manager library is used.
Changes to the library path in Settings take effect after restarting the server.

## Run on a Mac mini over Tailscale

Set `SM_WEB_HOST` to the Mac mini's Tailscale IPv4 address and a random
`SM_WEB_TOKEN` of at least 24 characters:

```sh
SM_WEB_HOST=100.x.y.z SM_WEB_TOKEN='replace-with-a-long-random-secret' \
  ./src-tauri/target/release/skills-manager-web
```

Open `http://100.x.y.z:1420/` on another device in the same tailnet and enter
the token in the sign-in form. The server accepts API requests only for its bound
host and same origin. Keep the token private; the browser stores it for the current
tab session. Do not bind the server to a public network address. The Web process
does not run the desktop app's background update, tray, or automatic backup tasks;
use the Backup page for manual Git synchronization.

The browser page can manage the library, presets, agent deployment, bundles,
basic settings, and Git skill installation through the original preview flow.
Browser file pickers cannot select directories on the Mac mini; for a
server-local skill path use the CLI on that machine. Some desktop-specific
actions, including opening the server's file manager, remain desktop-only.
