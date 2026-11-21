import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import { useThemeStore } from '@/store/useThemeStore';
import './index.css';

// Apply persisted theme (adds/removes the `dark` class on <html>).
useThemeStore.getState().setTheme(useThemeStore.getState().theme);

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
