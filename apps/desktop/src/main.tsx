import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { applyTheme } from "./theme";
import { getLang } from "./i18n";
import "./index.css";

// Appliquer thème + langue AVANT le rendu (pas de flash).
applyTheme();
document.documentElement.lang = getLang();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
