import { useStore } from '@/shared/lib';
import { checkStore, closeCheckResult } from '../model/check';

/** The result of the last plan check: every broken rule, or that there is none. */
export function CheckPanel() {
  const { result } = useStore(checkStore);
  if (!result) return null;
  const passed = result.errors.length === 0;
  return (
    <section className={'check-panel' + (passed ? ' passed' : '')} aria-label="계획 검사 결과">
      <header>
        <strong>{passed ? '검사 통과' : `규칙 위반 ${result.errors.length}건`}</strong>
        <span className="check-panel-meta">
          {result.basedOn && `기준: ${result.basedOn} · `}
          {new Date(result.checkedAt).toLocaleTimeString()}
        </span>
        <button className="close" aria-label="닫기" onClick={closeCheckResult}>
          ×
        </button>
      </header>
      {passed ? (
        <p>
          모든 정적 규칙을 통과했습니다. 형태와 저장소와의 일관성만 확인한 것이고, 설계가 좋은지는 판단하지 않습니다.
        </p>
      ) : (
        <ol>
          {result.errors.map((error) => (
            <li key={error}>{error}</li>
          ))}
        </ol>
      )}
    </section>
  );
}
