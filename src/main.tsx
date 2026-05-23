import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { WorkdirProvider } from "./contexts/WorkdirContext";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <WorkdirProvider>
      <App />
    </WorkdirProvider>
  </React.StrictMode>,
);
