import React from "react";
import ReactDOM from "react-dom/client";
import "./styles.css";
import App from "./App";
import { Dashboard } from "./Dashboard";
import { inTauri, loadApi } from "./api";

const view = new URLSearchParams(location.search).get("view") === "dashboard" ? "dashboard" : "widget";
document.documentElement.classList.add(inTauri ? "tauri" : "browser", view);

loadApi().then((api) => {
  ReactDOM.createRoot(document.getElementById("root")!).render(
    <React.StrictMode>{view === "dashboard" ? <Dashboard api={api} /> : <App api={api} />}</React.StrictMode>,
  );
});
