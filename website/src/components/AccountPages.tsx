import { useEffect, useState, type FormEvent } from 'react';
import {
  ArrowLeft,
  ArrowRight,
  Check,
  Eye,
  EyeOff,
  Heart,
  LoaderCircle,
  LockKeyhole,
  Mail,
  ShieldCheck,
  Trash2,
  UserRound,
} from 'lucide-react';
import {
  accountName,
  accountRequest,
  useAuth,
  type AccountUser,
  type WishlistItem,
} from '../lib/auth';
import { ProductCard } from './ProductCard';
import { parseProduct } from '../lib/api';
import type { Product, VariantSummary } from '../types';
import { Dialog } from './Dialog';
import '../accounts.css';

const destination = () => {
  const candidate = new URLSearchParams(location.search).get('returnTo');
  return candidate?.startsWith('/') &&
    !candidate.startsWith('//') &&
    !candidate.includes('\\') &&
    !/[\r\n]/.test(candidate)
    ? candidate
    : '/';
};
const navigate = (path: string) => {
  history.pushState(
    /^\/(product|variant)\//.test(path)
      ? { detailBackground: window.location.pathname + window.location.search }
      : null,
    '',
    path,
  );
  window.dispatchEvent(new PopStateEvent('popstate'));
};
const errorMessage = (error: unknown) =>
  error instanceof Error ? error.message : 'Something went wrong.';

