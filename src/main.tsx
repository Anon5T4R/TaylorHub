import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { useLocale } from "./lib/i18n";
import { applyTheme, getTheme } from "./lib/theme";

// Antes do render: evita flash do tema padrão no boot.
applyTheme(getTheme());

// Remonta a árvore ao trocar de idioma → todo t() reavalia.
function Root() {
  const locale = useLocale();
  return <App key={locale} />;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>,
);
