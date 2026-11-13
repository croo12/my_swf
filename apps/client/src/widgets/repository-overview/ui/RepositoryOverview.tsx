import { useState } from 'react';
import { LAYERS, planStore } from '@/entities/plan';
import {
  loadSliceModules,
  repositoryStore,
  sliceModulesStore,
  type OutsideLink,
  type RepoSlice,
  type SliceModules,
} from '@/entities/repository';
import { addSliceToPlan, countSliceInPlan } from '@/features/add-slice-to-plan';
import { analyzeRepository, analyzingStore } from '@/features/analyze-repository';
import { setView } from '@/features/switch-view';
import { useStore } from '@/shared/lib';

/**
 * Read-only overview of the analyzed repository. Nothing here can be edited. Selecting a slice
 * shows the modules inside it, what each offers and who uses it, and a plan can be started from
 * that structure.
 */
export function RepositoryOverview() {
  const repository = useStore(repositoryStore);
  const analyzing = useStore(analyzingStore);
  const { plan } = useStore(planStore);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const select = (key: string | null) => {
    setSelectedKey(key);
    // The modules of a slice are read from the server when the slice is opened.
    if (key) void loadSliceModules(key);
  };

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
                    onClick={() => select(slice.key === selectedKey ? null : slice.key)}
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
          onSelect={select}
          onClose={() => select(null)}
        />
      )}
    </div>
  );
}

interface SliceDetailProps {
  slice: RepoSlice;
  /** How many modules of the slice are already in the plan. */
  inPlan: number;
  onSelect: (key: string) => void;
  onClose: () => void;
}

function SliceDetail({ slice, inPlan, onSelect, onClose }: SliceDetailProps) {
  const state = useStore(sliceModulesStore)[slice.key];
  const modules = state && !('status' in state) ? state : null;
  const startPlan = async () => {
    if (await addSliceToPlan(slice.key)) setView('plan');
  };
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
      <div className="repo-detail-actions">
        <button className="btn primary" disabled={!modules || inPlan >= modules.nodes.length} onClick={startPlan}>
          {modules && inPlan >= modules.nodes.length ? '계획에 있음' : '이 구조로 계획 시작'}
        </button>
        <span className="help">
          아래 모듈과 그 사이의 의존을 기존 모듈로 계획에 가져옵니다. 계획 화면에서 수정·신규로 바꾸며 설계합니다.
        </span>
      </div>
      {state && 'status' in state && (
        <div className="repo-detail-section help">
          {state.status === 'loading' ? '슬라이스 구조를 읽는 중…' : `구조를 읽지 못했습니다: ${state.reason}`}
        </div>
      )}
      {modules && <SliceStructure modules={modules} />}
      <SliceLinks title="가져다 쓰는 슬라이스" kind="imports" keys={slice.imports} onSelect={onSelect} />
      <SliceLinks title="이 슬라이스를 쓰는 곳" kind="imported-by" keys={slice.importedBy} onSelect={onSelect} />
    </aside>
  );
}

const namesTitle = (links: OutsideLink[]) =>
  links.map((link) => `${link.placement}: ${link.names.join(', ') || '전체'}`).join('\n');

/** The modules of the slice by segment: what each offers, and how much outside code uses it. */
function SliceStructure({ modules }: { modules: SliceModules }) {
  const segments = [...new Set(modules.nodes.map((node) => node.path))];
  return (
    <div className="repo-detail-section">
      <h3>
        모듈 {modules.nodes.length} · 내부 의존 {modules.edges.length}
      </h3>
      {segments.map((segment) => (
        <div key={segment} className="repo-segment">
          <div className="repo-segment-name">{segment.slice(modules.slice.length + 1) || segment}</div>
          {modules.nodes
            .filter((node) => node.path === segment)
            .map((node) => {
              const usedBy = modules.incoming.filter((link) => link.module === node.id);
              const uses = modules.outgoing.filter((link) => link.module === node.id);
              const inside = modules.edges.filter((edge) => edge.to === node.id).length;
              return (
                <details key={node.id} className="repo-module">
                  <summary>
                    <span className="repo-module-name">{node.name}</span>
                    <span className="repo-module-counts">
                      <span title="다른 파일이 가져다 쓰는 이름">노출 {node.interfaces.length}</span>
                      <span title={namesTitle(usedBy)}>밖에서 {usedBy.length}</span>
                      <span title="슬라이스 안에서 이 모듈을 쓰는 모듈">안에서 {inside}</span>
                    </span>
                  </summary>
                  <div className="repo-module-body">
                    <div className="repo-tags">
                      {node.interfaces.length === 0 && <span className="help">밖에서 쓰는 이름 없음</span>}
                      {node.interfaces.map((item) => (
                        <span key={item.name} className={'kind-' + item.kind}>
                          {item.name}
                        </span>
                      ))}
                    </div>
                    <OutsideList title="이 모듈을 쓰는 곳" links={usedBy} />
                    <OutsideList title="이 모듈이 쓰는 곳" links={uses} />
                    <div className="help">파일 {node.files?.length ?? 0}개</div>
                  </div>
                </details>
              );
            })}
        </div>
      ))}
    </div>
  );
}

function OutsideList({ title, links }: { title: string; links: OutsideLink[] }) {
  if (links.length === 0) return null;
  return (
    <div className="repo-outside">
      <div className="repo-outside-title">
        {title} {links.length}
      </div>
      {links.map((link) => (
        <div key={link.placement} title={link.names.join(', ')}>
          <span>{link.placement}</span>
          <small>{link.names.join(', ')}</small>
        </div>
      ))}
    </div>
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
