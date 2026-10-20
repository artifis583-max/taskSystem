import React from "react";
import ReactDOM from "react-dom/client";
import "./styles.css";
import App from "./App";
import { Dashboard } from "./Dashboard";
import { inTauri, loadApi } from "./api";

/** In the app, the window label decides the view. In a browser preview, `?view=dashboard` does. */
async function currentView(): Promise<"dashboard" | "widget"> {
  if (inTauri) {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    return getCurrentWindow().label === "dashboard" ? "dashboard" : "widget";
  }
  return new URLSearchParams(location.search).get("view") === "dashboard" ? "dashboard" : "widget";
}

Promise.all([loadApi(), currentView()]).then(([api, view]) => {
  document.documentElement.classList.add(inTauri ? "tauri" : "browser", view);
  ReactDOM.createRoot(document.getElementById("root")!).render(
    <React.StrictMode>{view === "dashboard" ? <Dashboard api={api} /> : <App api={api} />}</React.StrictMode>,
  );
});
