import { useEffect } from 'react';
import { ArrowUpRight, Coffee, Heart } from 'lucide-react';
import '../information.css';

export function SupportPage() {
  useEffect(() => {
    document.title = 'Support — Flagship';
  }, []);

  return (
    <main className="information-page support-page" id="information-content" tabIndex={-1}>
      <div className="support-layout">
        <section className="support-story" aria-labelledby="creator-heading">
          <span className="support-note-label">A NOTE FROM THE CREATOR</span>
          <h2 id="creator-heading">
            Built between classes.
            <br />
            Made with a lot of care.
          </h2>
          <p>
            I’m doing my Bac this year, and I’m building Flagship alongside my studies to make
            comparing tech prices in Tunisia easier.
          </p>
          <p>
            It takes time and care to keep it running. If Flagship has helped you find a better price,
            I’d really appreciate any support you can give. It helps cover running costs and gives
            me more room to keep building.
          </p>
          <p>
            Buying a coffee, sharing the site with a friend, or simply using it—it all means
            something. Thank you for being here.
          </p>
          <div className="support-signoff">
            <span className="support-signoff-icon">
              <Heart size={17} />
            </span>
            <div>
              <strong>The person behind Flagship</strong>
              <span>Student. Builder. Grateful you’re here.</span>
            </div>
          </div>
        </section>
        <aside className="support-aside" aria-label="Ways to support Flagship">
          <section className="support-coffee-card">
            <div className="support-coffee-art" aria-hidden="true">
              <Coffee size={52} strokeWidth={1.3} />
              <span className="support-coffee-heart">
                <Heart size={15} fill="currentColor" />
              </span>
            </div>
            <span className="support-note-label">A SMALL GESTURE GOES A LONG WAY</span>
            <h2>Fuel the next little improvement.</h2>
            <p>If you’d like to help, a coffee is a lovely place to start.</p>
            <a
              className="support-coffee"
              href="https://buymeacoffee.com/flagship.tn"
              target="_blank"
              rel="noopener noreferrer"
            >
              <Coffee size={18} /> Buy me a coffee <ArrowUpRight size={17} />
            </a>
            <small>Always optional. Always appreciated.</small>
          </section>
        </aside>
      </div>
      <footer className="support-footer">
        <Heart size={14} />
        <p>Flagship stays 100% free. Your support is a gift, never a requirement.</p>
      </footer>
    </main>
  );
}
