import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import { ThemeProvider } from './context/ThemeContext';
import './index.css';
import './styles/sthira-theme-tokens.css';
import './styles/home-shell.css';
import './styles/focus-workspace.css';
import './styles/connections.css';
import './styles/all-files.css';
import './styles/project-overview.css';
import './styles/object-workspace.css';
import './styles/graph-view.css';
import './styles/settings-modal.css';

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <ThemeProvider>
      <App />
    </ThemeProvider>
  </React.StrictMode>
);