export function AuthPage({
  mode,
  modal = false,
  onClose,
}: {
  mode: 'login' | 'register' | 'forgot' | 'reset';
  modal?: boolean;
  onClose?: () => void;
}) {
  const { user, refreshSession } = useAuth();
  const [modalMode, setModalMode] = useState(mode);
  const activeMode = modalMode;
  useEffect(() => setModalMode(mode), [mode]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(() => {
    const code = new URLSearchParams(location.search).get('error');
    return code === 'google_link_required'
      ? 'For this Google email, sign up or sign in with email and password first, then connect Google in Account settings.'
      : code === 'google_cancelled'
        ? 'Google sign-in was cancelled. You can try again.'
        : code === 'google_failed'
          ? 'Google sign-in could not be completed. Start again from this window.'
          : '';
  });
  const [message, setMessage] = useState(() =>
    new URLSearchParams(location.search).get('verified') === '1'
      ? 'Email verified. Your account is ready. Sign in to continue.'
      : '',
  );
  const [pendingEmail, setPendingEmail] = useState('');
  const [signupReceipt, setSignupReceipt] = useState<string | undefined>();
  const [showPassword, setShowPassword] = useState(false);
  const [resendAt, setResendAt] = useState(0);
  useEffect(() => {
    if (!modal)
      document.title = `${activeMode === 'register' ? 'Create account' : activeMode === 'login' ? 'Sign in' : 'Password recovery'} — Flagship`;
  }, [activeMode, modal]);
  useEffect(() => {
    if (user && !busy && (activeMode === 'login' || activeMode === 'register')) {
      if (!user.email_verified) {
        setPendingEmail(user.email);
        return;
      }
      if (modal) onClose?.();
      else navigate(destination());
    }
  }, [user, activeMode, modal, onClose, busy]);
  useEffect(() => {
    if (!pendingEmail) return;
    const check = () => {
      void refreshSession();
    };
    window.addEventListener('focus', check);
    return () => window.removeEventListener('focus', check);
  }, [pendingEmail, refreshSession]);
  const changeMode = (next: typeof mode) => {
    setError('');
    setMessage('');
    setModalMode(next);
    setShowPassword(false);
  };
  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setBusy(true);
    setError('');
    setMessage('');
    const data = new FormData(event.currentTarget);
    try {
      if (activeMode === 'login' || activeMode === 'register') {
        const result = await accountRequest<{
          user?: AccountUser;
          email?: string;
          email_sent?: boolean;
          receipt?: string;
        }>(`/auth/${activeMode}`, {
          method: 'POST',
          body: JSON.stringify({ email: data.get('email'), password: data.get('password') }),
        });
        if (activeMode === 'register') {
          setSignupReceipt(result.receipt);
          setPendingEmail(result.email || String(data.get('email')));
          if (result.email_sent === false)
            setError('Your verification email could not be sent. Please try sending it again.');
          return;
        }
        await refreshSession();
        if (modal) onClose?.();
        else navigate(destination());
      } else if (activeMode === 'forgot') {
        const result = await accountRequest<{ message: string }>('/auth/password/forgot', {
          method: 'POST',
          body: JSON.stringify({ email: data.get('email') }),
        });
        setMessage(result.message);
      } else {
        await accountRequest('/auth/password/reset', {
          method: 'POST',
          body: JSON.stringify({ token: data.get('token'), password: data.get('password') }),
        });
        await refreshSession();
        setModalMode('login');
        setMessage('Password updated. Sign in with your new password.');
      }
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  };
  const title = pendingEmail
    ? 'Check your inbox'
    : activeMode === 'login'
      ? 'Welcome back'
      : activeMode === 'register'
        ? 'Create your account'
        : activeMode === 'forgot'
          ? 'Reset your password'
          : 'Choose a new password';
  const authPath = ['/login', '/register', '/forgot-password', '/reset-password'].includes(
    location.pathname,
  );
  const returnTarget = modal && !authPath ? location.pathname + location.search : destination();
  const resend = async () => {
    setBusy(true);
    setError('');
    setMessage('');
    try {
      const result = await accountRequest<{ email_sent?: boolean }>('/auth/email/resend', {
        method: 'POST',
        body: JSON.stringify({ receipt: signupReceipt }),
      });
      if (result.email_sent === false)
        throw new Error('We couldn’t send the email. Please try again shortly.');
      setResendAt(Date.now() + 60_000);
      setMessage('A fresh verification link is on its way. Check your spam folder too.');
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  };
  const card = (
    <section className={`account-card ${modal ? 'account-modal-card' : ''}`}>
      <div className="auth-brand">
        <img src="/icon.png" alt="" width="24" height="24" />
        flagship<span>.</span>
      </div>
      {pendingEmail && (
        <span className="account-icon">
          <Mail size={24} />
        </span>
      )}
      <h1>{title}</h1>
      <p>
        {pendingEmail ? (
          <>
            Verify <strong>{pendingEmail}</strong> to finish creating your account.
          </>
        ) : activeMode === 'register' ? (
          'A little home for your next great find.'
        ) : activeMode === 'login' ? (
          'Your saved finds, right where you left them.'
        ) : activeMode === 'forgot' ? (
          'We’ll email you a link to get back into your account.'
        ) : (
          'Choose a strong password you haven’t used before.'
        )}
      </p>
      {pendingEmail ? (
        <div className="verification-step">
          <div className="verification-note">
            <ShieldCheck size={18} />
            <span>
              Open the link in your email to verify your account, then sign in. Signup expires after
              24 hours.
            </span>
          </div>
          {error && (
            <div className="account-error" role="alert">
              {error}
            </div>
          )}
          {message && (
            <div className="account-success" role="status">
              {message}
            </div>
          )}
          <button
            className="primary-button"
            disabled={busy}
            onClick={() => {
              setPendingEmail('');
              changeMode('login');
            }}
          >
            <Check size={16} /> I’ve verified my email — sign in
          </button>
          <button
            className="auth-text-button"
            disabled={busy}
            onClick={() => {
              if (Date.now() < resendAt) {
                setMessage('Please wait a minute before requesting another email.');
                return;
              }
              void resend();
            }}
          >
            Didn’t receive it? Send again
          </button>
        </div>
      ) : (
        <>
          {(activeMode === 'login' || activeMode === 'register') && (
            <>
              <a
                className="google-button"
                href={`/api/auth/google/start?return_to=${encodeURIComponent(returnTarget)}`}
              >
                <GoogleMark />
                Continue with Google
              </a>
              <div className="auth-divider">
                <span>or continue with email</span>
              </div>
            </>
          )}
          <form onSubmit={submit}>
            {(activeMode === 'login' || activeMode === 'register' || activeMode === 'forgot') && (
              <label>
                Email address{' '}
                <input
                  name="email"
                  type="email"
                  placeholder="you@example.com"
                  autoComplete="email"
                  required
                />
              </label>
            )}
            {(activeMode === 'login' || activeMode === 'register' || activeMode === 'reset') && (
              <div className="auth-password-group">
                <span className="auth-label-row">
                  <label htmlFor="auth-password">
                    {activeMode === 'reset' ? 'New password' : 'Password'}
                  </label>
                  {activeMode === 'login' && (
                    <button type="button" onClick={() => changeMode('forgot')}>
                      Forgot password?
                    </button>
                  )}
                </span>
                <span className="password-field">
                  <input
                    id="auth-password"
                    name="password"
                    type={showPassword ? 'text' : 'password'}
                    minLength={activeMode === 'login' ? undefined : 8}
                    maxLength={128}
                    autoComplete={activeMode === 'login' ? 'current-password' : 'new-password'}
                    required
                  />
                  <button
                    type="button"
                    aria-label={showPassword ? 'Hide password' : 'Show password'}
                    onClick={() => setShowPassword((value) => !value)}
                  >
                    {showPassword ? <EyeOff size={17} /> : <Eye size={17} />}
                  </button>
                </span>
                {activeMode !== 'login' && <small>At least 8 characters.</small>}
              </div>
            )}
            {activeMode === 'reset' && (
              <input
                type="hidden"
                name="token"
                defaultValue={new URLSearchParams(location.search).get('token') || ''}
                required
              />
            )}
            {error && (
              <div className="account-error" role="alert">
                {error}
              </div>
            )}
            {message && (
              <div className="account-success" role="status">
                {message}
              </div>
            )}
            <button className="primary-button" disabled={busy}>
              {busy && <LoaderCircle className="spin" size={16} />}
              {activeMode === 'login'
                ? 'Sign in'
                : activeMode === 'register'
                  ? 'Create account & verify email'
                  : activeMode === 'forgot'
                    ? 'Send reset link'
                    : 'Reset password'}
            </button>
          </form>
          <div className="account-links">
            {activeMode === 'login' && (
              <>
                {modal ? (
                  <button type="button" onClick={() => changeMode('register')}>
                    Create an account
                  </button>
                ) : (
                  <a href="/register">Create an account</a>
                )}
              </>
            )}
            {activeMode === 'register' &&
              (modal ? (
                <button type="button" onClick={() => changeMode('login')}>
                  Already have an account? Sign in
                </button>
              ) : (
                <a href="/login">Already have an account?</a>
              ))}
            {(activeMode === 'forgot' || activeMode === 'reset') &&
              (modal ? (
                <button type="button" onClick={() => changeMode('login')}>
                  <ArrowLeft size={14} /> Back to sign in
                </button>
              ) : (
                <a href="/login">Back to sign in</a>
              ))}
          </div>
        </>
      )}
    </section>
  );
  return modal ? (
    <Dialog title={title} className="account-dialog" onClose={() => onClose?.()}>
      {card}
    </Dialog>
  ) : (
    <main className="account-page">{card}</main>
  );
}

function GoogleMark() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" aria-hidden="true">
      <path
        fill="#4285F4"
        d="M21.6 12.23c0-.71-.06-1.39-.18-2.05H12v3.88h5.38a4.6 4.6 0 0 1-2 3.02v2.51h3.24c1.9-1.75 2.98-4.33 2.98-7.36Z"
      />
      <path
        fill="#34A853"
        d="M12 22c2.7 0 4.96-.9 6.62-2.41l-3.24-2.51c-.9.6-2.04.97-3.38.97-2.6 0-4.8-1.76-5.59-4.12H3.07v2.59A10 10 0 0 0 12 22Z"
      />
      <path
        fill="#FBBC05"
        d="M6.41 13.93a6 6 0 0 1 0-3.86V7.48H3.07a10 10 0 0 0 0 9.04l3.34-2.59Z"
      />
      <path
        fill="#EA4335"
        d="M12 5.95c1.47 0 2.79.51 3.82 1.51l2.87-2.87A9.6 9.6 0 0 0 12 2a10 10 0 0 0-8.93 5.48l3.34 2.59C7.2 7.71 9.4 5.95 12 5.95Z"
      />
    </svg>
  );
}

