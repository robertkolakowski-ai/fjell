// Inngang: monterer App. Ingen logikk her.
import { mount } from "svelte";
import "./app.css";
import App from "./App.svelte";

mount(App, { target: document.getElementById("app")! });
