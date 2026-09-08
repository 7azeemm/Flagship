import { useEffect, useId, useRef, useState } from 'react';
import { ArrowDownWideNarrow, Check, ChevronDown } from 'lucide-react';
import type { CatalogQuery } from '../types';

const options = [
  { value: 'price_asc', label: 'Price: low to high' },
  { value: 'price_desc', label: 'Price: high to low' },
] as const;
export function SortSelect({
  value,
  onChange,
}: {
  value: CatalogQuery['sort'];
  onChange: (value: CatalogQuery['sort']) => void;
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const id = useId();
  useEffect(() => {
    if (!open) return;
    root.current?.querySelector<HTMLButtonElement>('[aria-checked="true"]')?.focus();
    const outside = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener('pointerdown', outside);
    return () => document.removeEventListener('pointerdown', outside);
  }, [open]);
  return (
    <div
      className="sort-select"
      ref={root}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) setOpen(false);
      }}
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          setOpen(false);
          trigger.current?.focus();
        }
        if (open && ['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) {
          event.preventDefault();
          const items = [
            ...(root.current?.querySelectorAll<HTMLButtonElement>('[role="menuitemradio"]') || []),
          ];
          const index = items.indexOf(document.activeElement as HTMLButtonElement);
          const next =
            event.key === 'Home'
              ? 0
              : event.key === 'End'
                ? items.length - 1
                : (index + (event.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length;
          items[next]?.focus();
        }
      }}
    >
      <button
        ref={trigger}
        className="sort-trigger"
        aria-label={`Sort products: ${options.find((option) => option.value === value)?.label}`}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={id}
        onClick={() => setOpen(!open)}
        onKeyDown={(event) => {
          if (!open && event.key === 'ArrowDown') {
            event.preventDefault();
            setOpen(true);
          }
        }}
      >
        <ArrowDownWideNarrow size={15} />
        {options.find((option) => option.value === value)?.label}
        <ChevronDown size={14} />
      </button>
      {open && (
        <div id={id} className="sort-menu" role="menu" aria-label="Sort products">
          {options.map((option) => (
            <button
              key={option.value}
              role="menuitemradio"
              aria-checked={value === option.value}
              onClick={() => {
                onChange(option.value);
                setOpen(false);
                trigger.current?.focus();
              }}
            >
              {option.label}
              {option.value === value && <Check size={15} />}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