export function WishlistPage() {
  const { user, loading, wishlist, toggleSaved } = useAuth();
  useEffect(() => {
    document.title = 'Wishlist — Flagship';
  }, []);
  useEffect(() => {
    if (!loading && !user) window.dispatchEvent(new Event('flagship:signin'));
  }, [loading, user]);
  if (!loading && !user) {
    return (
      <main className="account-page" id="account-content" tabIndex={-1}>
        <button
          className="primary-button"
          onClick={() => window.dispatchEvent(new Event('flagship:signin'))}
        >
          Sign in to view your wishlist
        </button>
      </main>
    );
  }
  return (
    <main className="account-page wide wishlist-page" id="account-content" tabIndex={-1}>
      <section className="account-card wishlist-surface">
        <span className="account-icon">
          <Heart size={24} />
        </span>
        <h1>Your wishlist</h1>
        <p>
          Your next upgrade starts here. {wishlist.length} saved{' '}
          {wishlist.length === 1 ? 'find' : 'finds'}.
        </p>
        {!wishlist.length ? (
          <div className="account-empty">
            <Heart size={28} />
            <h2>A place for your favorites</h2>
            <p>Tap a heart on any product to keep it here.</p>
            <a className="primary-button" href="/">
              Explore products <ArrowRight size={16} />
            </a>
          </div>
        ) : (
          <ul className="wishlist-grid">
            {wishlist.map((item) => (
              <WishlistEntry
                key={`${item.target_type}:${item.target_id}`}
                item={item}
                remove={() => toggleSaved(item.target_type, item.target_id)}
              />
            ))}
          </ul>
        )}
      </section>
    </main>
  );
}

