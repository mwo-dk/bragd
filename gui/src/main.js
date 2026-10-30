import { mount } from "svelte";
import App from "./App.svelte";
import "./looks.css";
import "./app.css";

mount(App, { target: /** @type {HTMLElement} */ (document.getElementById("app")) });
