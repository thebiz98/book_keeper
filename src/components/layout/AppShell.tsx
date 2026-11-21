import React from 'react';
import { Header } from './Header';
import { Sidebar } from './Sidebar';
import { HotkeyBar } from './HotkeyBar';

export interface AppShellProps {
  children: React.ReactNode;
}

export const AppShell: React.FC<AppShellProps> = ({ children }) => {
  return (
    <div className="h-screen w-screen overflow-hidden flex flex-col bg-slate-100 dark:bg-slate-950 text-slate-900 dark:text-slate-100 font-sans antialiased">
      <Header />
      <div className="flex-1 flex overflow-hidden">
        <Sidebar />
        <main className="flex-1 overflow-y-auto bg-slate-100 dark:bg-slate-950 p-4 relative">
          {children}
        </main>
      </div>
      <HotkeyBar />
    </div>
  );
};

export default AppShell;