function WishlistEntry({ item, remove }: { item: WishlistItem; remove: () => Promise<void> }) {
  const [product, setProduct] = useState<Product | null>(null);
  const [variant, setVariant] = useState<VariantSummary>();
  const [failed, setFailed] = useState(false);
  const [retry, setRetry] = useState(0);
  const [removeError, setRemoveError] = useState('');
  useEffect(() => {
    const controller = new AbortController();
    setFailed(false);
    fetch(`/api/wishlist/${item.target_type}/${encodeURIComponent(item.target_id)}/details`, {
      signal: controller.signal,
      headers: { Accept: 'application/json' },
      credentials: 'same-origin',
    })
      .then(async (response) => {
        if (!response.ok) throw new Error();
        const value = (await response.json()) as Record<string, unknown>;
        if (item.target_type === 'product') {
          setProduct(parseProduct(value.product));
        } else {
          const products = Array.isArray(value.products) ? value.products.map(parseProduct) : [];
          if (!products.length) throw new Error();
          const details = value.variant as Record<string, unknown>;
          const selected =
            products.find((p) => p.id === details.representative_product_id) || products[0];
          const price =
            typeof details.lowest_price === 'number' ? details.lowest_price : selected.price;
          setProduct({ ...selected, price });
          setVariant({
            id: item.target_id,
            title: selected.title,
            representative_product_id: selected.id,
            section: selected.section,
            image: selected.image,
            images: selected.images,
            price,
            original_price: null,
            components: selected.components,
            offer_count: Number(details.offer_count) || products.length,
            retailer_count: Number(details.retailer_count) || 1,
            in_stock_count: Number(details.in_stock_count) || 0,
            views: Number(details.views) || 0,
          });
        }
      })
      .catch(() => {
        if (!controller.signal.aborted) setFailed(true);
      });
    return () => controller.abort();
  }, [item.target_id, item.target_type, retry]);
  return (
    <li className="wishlist-entry">
      {failed ? (
        <div className="wishlist-unavailable">
          <Heart size={24} />
          <h2>This find is unavailable</h2>
          <p>It’s still saved. Try loading it again later.</p>
          <button className="secondary-button" onClick={() => setRetry((value) => value + 1)}>
            Try again
          </button>
          <button
            className="auth-text-button"
            onClick={() =>
              void remove().catch(() => setRemoveError('Couldn’t remove this item. Try again.'))
            }
          >
            Remove from wishlist
          </button>
          {removeError && <p role="alert">{removeError}</p>}
        </div>
      ) : product ? (
        <ProductCard
          product={product}
          variant={variant}
          fields={Object.keys(product.components).slice(0, 4)}
          onOpen={() =>
            navigate(
              variant
                ? `/variant/${encodeURIComponent(variant.id)}?${new URLSearchParams({ product: product.id })}`
                : `/product/${encodeURIComponent(product.id)}`,
            )
          }
        />
      ) : (
        <div className="wishlist-unavailable" role="status">
          <LoaderCircle className="spin" size={22} />
          Loading your saved find…
        </div>
      )}
      <small className="wishlist-saved-date">
        Saved {new Date(item.created_at).toLocaleDateString()}
      </small>
    </li>
  );
}

