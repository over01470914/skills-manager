import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { useEffect, useState, type ReactNode } from "react";
import { Toaster } from "sonner";
import { AppProvider } from "./context/AppContext";
import { ThemeProvider, useThemeContext } from "./context/ThemeContext";
import { HelpDialog } from "./components/HelpDialog";
import { CloseActionGuard } from "./components/CloseActionGuard";
import { FirstRunRestoreDialog } from "./components/FirstRunRestoreDialog";
import { Layout } from "./components/Layout";
import { Dashboard } from "./views/Dashboard";
import { MySkills } from "./views/MySkills";
import { WorkspaceView } from "./views/WorkspaceView";
import { CODING_WORKSPACE_CONFIG, LOBSTER_WORKSPACE_CONFIG } from "./views/workspaceConfigs";
import { InstallSkills } from "./views/InstallSkills";
import { Settings } from "./views/Settings";
import { ProjectDetail } from "./views/ProjectDetail";
import { Backup } from "./views/Backup";
import { Bundles } from "./views/Bundles";
import { isDesktop, webInvoke } from "./lib/tauri";

function WebAuthGate({ children }: { children: ReactNode }) {
  const [state, setState] = useState<"checking" | "ready" | "login" | "error">("checking");
  const [token, setToken] = useState("");
  const [message, setMessage] = useState("");
  useEffect(() => {
    void webInvoke<string>("get_central_repo_path").then(
      () => setState("ready"),
      (error) => {
        setMessage(String(error));
        setState(String(error).includes("Authentication required") ? "login" : "error");
      },
    );
  }, []);
  const signIn = async () => {
    sessionStorage.setItem("skills-manager:web-token", token);
    try {
      await webInvoke("get_central_repo_path");
      setState("ready");
      setMessage("");
    } catch (error) { setMessage(String(error)); }
  };
  if (state === "ready") return children;
  if (state === "checking") return <div className="p-8">連線中…</div>;
  return <div className="mx-auto mt-24 max-w-md rounded-xl border border-border bg-surface p-6">
    <h1 className="text-xl font-semibold">Skills Manager Web</h1>
    <p className="my-3 text-sm">{state === "login" ? "輸入伺服器的 SM_WEB_TOKEN" : "無法連接 Web 服務"}</p>
    {state === "login" && <><input type="password" className="w-full rounded border border-border bg-background p-2" value={token} onChange={(event) => setToken(event.target.value)} /><button className="mt-3 rounded bg-emerald-600 px-4 py-2 text-white" onClick={() => void signIn()}>登入</button></>}
    <p role="alert" className="mt-3 text-sm text-red-400">{message}</p>
  </div>;
}

function ThemedToaster() {
  const { resolvedTheme } = useThemeContext();
  return (
    <Toaster
      theme={resolvedTheme}
      position="bottom-right"
      toastOptions={{
        style: {
          background: "var(--color-surface)",
          border: "1px solid var(--color-border)",
          color: "var(--color-text-primary)",
        },
      }}
    />
  );
}

function App() {
  const application = (
    <ThemeProvider>
      <AppProvider>
        <BrowserRouter>
          <Routes>
            <Route element={<Layout />}>
              <Route path="/" element={<Dashboard />} />
              <Route path="/my-skills" element={<MySkills />} />
              <Route path="/library" element={<Navigate to="/my-skills" replace />} />
              <Route path="/global-workspace" element={<WorkspaceView config={CODING_WORKSPACE_CONFIG} />} />
              <Route path="/global-workspace/:agentKey" element={<WorkspaceView config={CODING_WORKSPACE_CONFIG} />} />
              <Route path="/lobster-workspace" element={<WorkspaceView config={LOBSTER_WORKSPACE_CONFIG} />} />
              <Route path="/lobster-workspace/:agentKey" element={<WorkspaceView config={LOBSTER_WORKSPACE_CONFIG} />} />
              <Route path="/install" element={<InstallSkills />} />
              <Route path="/backup" element={<Backup />} />
              <Route path="/bundles" element={<Bundles />} />
              <Route path="/project/:id" element={<ProjectDetail />} />
              <Route path="/settings" element={<Settings />} />
            </Route>
          </Routes>
          <HelpDialog />
          {isDesktop && <CloseActionGuard />}
          {isDesktop && <FirstRunRestoreDialog />}
        </BrowserRouter>
        <ThemedToaster />
      </AppProvider>
    </ThemeProvider>
  );
  return isDesktop ? application : <WebAuthGate>{application}</WebAuthGate>;
}

export default App;
