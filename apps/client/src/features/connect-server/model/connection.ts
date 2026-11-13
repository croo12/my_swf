// Connection to the Factory server: settings dialog state and the online/offline indicator.
import { apiRequest, errorMessage, isAllowedServerUrl, restoreToken, saveConnection } from '@/shared/api';
import { createStore } from '@/shared/lib';
import { notify } from '@/shared/ui/toast';

/** What the server's repository is checked out on. Planning reads that working tree as it is. */
export interface GitState {
  branch: string;
  commit: string;
  committedAt: string;
  uncommittedFiles: number;
}

export interface ServerStatus {
  text: string;
  color?: string;
  /** Longer explanation shown as a tooltip. */
  detail?: string;
}

export const serverStatusStore = createStore<ServerStatus>({ text: '● 오프라인 UI' });
export const connectionDialogStore = createStore(false);

export const openConnectionDialog = () => connectionDialogStore.set(true);
export const closeConnectionDialog = () => connectionDialogStore.set(false);

/** Asks the server for its status and updates the indicator. Resolves to whether the server answered. */
export async function checkConnection(): Promise<boolean> {
  try {
    const data = await apiRequest<{ repository?: string; git?: GitState | null; version?: string }>('/api/status');
    const name = data.repository?.split(/[\\/]/).pop() || '온라인';
    const git = data.git ?? null;
    const based = git ? ` · ${git.branch} @ ${git.commit}${git.uncommittedFiles > 0 ? ' +변경' : ''}` : '';
    serverStatusStore.set({
      text: `● ${name}${based}`,
      color: '#4ed2aa',
      detail: git
        ? [
            `계획은 서버의 이 작업 폴더를 그대로 읽습니다 (fetch·pull 하지 않음).`,
            `브랜치: ${git.branch}`,
            `커밋: ${git.commit} (${new Date(git.committedAt).toLocaleString()})`,
            git.uncommittedFiles > 0 ? `커밋하지 않은 변경: 파일 ${git.uncommittedFiles}개` : '커밋하지 않은 변경 없음',
            data.version ? `서버 버전: ${data.version}` : '',
          ]
            .filter(Boolean)
            .join('\n')
        : undefined,
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
