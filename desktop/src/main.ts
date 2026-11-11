// Inngang: monterer App. Dev-galleriet (?gallery) lastes bare i dev-serveren og havner ikke i produksjonsbygget.
import { mount } from "svelte";
import "./lib/tokens.css";
import "./lib/base.css";
import "./app.css";
import App from "./App.svelte";

const target = document.getElementById("app")!;
if (import.meta.env.DEV && new URLSearchParams(location.search).has("gallery")) {
  void import("./Gallery.svelte").then((m) => mount(m.default, { target }));
} else {
  mount(App, { target });
}
