import { useState } from 'react';
import { LAYERS, planStore } from '@/entities/plan';
import { repositoryStore, type RepoSlice } from '@/entities/repository';
import { addSliceToPlan, countSliceInPlan } from '@/features/add-slice-to-plan';
import { analyzeRepository, analyzingStore } from '@/features/analyze-repository';
import { useStore } from '@/shared/lib';

/**
 * Read-only overview of the analyzed repository at slice level. Nothing here can be edited;
 * selecting a slice shows what it depends on, and it can be brought into the plan for detailed work.
 */
export function RepositoryOverview() {
  const repository = useStore(repositoryStore);
  const analyzing = useStore(analyzingStore);
  const { plan } = useStore(planStore);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);

  if (!repository) {
    return (
      <div className="repo-empty">
        <p>아직 분석한 저장소가 없습니다.</p>
        <button className="btn primary" disabled={analyzing} onClick={analyzeRepository}>
          {analyzing ? '분석 중…' : '저장소 분석'}
        </button>
      </div>
    );
  }

  const selected = repository.slices.find((s) => s.key === selectedKey) ?? null;
  const imports = new Set(selected?.imports);
  const importedBy = new Set(selected?.importedBy);
  const relationOf = (slice: RepoSlice) => {
    if (!selected) return '';
    if (slice.key === selected.key) return ' selected';
    if (imports.has(slice.key)) return ' imports';
    if (importedBy.has(slice.key)) return ' imported-by';
    return ' dimmed';
  };

  return (
    <div className="repo">
      <div className="repo-main">
        <div className="repo-summary">
          <strong>{repository.sourceRoot || '저장소'}</strong>
          <span>슬라이스 {repository.slices.filter((slice) => slice.segments.length > 0).length}개</span>
          <span>세그먼트 {repository.nodes.length}개</span>
          <span>import 관계 {repository.edges.length}개</span>
          {repository.unresolvedImports > 0 && <span>추적 못 한 import {repository.unresolvedImports}건</span>}
          <span>분석 {new Date(repository.analyzedAt).toLocaleString()}</span>
          <button className="btn tiny" disabled={analyzing} onClick={analyzeRepository}>
            {analyzing ? '분석 중…' : '다시 분석'}
          </button>
        </div>
        {LAYERS.map((layer) => {
          const slices = repository.slices.filter((s) => s.layer === layer);
          return (
            <section key={layer} className="repo-lane" data-layer={layer}>
              <header>
                <span className="repo-lane-name">{layer.toUpperCase()}</span>
                <span className="repo-lane-count">{slices.length}</span>
              </header>
              <div className="repo-chips">
                {slices.map((slice) => (
                  <button
                    key={slice.key}
                    className={'repo-chip' + relationOf(slice)}
                    title={slice.key}
                    onClick={() => setSelectedKey(slice.key === selectedKey ? null : slice.key)}
                  >
                    <span className="repo-chip-name">{slice.name}</span>
                    {slice.segments.length > 0 && <span className="repo-chip-count">{slice.segments.length}</span>}
                  </button>
                ))}
              </div>
            </section>
          );
        })}
      </div>
      {selected && (
        <SliceDetail
          slice={selected}
          inPlan={countSliceInPlan(selected.key, plan.nodes)}
          onSelect={setSelectedKey}
          onClose={() => setSelectedKey(null)}
        />
      )}
    </div>
  );
}

interface SliceDetailProps {
  slice: RepoSlice;
  /** How many of the slice's segments are already in the plan. */
  inPlan: number;
  onSelect: (key: string) => void;
  onClose: () => void;
}

function SliceDetail({ slice, inPlan, onSelect, onClose }: SliceDetailProps) {
  const total = Math.max(slice.segments.length, 1);
  return (
    <aside className="repo-detail" aria-label="슬라이스 정보">
      <div className="repo-detail-head">
        <div>
          <div className="panel-tag">{slice.layer.toUpperCase()}</div>
          <h2>{slice.name}</h2>
        </div>
        <button className="close" aria-label="닫기" onClick={onClose}>
          ×
        </button>
      </div>
      {slice.segments.length > 0 && (
        <div className="repo-detail-section">
          <h3>세그먼트 {slice.segments.length}</h3>
          <div className="repo-tags">
            {slice.segments.map((segment) => (
              <span key={segment}>{segment}</span>
            ))}
          </div>
        </div>
      )}
      <SliceLinks title="가져다 쓰는 슬라이스" kind="imports" keys={slice.imports} onSelect={onSelect} />
      <SliceLinks title="이 슬라이스를 쓰는 곳" kind="imported-by" keys={slice.importedBy} onSelect={onSelect} />
      <div className="repo-detail-actions">
        <button className="btn primary" disabled={inPlan >= total} onClick={() => addSliceToPlan(slice.key)}>
          {inPlan >= total ? '계획에 표시됨' : '계획에 기존 모듈로 표시'}
        </button>
        <span className="help">새 모듈이 의존하는 기존 코드를 계획 화면에 참고용으로 놓습니다.</span>
      </div>
    </aside>
  );
}

function SliceLinks({
  title,
  kind,
  keys,
  onSelect,
}: {
  title: string;
  kind: string;
  keys: string[];
  onSelect: (key: string) => void;
}) {
  return (
    <div className="repo-detail-section">
      <h3>
        <span className={'repo-legend ' + kind} /> {title} {keys.length}
      </h3>
      {keys.length === 0 && <div className="help">없음</div>}
      <div className="repo-links">
        {keys.map((key) => (
          <button key={key} onClick={() => onSelect(key)}>
            {key}
          </button>
        ))}
      </div>
    </div>
  );
}
