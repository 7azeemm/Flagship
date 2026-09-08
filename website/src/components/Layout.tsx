import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import {
  ArrowUpRight,
  ChevronDown,
  Cpu,
  Heart,
  Info,
  Moon,
  Store,
  Sun,
  UserRound,
  Wrench,
} from 'lucide-react';
import type { ServerInfo } from '../types';
import { readQuery, sectionLabel } from '../lib/catalog';
import '../header.css';

export function Brand() {
  return (
    <span className="brand">
      <img src="/favicon.svg" alt="" width="34" height="34" />
      mythos<span className="brand-period">.</span>
    </span>
  );
}

export function Header({
  section,
  filters,
  categories,
  onSection,
  onAbout,
}: {
  section: string;
  filters: Record<string, string[]>;
  categories: ServerInfo['categories'];
  onSection: (url: string) => void;
  onAbout: () => void;
}) {
  const [theme, setTheme] = useState(document.documentElement.dataset.theme || 'dark');
  const [open, setOpen] = useState<string | null>(null);
  const [hidden, setHidden] = useState(false);
  const [left, setLeft] = useState(12);
  const root = useRef<HTMLElement>(null);
  const trigger = useRef<HTMLButtonElement | null>(null);
  const panel = useRef<HTMLDivElement>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastScrollY = useRef(window.scrollY);
  const scrollDistance = useRef(0);
  const cancelClose = () => {
    if (timer.current !== null) clearTimeout(timer.current);
    timer.current = null;
  };
  const close = () => {
    cancelClose();
    setOpen(null);
  };
  const scheduleClose = () => {
    cancelClose();
    timer.current = setTimeout(() => {
      if (!panel.current?.contains(document.activeElement)) setOpen(null);
    }, 180);
  };
  useEffect(() => {
    const outside = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(null);
    };
    document.addEventListener('pointerdown', outside);
    return () => {
      cancelClose();
      document.removeEventListener('pointerdown', outside);
    };
  }, []);
  useEffect(() => {
    let frame = 0;
    const onScroll = () => {
      if (frame) return;
      frame = requestAnimationFrame(() => {
        const next = Math.max(0, window.scrollY);
        const delta = next - lastScrollY.current;
        const headerHeight = root.current?.offsetHeight || 0;
        if (Math.sign(delta) !== Math.sign(scrollDistance.current)) scrollDistance.current = 0;
        scrollDistance.current += delta;
        if (next <= headerHeight) {
          scrollDistance.current = 0;
          setHidden(false);
        } else if (scrollDistance.current > 12) {
          setHidden(true);
          setOpen(null);
          scrollDistance.current = 0;
        } else if (scrollDistance.current < -12) {
          setHidden(false);
          scrollDistance.current = 0;
        }
        lastScrollY.current = next;
        frame = 0;
      });
    };
    window.addEventListener('scroll', onScroll, { passive: true });
    return () => {
      window.removeEventListener('scroll', onScroll);
      if (frame) cancelAnimationFrame(frame);
    };
  }, []);
  useLayoutEffect(() => {
    if (!open) return;
    const position = () => {
      const bounds = root.current?.getBoundingClientRect();
      const anchor = trigger.current?.getBoundingClientRect();
      if (bounds && anchor)
        setLeft(
          Math.max(
            12,
            Math.min(
              anchor.left - bounds.left,
              bounds.width - (panel.current?.offsetWidth || 320) - 12,
            ),
          ),
        );
    };
    position();
    window.addEventListener('resize', position);
    const scroller = root.current?.querySelector('.masthead-scroll');
    scroller?.addEventListener('scroll', position);
    return () => {
      window.removeEventListener('resize', position);
      scroller?.removeEventListener('scroll', position);
    };
  }, [open]);
  const active = (url: string) => {
    const target = readQuery(new URL(url, window.location.origin));
    return (
      target.section === section &&
      Object.entries(target.filters).every(
        ([key, values]) =>
          values.length === filters[key]?.length &&
          values.every((value) => filters[key].includes(value)),
      )
    );
  };
  const navigate = (event: React.MouseEvent<HTMLAnchorElement>, url: string) => {
    if (!event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey) {
      event.preventDefault();
      close();
      onSection(url);
    }
  };
  const changeTheme = () => {
    const next = theme === 'dark' ? 'light' : 'dark';
    setTheme(next);
    document.documentElement.dataset.theme = next;
    try {
      localStorage.setItem('mythos-theme', next);
    } catch {
      /* Storage is optional. */
    }
    document
      .querySelector('meta[name="theme-color"]')
      ?.setAttribute(
        'content',
        getComputedStyle(document.documentElement).getPropertyValue('--bg').trim(),
      );
  };
  const children = open ? categories[open] || [] : [];
  return (
    <header
      className={`masthead ${hidden ? 'masthead-hidden' : ''}`}
      ref={root}
      onFocusCapture={() => setHidden(false)}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) close();
      }}
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          close();
          trigger.current?.focus();
        }
      }}
    >
      <div className="masthead-row">
        <a
          className="masthead-brand"
          href="/GamingLaptop"
          aria-label="Mythos home"
          onClick={(event) => navigate(event, '/GamingLaptop')}
        >
          <img src="/favicon.svg" alt="" width="30" height="30" />
          <span>
            mythos<b>.</b>
          </span>
        </a>
        <div className="masthead-upcoming" aria-label="Upcoming features">
          <button disabled title="Coming soon">
            <Wrench size={14} />
            PC Builder
          </button>
          <button disabled title="Coming soon">
            <Store size={14} />
            Marketplace
          </button>
        </div>
        <div className="masthead-actions">
          <button
            className="masthead-optional"
            disabled
            aria-label="Account"
            title="Account coming soon"
          >
            <UserRound size={17} />
          </button>
          <button
            className="masthead-optional"
            disabled
            aria-label="Wishlist"
            title="Wishlist coming soon"
          >
            <Heart size={17} />
          </button>
          <button
            role="switch"
            aria-checked={theme === 'dark'}
            aria-label={theme === 'dark' ? 'Use light mode' : 'Use dark mode'}
            onClick={changeTheme}
          >
            {theme === 'dark' ? <Sun size={17} /> : <Moon size={17} />}
          </button>
          <button className="masthead-optional" onClick={onAbout} aria-label="About Mythos">
            <Info size={17} />
          </button>
        </div>
      </div>
      <div className="masthead-category-row">
        <div className="masthead-scroll">
          <nav className="masthead-categories" aria-label="Product categories">
            {Object.entries(categories).map(([parent, items], index) => (
              <button
                key={parent}
                id={`masthead-trigger-${index}`}
                className={`masthead-trigger ${items.some((child) => active(child.url)) ? 'active' : ''}`}
                aria-expanded={open === parent}
                aria-controls={open === parent ? 'masthead-menu' : undefined}
                onPointerEnter={(event) => {
                  if (event.pointerType === 'mouse') {
                    cancelClose();
                    trigger.current = event.currentTarget;
                    setOpen(parent);
                  }
                }}
                onPointerLeave={(event) => {
                  if (event.pointerType === 'mouse') scheduleClose();
                }}
                onClick={(event) => {
                  trigger.current = event.currentTarget;
                  cancelClose();
                  const touch =
                    'pointerType' in event.nativeEvent &&
                    event.nativeEvent.pointerType === 'touch';
                  setOpen(open === parent && (event.detail === 0 || touch) ? null : parent);
                }}
                onKeyDown={(event) => {
                  if (event.key === 'ArrowDown') {
                    event.preventDefault();
                    trigger.current = event.currentTarget;
                    setOpen(parent);
                    requestAnimationFrame(() => panel.current?.querySelector('a')?.focus());
                  }
                }}
              >
                {parent}
                <ChevronDown size={13} />
              </button>
            ))}
          </nav>
        </div>
      </div>
      {open && (
        <div
          ref={panel}
          id="masthead-menu"
          className={`masthead-menu ${children.length > 6 ? 'masthead-menu-wide' : ''}`}
          style={{
            left,
            width: Math.min(720, Math.max(290, Math.ceil(children.length / 4) * 220)),
          }}
          aria-labelledby={trigger.current?.id}
          onPointerEnter={cancelClose}
          onPointerLeave={(event) => {
            if (event.pointerType === 'mouse') scheduleClose();
          }}
        >
          <div
            className="masthead-links"
            style={{ gridTemplateRows: `repeat(${Math.min(children.length, 4)}, minmax(40px, auto))` }}
          >
            {children.map((child) => (
              <a
                href={child.url}
                key={child.url}
                aria-current={active(child.url) ? 'page' : undefined}
                onClick={(event) => navigate(event, child.url)}
              >
                <span>{child.name}</span>
                <ArrowUpRight size={14} />
              </a>
            ))}
          </div>
        </div>
      )}
    </header>
  );
}
export function Footer({
  onSection,
  onAbout,
}: {
  onSection: (section: string) => void;
  onAbout: () => void;
}) {
  return (
    <footer className="site-footer">
      <div className="footer-main">
        <div className="footer-brand">
          <Brand />
          <p>Tech, in perspective.</p>
          <span>Find your next upgrade across Tunisian retailers.</span>
        </div>
        <div className="footer-links">
          <h2>Explore the catalog</h2>
          {['GamingLaptop', 'GPU', 'Monitor'].map((section) => (
            <a
              href={`/${section}`}
              key={section}
              onClick={(event) => {
                if (!event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey) {
                  event.preventDefault();
                  onSection(section);
                  window.scrollTo({ top: 0 });
                }
              }}
            >
              {sectionLabel(section)} <ArrowUpRight size={13} />
            </a>
          ))}
        </div>
        <div className="footer-about">
          <Cpu size={21} />
          <h2>A little clarity goes a long way.</h2>
          <p>
            Compare prices and specifications in one place. Purchase directly from the retailer you
            choose.
          </p>
          <button className="text-button" onClick={onAbout}>
            How Mythos works <ArrowUpRight size={14} />
          </button>
        </div>
      </div>
      <div className="footer-bottom">
        <span>© {new Date().getFullYear()} Mythos</span>
        <p>Prices and availability may change. Confirm details with the retailer.</p>
        <span>Tunisia / TND</span>
      </div>
    </footer>
  );
}