let verificationRequest: { token: string; promise: Promise<unknown> } | null = null;

export function VerifyEmailPage() {
  const { refreshSession } = useAuth();
  const [status, setStatus] = useState('Verifying your email…');
  const [failed, setFailed] = useState(false);
  const [token] = useState(() => new URLSearchParams(location.search).get('token'));
  useEffect(() => {
    document.title = 'Email verification — Flagship';
    if (!token) {
      setFailed(true);
      setStatus('This verification link is incomplete.');
      return;
    }
    let active = true;
    // Share the request across StrictMode effects so a one-use link is consumed once.
    if (verificationRequest?.token !== token) {
      verificationRequest = {
        token,
        promise: accountRequest('/auth/email/verify', {
          method: 'POST',
          body: JSON.stringify({ token }),
        }),
      };
    }
    void verificationRequest.promise
      .then(async () => {
        if (!active) return;
        history.replaceState(null, '', '/verify-email');
        await refreshSession();
        if (active) navigate('/login?verified=1');
      })
      .catch((error) => {
        if (!active) return;
        verificationRequest = null;
        setFailed(true);
        setStatus(errorMessage(error));
      });
    return () => {
      active = false;
    };
  }, [token, refreshSession]);
  return (
    <main className="account-page" id="account-content" tabIndex={-1}>
      <section className="account-card">
        <div className="auth-brand">
          <img src="/icon.png" alt="" width="24" height="24" />
          flagship<span>.</span>
        </div>
        <span className="account-icon">
          {!failed ? <LoaderCircle size={24} className="spin" /> : <ShieldCheck size={24} />}
        </span>
        <h1>{failed ? 'Could not verify your email' : 'One moment…'}</h1>
        <p role="status">{status}</p>
        {failed && (
          <a className="primary-button" href="/login">
            Back to your account <ArrowRight size={16} />
          </a>
        )}
      </section>
    </main>
  );
}

