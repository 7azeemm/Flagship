import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';

export interface AccountUser {
  id: string;
  email: string;
  given_name: string | null;
  family_name: string | null;
  email_verified: boolean;
  has_password: boolean;
  google_connected: boolean;
  created_at: string;
}
export const accountName = (user: AccountUser) =>
  [user.given_name, user.family_name].filter(Boolean).join(' ') || user.email;
export interface WishlistItem {
  target_type: 'variant' | 'product';
  target_id: string;
  created_at: string;
}

const readCookie = (name: string) =>
  document.cookie
    .split(';')
    .map((value) => value.trim())
    .find((value) => value.startsWith(`${name}=`))
    ?.slice(name.length + 1) || '';

export async function accountRequest<T = unknown>(
  path: string,
  init: RequestInit = {},
): Promise<T> {
  const method = (init.method || 'GET').toUpperCase();
  const headers = new Headers(init.headers);
  headers.set('Accept', 'application/json');
  if (init.body) headers.set('Content-Type', 'application/json');
  if (!['GET', 'HEAD', 'OPTIONS'].includes(method)) {
    const csrf = readCookie('flagship_csrf');
    if (csrf) headers.set('X-CSRF-Token', csrf);
  }
  const response = await fetch(`/api${path}`, { ...init, headers, credentials: 'same-origin' });
  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as {
      error?: { message?: string };
    } | null;
    throw new Error(body?.error?.message || 'The request could not be completed.');
  }
  if (
    !['GET', 'HEAD', 'OPTIONS'].includes(method) &&
    /^\/alerts(?:\/[^/]+)?$/.test(path) &&
    path !== '/alerts/preview'
  ) {
    window.dispatchEvent(new Event('flagship:alerts-changed'));
  }
  return response.status === 204 ? (undefined as T) : (response.json() as Promise<T>);
}

interface AuthValue {
  user: AccountUser | null;
  loading: boolean;
  wishlist: WishlistItem[];
  refreshSession: () => Promise<void>;
  signOut: () => Promise<void>;
  isSaved: (type: 'variant' | 'product', id: string) => boolean;
  toggleSaved: (type: 'variant' | 'product', id: string) => Promise<void>;
}
const AuthContext = createContext<AuthValue>({
  user: null,
  loading: false,
  wishlist: [],
  refreshSession: async () => {},
  signOut: async () => {},
  isSaved: () => false,
  toggleSaved: async () => {
    throw new Error('Please sign in to save products.');
  },
});

async function loadAuthState() {
  const result = await accountRequest<{ user: AccountUser | null }>('/auth/session');
  const wishlist = result.user
    ? (await accountRequest<{ items: WishlistItem[] }>('/wishlist').catch(() => ({ items: [] })))
        .items
    : [];
  return { user: result.user, wishlist };
}
let initialAuthRequest: ReturnType<typeof loadAuthState> | null = null;

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<AccountUser | null>(null);
  const [loading, setLoading] = useState(true);
  const [wishlist, setWishlist] = useState<WishlistItem[]>([]);
  const revision = useRef(0);
  const owner = useRef<string | null>(null);
  const mutations = useRef(new Set<string>());
  owner.current = user?.id ?? null;
  const refreshSession = useCallback(async () => {
    const current = ++revision.current;
    try {
      const result = await loadAuthState();
      if (current !== revision.current) return;
      setUser(result.user);
      setWishlist(result.wishlist);
    } catch {
      // A transient request failure does not establish that the session ended.
    } finally {
      if (current === revision.current) setLoading(false);
    }
  }, []);
  useEffect(() => {
    let active = true;
    const current = ++revision.current;
    initialAuthRequest ??= loadAuthState().finally(() => {
      initialAuthRequest = null;
    });
    initialAuthRequest
      .then((result) => {
        if (!active || current !== revision.current) return;
        setUser(result.user);
        setWishlist(result.wishlist);
      })
      .catch(() => {
        if (!active || current !== revision.current) return;
        setUser(null);
        setWishlist([]);
      })
      .finally(() => {
        if (active && current === revision.current) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, []);
  const signOut = useCallback(async () => {
    ++revision.current;
    await accountRequest('/auth/logout', { method: 'POST' });
    ++revision.current;
    owner.current = null;
    setUser(null);
    setWishlist([]);
  }, []);
  const isSaved = useCallback(
    (target_type: 'variant' | 'product', target_id: string) =>
      wishlist.some((item) => item.target_type === target_type && item.target_id === target_id),
    [wishlist],
  );
  const toggleSaved = useCallback(
    async (target_type: 'variant' | 'product', target_id: string) => {
      if (!user) throw new Error('Please sign in to save products.');
      const key = `${user.id}:${target_type}:${target_id}`;
      if (mutations.current.has(key)) return;
      mutations.current.add(key);
      try {
        const existing = wishlist.find(
          (item) => item.target_type === target_type && item.target_id === target_id,
        );
        if (existing) {
          setWishlist((items) => items.filter((item) => item !== existing));
          try {
            await accountRequest(`/wishlist/${target_type}/${encodeURIComponent(target_id)}`, {
              method: 'DELETE',
            });
          } catch (error) {
            if (owner.current === user.id) setWishlist((items) => [existing, ...items]);
            throw error;
          }
        } else {
          const optimistic = { target_type, target_id, created_at: new Date().toISOString() };
          setWishlist((items) => [optimistic, ...items]);
          try {
            await accountRequest('/wishlist', {
              method: 'POST',
              body: JSON.stringify({ target_type, target_id }),
            });
          } catch (error) {
            if (owner.current === user.id)
              setWishlist((items) => items.filter((item) => item !== optimistic));
            throw error;
          }
        }
      } finally {
        mutations.current.delete(key);
      }
    },
    [user, wishlist],
  );
  const value = useMemo(
    () => ({ user, loading, wishlist, refreshSession, signOut, isSaved, toggleSaved }),
    [user, loading, wishlist, refreshSession, signOut, isSaved, toggleSaved],
  );
  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}
export function useAuth() {
  return useContext(AuthContext);
}
