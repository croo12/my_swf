// Transport to the Factory server. Inside Tauri the request goes through the native
// `factory_request` command (no CORS, works on macOS/Windows/Android); in a browser it uses fetch.

const BASE_KEY = 'factory-api-base-v1';
const TOKEN_KEY = 'factory-api-token-session';

type Invoke = (command: string, args: Record<string, unknown>) => Promise<unknown>;

declare global {
  interface Window {
    __TAURI__?: { core?: { invoke?: Invoke } };
  }
}

export const getBaseUrl = () => localStorage.getItem(BASE_KEY) ?? '';
export const getApiToken = () => sessionStorage.getItem(TOKEN_KEY) ?? '';

const tauriInvoke = () => window.__TAURI__?.core?.invoke;

/** True when the app can keep the token in the OS credential store (the Tauri desktop app). */
export const hasSecureTokenStore = () => Boolean(tauriInvoke());

/**
 * The URL is remembered in localStorage. The token is kept in the OS credential store by the
 * native side (Windows Credential Manager / macOS Keychain); in a plain browser it lasts one session.
 */
export async function saveConnection(url: string, token: string): Promise<void> {
  localStorage.setItem(BASE_KEY, url);
  sessionStorage.setItem(TOKEN_KEY, token);
  await tauriInvoke()?.('save_token', { token });
}

/** Loads the stored token at startup so the user does not re-enter it on every launch. */
export async function restoreToken(): Promise<void> {
  if (getApiToken()) return;
  try {
    const token = await tauriInvoke()?.('load_token', {});
    if (typeof token === 'string' && token) sessionStorage.setItem(TOKEN_KEY, token);
  } catch {
    // An unreadable credential store only means the token has to be entered again.
  }
}

export const isAllowedServerUrl = (url: string) =>
  /^https:\/\/[^/]+/.test(url) || /^http:\/\/(localhost|127\.0\.0\.1)(:\d+)?$/.test(url);

export async function apiRequest<T>(path: string, method: 'GET' | 'POST' | 'PUT' = 'GET', body?: unknown): Promise<T> {
  const base = getBaseUrl().replace(/\/+$/, '');
  const token = getApiToken();
  if (!base || !token) throw new Error('먼저 ⚙ 서버 연결에서 URL과 토큰을 설정하세요.');
  const invoke = window.__TAURI__?.core?.invoke;
  if (invoke) {
    return (await invoke('factory_request', { apiBase: base, token, path, method, body: body ?? null })) as T;
  }
  const response = await fetch(base + path, {
    method,
    cache: 'no-store',
    headers: { Authorization: 'Bearer ' + token, 'Content-Type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!response.ok) throw new Error('HTTP ' + response.status + ': ' + (await response.text()).slice(0, 160));
  return (await response.json()) as T;
}

/** Tauri rejects with a plain string, fetch with an Error. */
export const errorMessage = (err: unknown) => (err instanceof Error ? err.message : String(err));

// ── Connection status ──────────────────────────────────────────────────────
// Kept outside React so the first check can run at startup; components read it with useSyncExternalStore.

export interface ServerStatus { text: string; color?: string }

let serverStatus: ServerStatus = { text: '● 오프라인 UI' };
const statusListeners = new Set<() => void>();

export const getServerStatus = () => serverStatus;

export function subscribeServerStatus(listener: () => void): () => void {
  statusListeners.add(listener);
  return () => statusListeners.delete(listener);
}

/** Asks the server for its status and publishes the result. Resolves to whether the server answered. */
export async function checkConnection(): Promise<boolean> {
  let online = true;
  try {
    const data = await apiRequest<{ repository?: string }>('/api/status');
    serverStatus = { text: '● Factory 연결 · ' + (data.repository?.split(/[\\/]/).pop() || '온라인'), color: '#4ed2aa' };
  } catch {
    serverStatus = { text: '● 연결되지 않음', color: '#f0b982' };
    online = false;
  }
  statusListeners.forEach((listener) => listener());
  return online;
}
