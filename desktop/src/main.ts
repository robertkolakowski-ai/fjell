// Inngang: monterer App. Dev-galleriet (?gallery) lastes bare i dev-serveren og havner ikke i produksjonsbygget.
import { mount } from "svelte";
import "./lib/tokens.css";
import "./lib/base.css";
import "./app.css";
import App from "./App.svelte";
import { bridge } from "./bridge/bridge";
import { initLanguage } from "./lib/i18n";

const target = document.getElementById("app")!;
if (import.meta.env.DEV && new URLSearchParams(location.search).has("gallery")) {
  void import("./Gallery.svelte").then((m) => mount(m.default, { target }));
} else {
  // Språkvalget lagres i skallet (shell.json); localStorage er reserve og migreres ved første start. Svarer skallet feil, brukes reserven.
  void initLanguage(bridge);
  mount(App, { target });
}
