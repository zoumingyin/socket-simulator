/**
 * main.tsx - 前端入口
 */
import '@ant-design/v5-patch-for-react-19';
import React from 'react';
import ReactDOM from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import App from './App.jsx';
import './index.css';

// 仅在 Tauri 环境放开 F12 打开 DevTools（打包后的 release 应用也生效）
if ('__TAURI_INTERNALS__' in window) {
  window.addEventListener('keydown', (e) => {
    if (e.key === 'F12') {
      e.preventDefault();
      invoke('open_devtools').catch((err) =>
        console.error('[F12] open_devtools failed:', err)
      );
    }
  });
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
