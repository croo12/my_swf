import { useStore } from '@/shared/lib';
import { toastStore } from './model';

export function Toast() {
  const { message, visible } = useStore(toastStore);
  return (
    <div className={visible ? 'toast show' : 'toast'} role="status">
      {message}
    </div>
  );
}
