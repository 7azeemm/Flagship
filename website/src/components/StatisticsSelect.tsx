import { useEffect, useId, useRef, useState } from 'react';
import { Check, ChevronDown } from 'lucide-react';
import { money } from '../lib/catalog';

type Option = { value: string; label: string; count?: number; disabled?: boolean };

export function StatisticsSelect({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: string;
  options: Option[];
  onChange: (value: string) => void;
}) {
  const id = useId();
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const typeahead = useRef({ text: '', time: 0 });
  const [open, setOpen] = useState(false);
  const [placement, setPlacement] = useState({ above: false, height: 320 });
  const selected = options.find((option) => option.value === value);
  const openMenu = () => {
    const rect = trigger.current?.getBoundingClientRect();
    if (rect) {
      const below = window.innerHeight - rect.bottom - 12;
      const above = rect.top - 12;
      const useAbove = below < 250 && above > below;
      setPlacement({
        above: useAbove,
        height: Math.max(100, Math.min(320, useAbove ? above : below)),
      });
    }
    typeahead.current = { text: '', time: 0 };
    setOpen(true);
  };
  useEffect(() => {
    if (!open) return;
    const option =
      menu.current?.querySelector<HTMLButtonElement>('[aria-selected="true"]') ||
      menu.current?.querySelector<HTMLButtonElement>('button:not(:disabled)');
    option?.focus({ preventScroll: true });
    if (option && menu.current)
      menu.current.scrollTop = option.offsetTop - menu.current.clientHeight / 2;
    const outside = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener('pointerdown', outside);
    return () => document.removeEventListener('pointerdown', outside);
  }, [open]);
  return (
    <div
      className={`statistics-select${value ? ' is-refined' : ''}`}
      data-label={label}
      ref={root}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) setOpen(false);
      }}
      onKeyDown={(event) => {
        if (event.key === 'Escape' && open) {
          event.preventDefault();
          event.stopPropagation();
          setOpen(false);
          trigger.current?.focus();
          return;
        }
        if (!open) return;
        const items = [
          ...(menu.current?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)') || []),
        ];
        const current = items.indexOf(document.activeElement as HTMLButtonElement);
        let next = -1;
        if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) {
          next =
            event.key === 'Home'
              ? 0
              : event.key === 'End'
                ? items.length - 1
                : (current + (event.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length;
        } else if (
          event.key.length === 1 &&
          event.key !== ' ' &&
          !event.ctrlKey &&
          !event.metaKey &&
          !event.altKey
        ) {
          const now = Date.now();
          typeahead.current = {
            text:
              (now - typeahead.current.time < 800 ? typeahead.current.text : '') +
              event.key.toLocaleLowerCase(),
            time: now,
          };
          for (let offset = 1; offset <= items.length; offset++) {
            const index = (current + offset + items.length) % items.length;
            if (
              items[index].dataset.label?.toLocaleLowerCase().startsWith(typeahead.current.text)
            ) {
              next = index;
              break;
            }
          }
        }
        if (next >= 0) {
          event.preventDefault();
          items[next]?.focus({ preventScroll: true });
          items[next]?.scrollIntoView({ block: 'nearest' });
        }
      }}
    >
      <span className="statistics-select-label" id={`${id}-label`}>
        {label}
      </span>
      <button
        type="button"
        className="statistics-select-trigger"
        ref={trigger}
        aria-labelledby={`${id}-label ${id}-value`}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        onClick={() => {
          if (open) setOpen(false);
          else openMenu();
        }}
        onKeyDown={(event) => {
          if (!open && ['ArrowDown', 'ArrowUp'].includes(event.key)) {
            event.preventDefault();
            openMenu();
          }
        }}
      >
        <span id={`${id}-value`} title={selected?.label || value}>
          {selected?.label || value}
        </span>
        {selected?.count != null && (
          <small className="statistics-select-count">{money(selected.count)}</small>
        )}
        <ChevronDown size={14} />
      </button>
      {open && (
        <div
          id={id}
          role="listbox"
          aria-labelledby={`${id}-label`}
          ref={menu}
          className={`statistics-select-menu${placement.above ? ' opens-above' : ''}`}
          style={{ maxHeight: placement.height }}
        >
          {options.map((option) => (
            <button
              type="button"
              key={option.value}
              role="option"
              aria-selected={value === option.value}
              disabled={option.disabled}
              tabIndex={-1}
              data-label={option.label}
              onClick={() => {
                onChange(option.value);
                setOpen(false);
                trigger.current?.focus();
              }}
            >
              <span title={option.label}>{option.label}</span>
              {option.count != null && (
                <small className="statistics-select-count">{money(option.count)}</small>
              )}
              <span className="statistics-select-check">
                {value === option.value && <Check size={14} />}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
