import React from "react";
import ReactDOM from "react-dom/client";
import "./styles.css";
import App from "./App";
import { inTauri, loadApi } from "./api";

document.documentElement.classList.add(inTauri ? "tauri" : "browser");

loadApi().then((api) => {
  ReactDOM.createRoot(document.getElementById("root")!).render(
    <React.StrictMode>
      <App api={api} />
    </React.StrictMode>,
  );
});
