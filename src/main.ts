import { mount } from "svelte";
import "./styles.css";
import App from "./App.svelte";
import { lang } from "./lib/i18n.svelte";

document.documentElement.lang = lang();

export default mount(App, { target: document.getElementById("app")! });
