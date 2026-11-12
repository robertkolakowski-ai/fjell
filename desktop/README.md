# desktop

Tauri 2-skall (ARKITEKTUR beslutning 11, 17, 21). Foreløpig bare skall og en enkel tilstandsside, ingen UI-komponenter (M6).

- `src-tauri/` er et eget Cargo-workspace utenfor `core/`, slik at `just check` ikke krever webkit2gtk/WebView2.
  - `shell/` (`fjell-shell`): kommandologikk og DTO-er mot `FjellApi` (foreløpig `MemApi`). Ren Rust, testes overalt.
  - roten (`fjell-desktop`): tynne `#[tauri::command]`-omslag (`get_state`, `start_scan`, `list_cards`, `decide_cards`, `undo`). Ingen sletting herfra (beslutning 17).
- `src/`: Svelte 5 + TypeScript + Vite. Systemfonter, alt bundlet, ingen CDN. `src/bridge/` er embryo til `CoreBridge`.

Kommandoer: `just desktop-check` (shell-test/clippy, npm ci + svelte-check + vite build, full Tauri-sjekk bare hvis webkit2gtk-4.1 finnes), `npm run tauri dev` (krever Tauri-forutsetninger).
Ikke verifisert på Linux uten webkit: kompilering av `fjell-desktop` og selve vinduet. Ikon er plassholder.
