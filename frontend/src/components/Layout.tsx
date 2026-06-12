import { NavLink, Outlet } from 'react-router-dom';
import {
  LayoutDashboard,
  Server,
  Key,
  Cpu,
  GitFork,
  Users,
  BarChart2,
  BookOpen,
  LogOut,
} from 'lucide-react';

const navItems = [
  { to: '/admin/', label: 'Dashboard', icon: LayoutDashboard, end: true },
  { to: '/admin/providers', label: 'Providers', icon: Server },
  { to: '/admin/keys', label: 'Upstream Keys', icon: Key },
  { to: '/admin/models', label: 'Models', icon: Cpu },
  { to: '/admin/routing', label: 'Routing', icon: GitFork },
  { to: '/admin/docs', label: 'Docs', icon: BookOpen },
  { to: '/admin/clients', label: 'Client Keys', icon: Users },
  { to: '/admin/usage', label: 'Usage', icon: BarChart2 },
];

function handleLogout() {
  localStorage.removeItem('xplan_admin_token');
  window.location.href = '/admin/login';
}

export default function Layout() {
  return (
    <div className="flex h-screen bg-gray-950 text-gray-100">
      {/* Sidebar */}
      <aside className="w-56 flex flex-col border-r border-gray-800 bg-gray-900">
        <div className="px-4 py-5 border-b border-gray-800">
          <h1 className="text-lg font-semibold tracking-tight text-white">xplan</h1>
          <p className="text-xs text-gray-500 mt-0.5">Admin Panel</p>
        </div>
        <nav className="flex-1 px-2 py-3 space-y-0.5">
          {navItems.map(({ to, label, icon: Icon, end }) => (
            <NavLink
              key={to}
              to={to}
              end={end}
              className={({ isActive }) =>
                `flex items-center gap-2.5 px-3 py-2 rounded-md text-sm transition-colors ${
                  isActive
                    ? 'bg-indigo-600 text-white'
                    : 'text-gray-400 hover:text-gray-100 hover:bg-gray-800'
                }`
              }
            >
              <Icon size={16} />
              {label}
            </NavLink>
          ))}
        </nav>
        <div className="px-2 py-3 border-t border-gray-800">
          <button
            onClick={handleLogout}
            className="flex items-center gap-2.5 px-3 py-2 rounded-md text-sm text-gray-400 hover:text-gray-100 hover:bg-gray-800 w-full transition-colors"
          >
            <LogOut size={16} />
            Logout
          </button>
        </div>
      </aside>

      {/* Main content */}
      <main className="flex-1 overflow-auto">
        <Outlet />
      </main>
    </div>
  );
}
