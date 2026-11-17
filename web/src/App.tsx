import { useCallback, useState } from "react";
import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AuthProvider, useAuth } from "./lib/auth";
import { ApiError } from "./api/client";
import { Layout } from "./components/Layout";
import { Splash } from "./components/Splash";
import { Login } from "./pages/Login";
import { Overview } from "./pages/Overview";
import { Detail } from "./pages/Detail";
import { Positions } from "./pages/Positions";
import { PositionDetail } from "./pages/PositionDetail";
import { Groups } from "./pages/Groups";
import { Orders } from "./pages/Orders";
import { SyncRuns } from "./pages/SyncRuns";
import { Accounts } from "./pages/Accounts";
import { Settings } from "./pages/Settings";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: (count, err) => !(err instanceof ApiError && (err.unauthorized || err.status === 404)) && count < 2,
      refetchOnWindowFocus: true,
    },
  },
});

function Gate() {
  const { token } = useAuth();
  if (!token) return <Login />;
  return (
    <Routes>
      <Route element={<Layout />}>
        <Route index element={<Overview />} />
        <Route path="etf" element={<Detail />} />
        <Route path="etf/:name" element={<Detail />} />
        <Route path="positions" element={<Positions />} />
        <Route path="positions/:id" element={<PositionDetail />} />
        <Route path="groups" element={<Groups />} />
        <Route path="orders" element={<Orders />} />
        <Route path="sync" element={<SyncRuns />} />
        <Route path="accounts" element={<Accounts />} />
        <Route path="settings" element={<Settings />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Route>
    </Routes>
  );
}

export default function App() {
  const [splash, setSplash] = useState(() => !sessionStorage.getItem("kgbctl.splashed"));
  const done = useCallback(() => { setSplash(false); try { sessionStorage.setItem("kgbctl.splashed", "1"); } catch { /* ignore */ } }, []);
  return (
    <QueryClientProvider client={queryClient}>
      <AuthProvider>
        <BrowserRouter>
          {splash && <Splash onDone={done} />}
          <Gate />
        </BrowserRouter>
      </AuthProvider>
    </QueryClientProvider>
  );
}
