import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClientProvider } from "@tanstack/react-query";
import { queryClient, queryKeys } from "./hooks/queries";
import i18n from "./i18n";
import { tauriApi } from "./api";
import { useUIStore } from "./store/uiStore";
import App from "./App";
import "./App.css";

async function bootstrap() {
  try {
    const config = await tauriApi.getConfig();
    if (config?.language) {
      i18n.changeLanguage(config.language);
      useUIStore.getState().setLanguage(config.language);
    }
    queryClient.setQueryData(queryKeys.config(), config);
  } catch (err) {
    console.error("Failed to load initial config from backend:", err);
  }

  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <QueryClientProvider client={queryClient}>
        <App />
      </QueryClientProvider>
    </React.StrictMode>,
  );
}

bootstrap();