export function SettingsPage() {
  const { user, loading, refreshSession, signOut } = useAuth();
  const [status, setStatus] = useState('');
  const [error, setError] = useState('');
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    document.title = 'Settings — Flagship';
  }, []);
  useEffect(() => {
    if (!loading && !user) navigate(`/login?returnTo=${encodeURIComponent('/settings')}`);
  }, [loading, user]);
  if (!loading && !user) {
    return null;
  }
  if (!user)
    return (
      <main className="account-page" id="account-content" tabIndex={-1}>
        <LoaderCircle className="spin" />
      </main>
    );
  const action = async (work: () => Promise<unknown>, message: string) => {
    if (saving) return;
    setSaving(true);
    setError('');
    setStatus('');
    try {
      await work();
      setStatus(message);
      await refreshSession();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setSaving(false);
    }
  };
  return (
    <main className="account-page wide settings-page" id="account-content" tabIndex={-1}>
      <section className="account-card settings-card">
        <header className="settings-heading">
          <a href="/" className="settings-back">
            <ArrowLeft size={14} /> Back to exploring
          </a>
          <h1>Your account</h1>
          <p>A few details. A little peace of mind.</p>
        </header>
        <div className="settings-identity">
          <span className="account-avatar large">
            {accountName(user).slice(0, 1).toUpperCase()}
          </span>
          <div>
            <strong>{user.given_name ? accountName(user) : 'Your Flagship account'}</strong>
            <span>{user.email}</span>
          </div>
          <span className={`account-status ${user.email_verified ? 'verified' : ''}`}>
            <ShieldCheck size={14} />
            {user.email_verified ? 'Verified' : 'Verification needed'}
          </span>
        </div>
        <div className="settings-feedback" aria-live="polite">
          {error && (
            <div className="account-error" role="alert">
              {error}
            </div>
          )}
          {status && <div className="account-success">{status}</div>}
        </div>
        <fieldset className="settings-grid" disabled={saving}>
          <section className="settings-details">
            <div className="settings-section-title">
              <UserRound size={18} />
              <div>
                <h2>Account details</h2>
                <p>Your sign-in information, in one place.</p>
              </div>
            </div>
            <dl className="account-details-list">
              <div>
                <dt>Email address</dt>
                <dd>{user.email}</dd>
              </div>
              {(user.given_name || user.family_name) && (
                <div>
                  <dt>Name from Google</dt>
                  <dd>{accountName(user)}</dd>
                </div>
              )}
            </dl>
            <p className="verification">
              <ShieldCheck size={16} />
              {user.email_verified ? 'Email verified' : 'Email not verified'}
            </p>
            {!user.email_verified && (
              <button
                type="button"
                className="secondary-button"
                onClick={() =>
                  void action(async () => {
                    const result = await accountRequest<{ email_sent?: boolean }>(
                      '/auth/email/resend',
                      { method: 'POST' },
                    );
                    if (result.email_sent === false)
                      throw new Error(
                        'We couldn’t send the verification email. Please try again shortly.',
                      );
                  }, 'A new verification link was requested.')
                }
              >
                Resend verification
              </button>
            )}
          </section>
          <form
            onSubmit={(event) => {
              event.preventDefault();
              const data = new FormData(event.currentTarget);
              void action(
                () =>
                  accountRequest('/auth/password/change', {
                    method: 'POST',
                    body: JSON.stringify({
                      current_password: data.get('current_password') || null,
                      new_password: data.get('new_password'),
                    }),
                  }),
                'Password updated and other sessions signed out.',
              );
            }}
          >
            <div className="settings-section-title">
              <LockKeyhole size={18} />
              <div>
                <h2>{user.has_password ? 'Password & security' : 'Set a password'}</h2>
                <p>Keep your account yours.</p>
              </div>
            </div>
            {user.has_password && (
              <label>
                Current password
                <input
                  name="current_password"
                  type="password"
                  autoComplete="current-password"
                  required
                />
              </label>
            )}
            <label>
              New password
              <input
                name="new_password"
                type="password"
                minLength={8}
                maxLength={128}
                autoComplete="new-password"
                required
              />
            </label>
            <small className="settings-hint">
              At least 8 characters. Updating your password signs out your other devices.
            </small>
            <button className="secondary-button">Update password</button>
          </form>
          <section>
            <div className="settings-section-title">
              <GoogleMark />
              <div>
                <h2>Google sign-in</h2>
                <p>
                  {user.google_connected
                    ? 'Connected. One less password to remember.'
                    : 'A quicker way to get back to your finds.'}
                </p>
              </div>
            </div>
            {!user.google_connected && (
              <button
                className="secondary-button"
                onClick={() =>
                  void action(async () => {
                    const result = await accountRequest<{ url: string }>('/auth/google/link', {
                      method: 'POST',
                    });
                    location.assign(result.url);
                  }, '')
                }
              >
                Connect Google
              </button>
            )}
          </section>
          <section className="sessions-section">
            <div className="settings-section-title">
              <ShieldCheck size={18} />
              <div>
                <h2>Your sessions</h2>
                <p>Signed in on a shared device? Sign out everywhere.</p>
              </div>
            </div>
            <button
              className="secondary-button"
              onClick={() =>
                void action(async () => {
                  await accountRequest('/auth/logout-all', { method: 'POST' });
                  await signOut();
                  navigate('/');
                }, '')
              }
            >
              Sign out of all devices
            </button>
          </section>
          <section className="danger-zone">
            <div>
              <h2>Delete account</h2>
              <p>Permanently remove your account and saved products.</p>
            </div>
            <DeleteAccount user={user} />
          </section>
        </fieldset>
      </section>
    </main>
  );
}

