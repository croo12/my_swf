import { useStore } from '@/shared/lib';
import { applyProposal, jobStore } from '../model/job';

/** Status line of the current planning job, with the button that applies a finished proposal. */
export function JobBar() {
  const job = useStore(jobStore);
  if (!job) return null;
  return (
    <div className="jobbar">
      <span>{job.message}</span>
      {job.proposal && (
        <button className="btn primary tiny" onClick={applyProposal}>
          제안 그래프 적용
        </button>
      )}
    </div>
  );
}
