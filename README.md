# Porter

Porter is a tiny Tauri tray app that sits between you and your browsers. Make it
your system default browser, click a link anywhere, and pick the browser and
profile that link should open in — manually, or automatically via routes.

No tab soup, no browser juggling. It lives in the tray and stays out of the way.

## Features

- **Route links by domain** — send `github.com` to Chrome "Work", `gitlab.com`
  to Firefox "Personal". Routes match exact hosts and subdomains
  (`gist.github.com` follows the `github.com` route); the longest match wins.
- **Profile launching** — Chromium browsers launch with `--profile-directory`,
  Firefox with `-P`. Profiles are read straight from the browser's own config
  (`Local State` / `profiles.ini`).
- **Auto-detect installed browsers** — Chrome, Chromium, Brave, Edge, Vivaldi,
  Helium, Firefox, LibreWolf, Zen, Safari; snap/flatpak and Windows/macOS/Linux
  layouts included. Add anything else by picking its executable and profile
  folder.
- **Set as default browser** — one click on macOS (LaunchServices) and Linux
  (`xdg-settings` via a bundled script).
- **One app, no window clutter** — a borderless always-on-top panel that hides
  itself after use and reappears from the tray icon. Closing hides; it never
  quits until you say Quit.
- **`gh:` / `github:` shortcuts** — `gh:owner/repo` is accepted as a URL
  argument and in routes.

## How a link flows

1. The system hands Porter a URL (default browser registration, single-instance
   forward, or macOS AppleEvent).
2. Porter checks its routes for the best matching origin.
3. No route? The default browser/profile wins. Still nothing? Whatever you
   picked last — or the first browser in the list.
4. The panel shows the URL and browser; press Enter (or click Launch) and the
   URL is handed to the real browser with the chosen profile.

## Development

Requires Node.js, Rust, and the [Tauri v2
prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform.

```bash
npm install
npm run dev      # tauri dev
npm run build    # tauri build (bundled app in src-tauri/target)
```

### Tests

```bash
cd src-tauri && cargo test
```

Covers route matching (subdomains, longest match, no cross-domain leaks),
launch argument plans, profile parsing, config save/load, and URL validation.

## Setting Porter as the default browser

- **macOS** — click "Set as default browser" in the Config tab (Porter must be
  installed as `Porter.app`). If links still open elsewhere, check System
  Settings › Desktop & Dock.
- **Linux** — the same button runs the bundled `install-default-browser.sh`,
  which writes `~/.local/share/applications/porter.desktop` and points
  `xdg-settings` at it. Revert with:

  ```bash
  ./install-default-browser.sh /path/to/porter --revert
  ```

## Configuration

State lives in `porter.json` under the platform config dir:

| Platform | Path |
| --- | --- |
| Linux | `~/.config/dev.porter.app/porter.json` |
| macOS | `~/Library/Application Support/dev.porter.app/porter.json` |
| Windows | `%APPDATA%\dev.porter.app\porter.json` |

It stores browsers, routes/presets, and the default browser + profile. Deleting
it resets Porter to a fresh, auto-detected install.

## License

Private project. All rights reserved.
