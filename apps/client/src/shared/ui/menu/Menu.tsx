import { useState } from 'react';

export interface MenuItem {
  label: string;
  /** Short explanation shown under the label. */
  hint?: string;
  danger?: boolean;
  onSelect: () => void;
}

/** A button that opens a dropdown list of actions. Clicking anywhere else closes it. */
export function Menu({ label, items }: { label: string; items: MenuItem[] }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="menu">
      <button
        className={open ? 'btn active' : 'btn'}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      >
        {label} ▾
      </button>
      {open && (
        <>
          <div className="menu-backdrop" onClick={() => setOpen(false)} />
          <div className="menu-list" role="menu">
            {items.map((item) => (
              <button
                key={item.label}
                role="menuitem"
                className={item.danger ? 'menu-item danger' : 'menu-item'}
                onClick={() => {
                  setOpen(false);
                  item.onSelect();
                }}
              >
                <span>{item.label}</span>
                {item.hint && <small>{item.hint}</small>}
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
