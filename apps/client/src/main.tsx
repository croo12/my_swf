import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { checkConnection, restoreToken } from './api/factory';
import { App } from './App';
import './styles.css';

void restoreToken().then(checkConnection);

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
