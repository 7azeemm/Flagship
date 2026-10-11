import { memo, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import {
  ArrowUpRight,
  ChevronDown,
  Cpu,
  LoaderCircle,
  Moon,
  Search,
  Heart,
  Bell,
  LogOut,
  Settings,
  Sun,
  Store,
  Hammer,
  Sparkles,
} from 'lucide-react';
import type { Product, ServerInfo } from '../types';
import { readQuery, sectionLabel } from '../lib/catalog';
import { parseGlobalSearch, request } from '../lib/api';
import '../header.css';
import { accountName, accountRequest, useAuth } from '../lib/auth';

type SearchOption = Pick<Product, 'id' | 'title' | 'section' | 'image' | 'price'>;

export function Brand() {
  return (
    <span className="brand">
      <img src="/icon.png" alt="" width="34" height="34" />
      Flagship<span className="brand-period">.</span>
    </span>
  );
}

export const Header = memo(function Header({
  section,
  locationKey,
  filters,
  categories,
  onSection,
  onProduct,
  onHome,
  onNavigate,
  onSignIn,
}: {
  section: string;
  locationKey: string;
  filters: Record<string, string[]>;
  categories: ServerInfo['categories'];
  onSection: (url: string) => void;
  onProduct: (id: string) => void;
  onHome: () => void;
  onNavigate: (path: string) => void;
  onSignIn: () => void;
}) {
  const { user, wishlist, loading: authLoading, signOut } = useAuth();
  const [alertCount, setAlertCount] = useState(0);
  useEffect(() => {
    setAlertCount(0);
    if (!user) return;
    let controller: AbortController | undefined;
    const refresh = () => {
      if (document.hidden) return;
      controller?.abort();
      const next = new AbortController();
      controller = next;
      void accountRequest<{ count: number }>('/alerts/count', { signal: next.signal })
        .then(({ count }) => {
          if (!next.signal.aborted) setAlertCount(count);
        })
        .catch(() => {});
    };
    refresh();
    const timer = window.setInterval(refresh, 30000);
    window.addEventListener('flagship:alerts-changed', refresh);
    document.addEventListener('visibilitychange', refresh);
    return () => {
      controller?.abort();
      clearInterval(timer);
      window.removeEventListener('flagship:alerts-changed', refresh);
      document.removeEventListener('visibilitychange', refresh);
    };
  }, [user?.id]);
  const [accountOpen, setAccountOpen] = useState(false);
  const [signOutError, setSignOutError] = useState('');
  const accountRoot = useRef<HTMLDivElement>(null);
  const accountTrigger = useRef<HTMLButtonElement>(null);
  const [theme, setTheme] = useState(document.documentElement.dataset.theme || 'dark');
  const [open, setOpen] = useState<string | null>(null);
  const [hidden, setHidden] = useState(false);
  const [left, setLeft] = useState(12);
  const [search, setSearch] = useState('');
  const [suggestions, setSuggestions] = useState<SearchOption[]>([]);
  const [searchPage, setSearchPage] = useState(1);
  const [searchTotalPages, setSearchTotalPages] = useState(0);
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchLoading, setSearchLoading] = useState(false);
  const [searchMoreLoading, setSearchMoreLoading] = useState(false);
  const [searchError, setSearchError] = useState(false);
  const [searchMoreError, setSearchMoreError] = useState(false);
  const [activeSuggestion, setActiveSuggestion] = useState(-1);
  const root = useRef<HTMLElement>(null);
  const trigger = useRef<HTMLButtonElement | null>(null);
  const panel = useRef<HTMLDivElement>(null);
  const searchMoreController = useRef<AbortController | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastScrollY = useRef(window.scrollY);
  const scrollDistance = useRef(0);
  const collapsed = hidden && !accountOpen && !open;
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
    setHidden(false);
    setOpen(null);
    setAccountOpen(false);
    setSearchOpen(false);
    lastScrollY.current = window.scrollY;
    scrollDistance.current = 0;
  }, [locationKey]);
  useEffect(() => {
    const outside = (event: PointerEvent) => {
      if (!accountRoot.current?.contains(event.target as Node)) setAccountOpen(false);
      if (!root.current?.contains(event.target as Node)) {
        setOpen(null);
        setSearchOpen(false);
        setAccountOpen(false);
      }
    };
    document.addEventListener('pointerdown', outside);
    return () => {
      cancelClose();
      document.removeEventListener('pointerdown', outside);
    };
  }, []);
  useEffect(() => {
    const value = search.trim();
    setActiveSuggestion(-1);
    setSearchError(false);
    setSearchMoreError(false);
    setSuggestions([]);
    setSearchPage(1);
    setSearchTotalPages(0);
    searchMoreController.current?.abort();
    searchMoreController.current = null;
    setSearchMoreLoading(false);
    if (value.length < 2) {
      setSearchLoading(false);
      return;
    }
    const controller = new AbortController();
    setSearchLoading(true);
    const timer = window.setTimeout(() => {
      request(`/search?q=${encodeURIComponent(value)}&page=1`, controller.signal)
        .then(parseGlobalSearch)
        .then((result) => {
          if (controller.signal.aborted) return;
          setSuggestions(
            result.items.map(({ id, title, section, image, price }) => ({
              id,
              title,
              section,
              image,
              price,
            })),
          );
          setSearchPage(result.page);
          setSearchTotalPages(result.total_pages);
        })
        .catch(() => {
          if (!controller.signal.aborted) {
            setSuggestions([]);
            setSearchError(true);
          }
        })
        .finally(() => {
          if (!controller.signal.aborted) setSearchLoading(false);
        });
    }, 220);
    return () => {
      window.clearTimeout(timer);
      controller.abort();
      searchMoreController.current?.abort();
    };
  }, [search]);
  useEffect(() => {
    let frame = 0;
    const onScroll = () => {
      if (frame) return;
      frame = requestAnimationFrame(() => {
        const maxScroll = Math.max(0, document.documentElement.scrollHeight - window.innerHeight);
        const next = Math.min(maxScroll, Math.max(0, window.scrollY));
        const delta = next - lastScrollY.current;
        const headerHeight = root.current?.offsetHeight || 0;
        if (Math.sign(delta) !== Math.sign(scrollDistance.current)) scrollDistance.current = 0;
        scrollDistance.current += delta;
        if (
          next <= headerHeight ||
          open ||
          accountOpen ||
          root.current?.contains(document.activeElement)
        ) {
          scrollDistance.current = 0;
          setHidden(false);
        } else if (scrollDistance.current > 18) {
          setHidden(true);
          setOpen(null);
          scrollDistance.current = 0;
        } else if (scrollDistance.current < -18) {
          setHidden(false);
          scrollDistance.current = 0;
        }
        lastScrollY.current = next;
        frame = 0;
      });
    };
    const onResize = () => {
      setHidden(false);
      lastScrollY.current = window.scrollY;
      scrollDistance.current = 0;
    };
    window.addEventListener('scroll', onScroll, { passive: true });
    window.addEventListener('resize', onResize);
    return () => {
      window.removeEventListener('scroll', onScroll);
      window.removeEventListener('resize', onResize);
      if (frame) cancelAnimationFrame(frame);
    };
  }, [open, accountOpen]);
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
  const activeUrls = useMemo(() => {
    const result = new Set<string>();
    for (const item of Object.values(categories).flat()) {
      const target = readQuery(new URL(item.url, window.location.origin));
      if (
        target.section === section &&
        Object.entries(target.filters).every(
          ([key, values]) =>
            values.length === filters[key]?.length &&
            values.every((value) => filters[key].includes(value)),
        )
      )
        result.add(item.url);
    }
    return result;
  }, [categories, filters, section]);
  const active = (url: string) => activeUrls.has(url);
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
      localStorage.setItem('flagship-theme', next);
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
  const loadMoreSearch = async () => {
    const value = search.trim();
    if (value.length < 2 || searchMoreController.current || searchPage >= searchTotalPages) return;
    const controller = new AbortController();
    searchMoreController.current = controller;
    setSearchMoreLoading(true);
    setSearchMoreError(false);
    try {
      const result = parseGlobalSearch(
        await request(
          `/search?q=${encodeURIComponent(value)}&page=${searchPage + 1}`,
          controller.signal,
        ),
      );
      if (controller.signal.aborted) return;
      setSuggestions((current) => {
        const seen = new Set(current.map((item) => item.id));
        const more = result.items
          .filter((item) => !seen.has(item.id))
          .map(({ id, title, section, image, price }) => ({ id, title, section, image, price }));
        return [...current, ...more];
      });
      setSearchPage(result.page);
      setSearchTotalPages(result.total_pages);
    } catch {
      if (!controller.signal.aborted) setSearchMoreError(true);
    } finally {
      if (searchMoreController.current === controller) {
        searchMoreController.current = null;
        if (!controller.signal.aborted) setSearchMoreLoading(false);
      }
    }
  };
  const selectSearchProduct = (item: SearchOption) => {
    setSearch(item.title);
    setSearchOpen(false);
    onProduct(item.id);
  };
  const children = open ? categories[open] || [] : [];
  return (
    <header
      className={`masthead ${collapsed ? 'masthead-collapsed' : ''}`}
      ref={root}
      onFocusCapture={() => setHidden(false)}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) {
          close();
          setSearchOpen(false);
        }
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
          href="/"
          aria-label="Flagship home"
          onClick={(event) => {
            if (!event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey) {
              event.preventDefault();
              close();
              setSearchOpen(false);
              onHome();
            }
          }}
        >
          <img src="/icon.png" alt="" width="30" height="30" />
          <span>
            Flagship<b>.</b>
          </span>
        </a>
        <form
          className="masthead-search"
          role="search"
          onSubmit={(event) => {
            event.preventDefault();
            if (activeSuggestion >= 0 && suggestions[activeSuggestion]) {
              selectSearchProduct(suggestions[activeSuggestion]);
            } else {
              void loadMoreSearch();
            }
          }}
        >
          <Search size={18} aria-hidden="true" />
          <input
            value={search}
            onChange={(event) => {
              setSearch(event.target.value);
              setSearchOpen(true);
            }}
            onFocus={() => {
              setOpen(null);
              setSearchOpen(true);
            }}
            onKeyDown={(event) => {
              if (event.key === 'ArrowDown') {
                event.preventDefault();
                setActiveSuggestion((value) => Math.min(suggestions.length - 1, value + 1));
              } else if (event.key === 'ArrowUp') {
                event.preventDefault();
                setActiveSuggestion((value) => Math.max(-1, value - 1));
              } else if (event.key === 'Escape') {
                event.stopPropagation();
                setSearchOpen(false);
              }
            }}
            placeholder="Search products…"
            aria-label="Search the Flagship catalog"
            role="combobox"
            aria-autocomplete="list"
            aria-expanded={searchOpen && search.trim().length >= 2}
            aria-controls="masthead-search-results"
            aria-activedescendant={
              activeSuggestion >= 0 ? `search-result-${activeSuggestion}` : undefined
            }
          />
          {(searchLoading || searchMoreLoading) && (
            <LoaderCircle className="masthead-search-spinner" size={16} aria-label="Searching" />
          )}
          {searchOpen && search.trim().length >= 2 && (
            <div className="masthead-search-panel">
              {suggestions.length > 0 && <p>Products</p>}
              <div
                id="masthead-search-results"
                role="listbox"
                aria-label="Product suggestions"
                aria-busy={searchLoading || searchMoreLoading}
              >
                {suggestions.map((item, index) => (
                  <button
                    type="button"
                    id={`search-result-${index}`}
                    role="option"
                    aria-selected={activeSuggestion === index}
                    className={activeSuggestion === index ? 'active' : ''}
                    key={item.id}
                    onPointerDown={(event) => event.preventDefault()}
                    onClick={() => selectSearchProduct(item)}
                  >
                    <span className="masthead-result-image">
                      {item.image && <img src={item.image} alt="" />}
                    </span>
                    <span className="masthead-result-copy">
                      <strong>{item.title}</strong>
                      <small>
                        {sectionLabel(item.section)} · {item.price.toLocaleString('en-TN')} DT
                      </small>
                    </span>
                    <ArrowUpRight size={15} />
                  </button>
                ))}
              </div>
              {!suggestions.length && searchError ? (
                <div className="masthead-search-empty">
                  <strong>Search is temporarily unavailable</strong>
                  <span>Please try again in a moment.</span>
                </div>
              ) : !suggestions.length && !searchLoading ? (
                <div className="masthead-search-empty">
                  <strong>No products found</strong>
                  <span>Try a model, brand, or component name.</span>
                </div>
              ) : null}
              {searchMoreError && (
                <div className="masthead-search-more-error" role="status">
                  Couldn’t load more products. Try again.
                </div>
              )}
              {searchPage < searchTotalPages && (
                <button
                  type="button"
                  className="masthead-search-all"
                  onPointerDown={(event) => event.preventDefault()}
                  onClick={() => void loadMoreSearch()}
                  disabled={searchMoreLoading}
                >
                  {searchMoreLoading ? (
                    <>
                      Loading more products <LoaderCircle size={15} className="spin" />
                    </>
                  ) : searchMoreError ? (
                    <>
                      Try again <ArrowUpRight size={15} />
                    </>
                  ) : (
                    <>
                      Load more
                      <ArrowUpRight size={15} />
                    </>
                  )}
                </button>
              )}
            </div>
          )}
        </form>
        <div className="masthead-actions">
          {[
            { label: 'Marketplace', icon: Store },
            { label: 'PC builder', icon: Hammer },
          ].map(({ label, icon: Icon }) => (
            <button
              type="button"
              className="masthead-coming-soon"
              key={label}
              disabled
              title={`${label} · Coming soon`}
              aria-label={`${label} · Coming soon`}
            >
              <Icon size={17} />
              <span>{label}</span>
              <small>Soon</small>
            </button>
          ))}
          {[
            { path: '/support', label: 'Support', icon: Sparkles },
            { path: '/wishlist', label: 'Wishlist', icon: Heart },
            { path: '/alerts', label: 'Alerts', icon: Bell },
          ].map(({ path, label, icon: Icon }) => (
            <a
              key={path}
              className={
                path === '/support'
                  ? 'masthead-action-link masthead-support'
                  : 'masthead-action-link'
              }
              href={path}
              title={label}
              aria-label={
                label === 'Wishlist' && user
                  ? `Wishlist (${wishlist.length} saved)`
                  : label === 'Alerts' && user
                    ? `Alerts (${alertCount} active)`
                    : label
              }
              aria-current={locationKey === path ? 'page' : undefined}
              onClick={(event) => {
                if (!event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey) {
                  event.preventDefault();
                  if (!user && (path === '/wishlist' || path === '/alerts')) {
                    onSignIn();
                  } else {
                    onNavigate(path);
                  }
                }
              }}
            >
              <Icon size={17} />
              {path === '/support' && <span>Support</span>}
              {path === '/wishlist' && user && wishlist.length > 0 && (
                <span className="masthead-action-count" aria-hidden="true">
                  {wishlist.length > 99 ? '99+' : wishlist.length}
                </span>
              )}
              {path === '/alerts' && user && alertCount > 0 && (
                <span className="masthead-action-count" aria-hidden="true">
                  {alertCount > 99 ? '99+' : alertCount}
                </span>
              )}
            </a>
          ))}
          <button
            role="switch"
            aria-checked={theme === 'dark'}
            aria-label={theme === 'dark' ? 'Use light mode' : 'Use dark mode'}
            title={theme === 'dark' ? 'Use light mode' : 'Use dark mode'}
            onClick={changeTheme}
          >
            {theme === 'dark' ? <Sun size={17} /> : <Moon size={17} />}
          </button>
          {!authLoading &&
            (user ? (
              <div
                className="account-menu-wrap"
                ref={accountRoot}
                onBlur={(event) => {
                  if (event.relatedTarget && !event.currentTarget.contains(event.relatedTarget))
                    setAccountOpen(false);
                }}
                onKeyDown={(event) => {
                  if (event.key === 'Escape') {
                    event.stopPropagation();
                    setAccountOpen(false);
                    accountTrigger.current?.focus();
                  }
                }}
              >
                <button
                  ref={accountTrigger}
                  className="account-trigger"
                  aria-label="Account menu"
                  aria-expanded={accountOpen}
                  aria-controls="account-popover"
                  onClick={() => setAccountOpen((value) => !value)}
                >
                  <span className="account-avatar">
                    {accountName(user).slice(0, 1).toUpperCase()}
                  </span>
                  <ChevronDown size={12} />
                </button>
                {accountOpen && (
                  <div className="account-menu" id="account-popover" tabIndex={-1}>
                    <div className="account-menu-identity">
                      <span className="account-avatar">
                        {accountName(user).slice(0, 1).toUpperCase()}
                      </span>
                      <div>
                        <strong>{user.given_name ? accountName(user) : 'Your account'}</strong>
                        <small>{user.email}</small>
                      </div>
                    </div>
                    <a
                      href="/settings"
                      onClick={(event) => {
                        event.preventDefault();
                        setAccountOpen(false);
                        onNavigate('/settings');
                      }}
                    >
                      <Settings size={15} />
                      <span>Account settings</span>
                    </a>
                    <div className="account-menu-divider" />
                    <button
                      onClick={() =>
                        void signOut()
                          .then(() => {
                            setAccountOpen(false);
                            onNavigate('/');
                          })
                          .catch(() => setSignOutError('Couldn’t sign out. Please try again.'))
                      }
                    >
                      <LogOut size={15} />
                      Sign out
                    </button>
                    {signOutError && <small role="alert">{signOutError}</small>}
                  </div>
                )}
              </div>
            ) : (
              <a
                className="masthead-signin"
                href="/login"
                onClick={(event) => {
                  event.preventDefault();
                  onSignIn();
                }}
              >
                Sign in
              </a>
            ))}
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
                    'pointerType' in event.nativeEvent && event.nativeEvent.pointerType === 'touch';
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
          <nav className="masthead-page-links" aria-label="Explore Flagship">
            {[
              ['/statistics', 'Statistics'],
              ['/activity', 'Catalog activity'],
              ['/future-plans', 'Future plans'],
            ].map(([path, label]) => (
              <a
                key={path}
                className={
                  path === '/support'
                    ? 'masthead-statistics-link masthead-support'
                    : 'masthead-statistics-link'
                }
                href={path}
                aria-current={locationKey.split('?')[0] === path ? 'page' : undefined}
                onClick={(event) => {
                  if (!event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey) {
                    event.preventDefault();
                    onNavigate(path);
                  }
                }}
              >
                {path === '/support' && <Sparkles size={13} />}
                {label} {path !== '/support' && <ArrowUpRight size={13} />}
              </a>
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
            style={{
              gridTemplateRows: `repeat(${Math.min(children.length, 4)}, minmax(40px, auto))`,
            }}
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
});
export function Footer({ onSection }: { onSection: (section: string) => void }) {
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
          <a href="/statistics">
            Statistics <ArrowUpRight size={13} />
          </a>
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
          <div className="footer-contact">
            <a href="mailto:flagship.tn@gmail.com">flagship.tn@gmail.com</a>
          </div>
        </div>
      </div>
      <div className="footer-bottom">
        <span>© {new Date().getFullYear()} Flagship</span>
        <p>Prices and availability may change. Confirm details with the retailer.</p>
        <a className="footer-legal" href="/legal">
          Terms & privacy
        </a>
        <span>Tunisia / TND</span>
      </div>
    </footer>
  );
}
