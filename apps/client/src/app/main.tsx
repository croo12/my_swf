import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { initConnection } from '@/features/connect-server';
import { WorkbenchPage } from '@/pages/workbench';
import './styles/index.css';

void initConnection();

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <WorkbenchPage />
  </StrictMode>,
);
