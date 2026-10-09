import { useEffect, useRef, useState } from 'react';
import { Flame } from 'lucide-react';
import { accountRequest, useAuth } from '../lib/auth';
import '../information.css';
import { SupportPage } from './SupportPage';
import { LegalPage } from './LegalPage';

interface Feature {
  id: string;
  name: string;
  description: string;
  hype_count: number;
  hyped: boolean;
}

export function InformationPage({ page }: { page: 'support' | 'future-plans' | 'legal' }) {
  if (page === 'legal') return <LegalPage />;
  return page === 'support' ? <SupportPage /> : <FuturePlansPage />;
}

function FuturePlansPage() {
  const { user, loading: authLoading } = useAuth();
  const [features, setFeatures] = useState<Feature[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState<string | null>(null);
  const [voteError, setVoteError] = useState('');
  const [retry, setRetry] = useState(0);
  const requestVersion = useRef(0);
  const voting = useRef(false);

  useEffect(() => {
    document.title = 'Future plans — Flagship';
    const controller = new AbortController();
    requestVersion.current += 1;
    setLoading(true);
    setError('');
    setVoteError('');
    if (authLoading) return () => controller.abort();
    const load = async () => {
      try {
        {
          const data = await accountRequest<{ features: Feature[] }>('/future-plans', {
            signal: controller.signal,
          });
          if (!controller.signal.aborted) setFeatures(data.features);
        }
      } catch (cause) {
        if (!controller.signal.aborted)
          setError(cause instanceof Error ? cause.message : 'Could not load this page.');
      } finally {
        if (!controller.signal.aborted) setLoading(false);
      }
    };
    void load();
    return () => {
      controller.abort();
      requestVersion.current += 1;
    };
  }, [user?.id, authLoading, retry]);

  const hype = async (feature: Feature) => {
    if (voting.current) return;
    voting.current = true;
    const version = requestVersion.current;
    setBusy(feature.id);
    setVoteError('');
    try {
      const result = await accountRequest<{ features: Feature[] }>(
        `/future-plans/${feature.id}/hype`,
        { method: 'POST', body: JSON.stringify({ vote: feature.hyped ? -1 : 1 }) },
      );
      if (version === requestVersion.current) {
        setFeatures(result.features);
      }
    } catch (cause) {
      if (version === requestVersion.current)
        setVoteError(
          cause instanceof Error ? cause.message : 'Could not save your hype. Please try again.',
        );
    } finally {
      voting.current = false;
      setBusy(null);
    }
  };

  return (
    <main className="information-page future-plans-page" id="information-content" tabIndex={-1}>
      <header className="information-intro">
        <h1>Future Plans</h1>
        <p>Hype the features you’d like next.</p>
      </header>
      {loading ? (
        <p role="status">Loading future features…</p>
      ) : error ? (
        <div className="information-card" role="alert">
          <p>{error}</p>
          <button className="secondary-button" onClick={() => setRetry((value) => value + 1)}>
            Try again
          </button>
        </div>
      ) : (
        <>
          <p className="hype-guidance">
            {user
              ? user.email_verified
                ? 'One hype per feature, per account. Click again to remove your hype.'
                : 'Verify your email in account settings to hype a feature.'
              : 'Hype your favorites. Guest votes are saved for this browser; click again to unhype.'}
          </p>
          {voteError && <p role="alert">{voteError}</p>}
          <section className="information-grid feature-grid" aria-label="Planned features">
            {features.map((feature) => (
              <article className="information-card feature-card" key={feature.id}>
                <span className="information-status">On the horizon</span>
                <h2>{feature.name}</h2>
                <p>{feature.description}</p>
                <button
                  className={`hype-button ${feature.hyped ? 'is-hyped' : ''}`}
                  aria-label={`${feature.hyped ? 'Unhype' : 'Hype'} ${feature.name}, ${feature.hype_count} ${feature.hype_count === 1 ? 'vote' : 'votes'}`}
                  aria-pressed={feature.hyped}
                  aria-busy={busy === feature.id}
                  aria-disabled={busy !== null}
                  disabled={!!user && !user.email_verified}
                  onClick={() => void hype(feature)}
                >
                  <Flame size={17} />{' '}
                  {busy === feature.id ? 'Saving…' : feature.hyped ? 'Hyped' : 'Hype'}
                  <span>{feature.hype_count}</span>
                </button>
              </article>
            ))}
          </section>
        </>
      )}
    </main>
  );
}
