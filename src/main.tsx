import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { IslandWindow } from "./windows/island/IslandWindow";
import { PanelWindow } from "./windows/panel/PanelWindow";
import { QuickAddWindow } from "./windows/quickadd/QuickAddWindow";
import "./styles/base.css";

function applyWindowRootClass(label: string) {
  const rootClass =
    label === "panel" ? "panel-root" : label === "quickadd" ? "quickadd-root" : "island-root";
  document.documentElement.classList.add(rootClass);
}

async function bootstrap() {
  const label = getCurrentWindow().label;
  // 在 React 首屏前挂上窗口根类，打包版外链 CSS 未就绪时也能保持透明/无嵌套边
  applyWindowRootClass(label);

  let App: React.FC;
  switch (label) {
    case "panel":
      App = PanelWindow;
      break;
    case "quickadd":
      App = QuickAddWindow;
      break;
    case "island":
    default:
      App = IslandWindow;
      break;
  }

  ReactDOM.createRoot(document.getElementById("root")!).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>
  );
}

bootstrap();