function DeleteAccount({ user }: { user: AccountUser }) {
  const [open, setOpen] = useState(false);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  return (
    <>
      <button
        type="button"
        className="danger-button"
        onClick={() => {
          setError('');
          setOpen(true);
        }}
      >
        <Trash2 size={16} />
        Delete account
      </button>
      {open && (
        <Dialog
          title="Confirm account deletion"
          className="delete-account-dialog"
          onClose={() => {
            if (!busy) setOpen(false);
          }}
        >
          <section className="account-card delete-account-card">
            <span className="account-icon danger-account-icon">
              <Trash2 size={22} />
            </span>
            <h1>Delete your account?</h1>
            <p>
              This permanently removes your account and saved products. This action cannot be undone.
            </p>
            <form
              onSubmit={async (event) => {
                event.preventDefault();
                if (busy) return;
                const data = new FormData(event.currentTarget);
                setBusy(true);
                setError('');
                try {
                  await accountRequest('/account', {
                    method: 'DELETE',
                    body: JSON.stringify({
                      password: data.get('password') || null,
                    }),
                  });
                  location.assign('/');
                } catch (cause) {
                  setError(errorMessage(cause));
                  setBusy(false);
                }
              }}
            >
              {user.has_password && (
                <label>
                  Confirm with your password
                  <input
                    name="password"
                    type="password"
                    autoComplete="current-password"
                    required
                    disabled={busy}
                  />
                </label>
              )}
              <label className="delete-account-confirm">
                <input type="checkbox" name="confirm" required disabled={busy} />
                I understand this permanently deletes my account.
              </label>
              {error && (
                <div className="account-error" role="alert">
                  {error}
                </div>
              )}
              <div className="delete-account-actions">
                <button
                  type="button"
                  className="secondary-button"
                  disabled={busy}
                  onClick={() => setOpen(false)}
                >
                  Keep my account
                </button>
                <button type="submit" className="danger-button" disabled={busy}>
                  {busy ? <LoaderCircle className="spin" size={16} /> : <Trash2 size={16} />}
                  Permanently delete
                </button>
              </div>
            </form>
          </section>
        </Dialog>
      )}
    </>
  );
}
