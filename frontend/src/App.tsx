import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { ToastProvider } from './components/Toast';
import Layout from './components/Layout';
import ProtectedRoute from './components/ProtectedRoute';
import Login from './pages/Login';
import Dashboard from './pages/Dashboard';
import Providers from './pages/Providers';
import UpstreamKeys from './pages/UpstreamKeys';
import Models from './pages/Models';
import ClientKeys from './pages/ClientKeys';
import Usage from './pages/Usage';
import Routing from './pages/Routing';
import Docs from './pages/Docs';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      retry: 1,
    },
  },
});

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
      <BrowserRouter>
        <Routes>
          <Route path="/admin/login" element={<Login />} />
          <Route
            path="/admin"
            element={
              <ProtectedRoute>
                <Layout />
              </ProtectedRoute>
            }
          >
            <Route index element={<Dashboard />} />
            <Route path="providers" element={<Providers />} />
            <Route path="keys" element={<UpstreamKeys />} />
            <Route path="models" element={<Models />} />
            <Route path="routing" element={<Routing />} />
            <Route path="docs" element={<Docs />} />
            <Route path="clients" element={<ClientKeys />} />
            <Route path="usage" element={<Usage />} />
          </Route>
          <Route path="*" element={<Navigate to="/admin/" replace />} />
        </Routes>
      </BrowserRouter>
      </ToastProvider>
    </QueryClientProvider>
  );
}
