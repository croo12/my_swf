import { useState } from 'react';
import { getApiToken, getBaseUrl } from '../api/factory';

interface ConnectionModalProps {
  onSave: (url: string, token: string) => void;
  onCancel: () => void;
}

export function ConnectionModal({ onSave, onCancel }: ConnectionModalProps) {
  const [url, setUrl] = useState(getBaseUrl);
  const [token, setToken] = useState(getApiToken);
  return (
    <div className="connection-modal">
      <div className="connection-card" role="dialog" aria-modal="true" aria-labelledby="connectionTitle">
        <h2 id="connectionTitle">Mac Studio 연결 설정</h2>
        <p>macOS · Windows · Android 모두 동일한 Factory 서버에 접속합니다. Tailscale Serve HTTPS 주소를 입력하세요.</p>
        <label>
          서버 URL{' '}
          <input type="url" spellCheck={false} placeholder="https://mac-studio.your-tailnet.ts.net" autoComplete="url" value={url} onChange={(ev) => setUrl(ev.target.value)} />
        </label>
        <label>
          API 토큰 (앱 세션에만 보관){' '}
          <input type="password" autoComplete="off" placeholder="FACTORY_API_TOKEN" value={token} onChange={(ev) => setToken(ev.target.value)} />
        </label>
        <div className="panel-actions">
          <button className="btn" onClick={onCancel}>취소</button>
          <button className="btn primary" onClick={() => onSave(url.trim().replace(/\/+$/, ''), token.trim())}>연결 확인</button>
        </div>
      </div>
    </div>
  );
}
