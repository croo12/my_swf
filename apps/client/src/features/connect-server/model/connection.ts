// Connection to the Factory server: settings dialog state and the online/offline indicator.
import { apiRequest, errorMessage, isAllowedServerUrl, restoreToken, saveConnection } from '@/shared/api';
import { createStore } from '@/shared/lib';
import { notify } from '@/shared/ui/toast';

export interface ServerStatus {
  text: string;
  color?: string;
}

export const serverStatusStore = createStore<ServerStatus>({ text: '● 오프라인 UI' });
export const connectionDialogStore = createStore(false);

export const openConnectionDialog = () => connectionDialogStore.set(true);
export const closeConnectionDialog = () => connectionDialogStore.set(false);

/** Asks the server for its status and updates the indicator. Resolves to whether the server answered. */
export async function checkConnection(): Promise<boolean> {
  try {
    const data = await apiRequest<{ repository?: string }>('/api/status');
    serverStatusStore.set({
      text: '● Factory 연결 · ' + (data.repository?.split(/[\\/]/).pop() || '온라인'),
      color: '#4ed2aa',
    });
    return true;
  } catch {
    serverStatusStore.set({ text: '● 연결되지 않음', color: '#f0b982' });
    return false;
  }
}

/** Run once at startup: load the stored token, then show whether the server is reachable. */
export const initConnection = () => restoreToken().then(checkConnection);

/** Saves the settings entered in the dialog and verifies them against the server. */
export async function submitConnection(url: string, token: string): Promise<void> {
  if (!isAllowedServerUrl(url)) {
    notify('HTTPS 서버 주소를 입력하세요. HTTP는 localhost만 허용됩니다.');
    return;
  }
  closeConnectionDialog();
  try {
    await saveConnection(url, token);
  } catch (err) {
    notify('토큰을 보안 저장소에 저장하지 못했습니다: ' + errorMessage(err));
    return;
  }
  notify((await checkConnection()) ? '서버와 연결되었습니다.' : '연결 확인 실패: 서버 주소, 토큰, Tailscale 확인');
}
