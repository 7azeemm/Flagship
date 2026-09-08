import { useEffect, useRef, useState } from 'react';
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
import { sectionLabel } from '../lib/catalog';

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
  categories,
  onSection,
  onAbout,
}: {
  section: string;
  categories: ServerInfo['categories'];
  onSection: (section: string) => void;
  onAbout: () => void;
}) {
  const [theme, setTheme] = useState(document.documentElement.dataset.theme || 'dark');
  const [open, setOpen] = useState<string | null>(null);
  const root = useRef<HTMLElement>(null);
  const closeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const cancelClose = () => {
    if (closeTimer.current !== null) clearTimeout(closeTimer.current);
    closeTimer.current = null;
  };
  useEffect(() => cancelClose, []);
  const [hidden, setHidden] = useState(false);
  useEffect(() => {
    let previousY = Math.max(0, window.scrollY);
    let resizingUntil = 0;
    const onResize = () => {
      previousY = Math.max(0, window.scrollY);
      resizingUntil = performance.now() + 200;
      setHidden(false);
    };
    const onScroll = () => {
      const y = Math.max(0, window.scrollY);
      if (performance.now() < resizingUntil) {
        previousY = y;
        return;
      }
      const delta = y - previousY;
      if (y < 130) {
        setHidden(false);
      } else if (Math.abs(delta) >= 8) {
        setHidden(delta > 0);
        if (delta > 0) setOpen(null);
      } else {
        return;
      }
      previousY = y;
    };
    window.addEventListener('scroll', onScroll, { passive: true });
    window.addEventListener('resize', onResize);
    return () => {
      window.removeEventListener('scroll', onScroll);
      window.removeEventListener('resize', onResize);
    };
  }, []);
  useEffect(() => {
    const outside = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(null);
    };
    document.addEventListener('pointerdown', outside);
    return () => document.removeEventListener('pointerdown', outside);
  }, []);
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
  return (
    <header className={`site-header ${hidden ? 'is-hidden' : ''}`} ref={root}
      onPointerEnter={cancelClose}
      onPointerLeave={(event) => {
        if (event.pointerType === 'mouse') {
          cancelClose();
          closeTimer.current = setTimeout(() => setOpen(null), 250);
        }
      }}>
      <div className="header-inner">
        <a
          href="/GamingLaptop"
          aria-label="Mythos home"
          onClick={(event) => {
            if (!event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey) {
              event.preventDefault();
              setOpen(null);
              onSection('GamingLaptop');
            }
          }}
        >
          <Brand />
        </a>
        <div className="header-navigation">
        <nav
          className="header-categories"
          aria-label="Product categories"
        >
          {Object.entries(categories).map(([parent, children], index) => {
            const expanded = open === parent;
            const active = children.some((child) => child.id === section);
            const menuId = `category-menu-${index}`;
            return (
              <div
                className="header-category"
                key={parent}
                onPointerEnter={(event) => {
                  cancelClose();
                  if (event.pointerType === 'mouse') setOpen(parent);
                }}
                onBlur={(event) => {
                  if (!event.currentTarget.contains(event.relatedTarget)) setOpen(null);
                }}
                onKeyDown={(event) => {
                  if (event.key === 'Escape') {
                    setOpen(null);
                    event.currentTarget
                      .querySelector<HTMLButtonElement>('.category-trigger')
                      ?.focus();
                  }
                }}
              >
                <button
                  className={`category-trigger ${active ? 'active' : ''}`}
                  aria-expanded={expanded}
                  aria-controls={menuId}
                  onClick={() => setOpen(expanded ? null : parent)}
                  onKeyDown={(event) => {
                    if (event.key === 'ArrowDown') {
                      event.preventDefault();
                      setOpen(parent);
                      requestAnimationFrame(() =>
                        document
                          .getElementById(menuId)
                          ?.querySelector<HTMLAnchorElement>('a')
                          ?.focus(),
                      );
                    }
                  }}
                >
                  {parent}
                  <ChevronDown size={14} />
                </button>
                {expanded && (
                  <div className="mega-menu" id={menuId} onPointerEnter={cancelClose}>
                    <div className="mega-heading">
                      <span>{parent}</span>
                      <span>{children.length} categories</span>
                    </div>
                    <div
                      className="mega-links"
                      style={{
                        gridTemplateColumns: `repeat(${Math.ceil(children.length / 4)}, minmax(180px, 1fr))`,
                      }}
                    >
                      {children.map((child) => (
                        <a
                          key={child.id}
                          href={`/${child.id}`}
                          aria-current={child.id === section ? 'page' : undefined}
                          onClick={(event) => {
                            if (
                              !event.ctrlKey &&
                              !event.metaKey &&
                              !event.shiftKey &&
                              !event.altKey
                            ) {
                              event.preventDefault();
                              setOpen(null);
                              onSection(child.id);
                            }
                          }}
                        >
                          <span>{child.name}</span>
                          <ArrowUpRight size={14} />
                        </a>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </nav>
        <div className="future-navigation" aria-label="Upcoming features">
          <button className="future-link" disabled title="Coming soon">
            <Wrench size={14} />
            PC Builder
            <small>Soon</small>
          </button>
          <button className="future-link" disabled title="Coming soon">
            <Store size={14} />
            Marketplace
            <small>Soon</small>
          </button>
        </div>
        </div>
        <div className="header-right">
          <button className="header-icon" disabled aria-label="Account" title="Account coming soon">
            <UserRound size={17} />
          </button>
          <button
            className="header-icon"
            disabled
            aria-label="Wishlist"
            title="Wishlist coming soon"
          >
            <Heart size={17} />
          </button>
          <button
            className="header-icon theme-button"
            role="switch"
            aria-checked={theme === 'dark'}
            aria-label={theme === 'dark' ? 'Use light mode' : 'Use dark mode'}
            title={theme === 'dark' ? 'Use light mode' : 'Use dark mode'}
            onClick={changeTheme}
          >
            {theme === 'dark' ? <Sun size={17} /> : <Moon size={17} />}
          </button>
          <button className="header-about" onClick={onAbout} title="About Mythos">
            <Info size={15} />
            <span>About</span>
          </button>
        </div>
      </div>
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
