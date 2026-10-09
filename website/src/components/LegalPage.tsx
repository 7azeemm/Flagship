import '../information.css';

export function LegalPage() {
  return (
    <main className="information-page legal-page" id="information-content" tabIndex={-1}>
      <header className="information-intro">
        <h1>Terms & privacy</h1>
        <p>A few simple things to know when using Flagship.</p>
        <p className="legal-updated">Last updated: September 30, 2026</p>
      </header>

      <section aria-labelledby="legal-terms">
        <h2 id="legal-terms">Terms of service</h2>
        <p>
          By using Flagship, you agree to these terms. Flagship helps you discover and compare tech
          products from Tunisian retailers. All features are free, with no paid plans or
          subscriptions. We do not sell the products or handle your purchases.
        </p>
        <p>
          Use the site lawfully, keep your account secure, and do not misuse the service, disrupt
          it, or try to access someone else’s data. Features may change or be unavailable from time
          to time. Retailer names, logos, and product content belong to their respective owners.
        </p>
      </section>

      <section aria-labelledby="legal-disclaimer">
        <h2 id="legal-disclaimer">Disclaimer</h2>
        <p>
          Prices, availability, specifications, and price history come from retailer listings and
          recorded observations. They may be incorrect, incomplete, or out of date. We provide this
          information as it is, without guaranteeing its accuracy or that an offer is still
          available.
        </p>
        <p>
          Before purchasing, recheck the product information, final price, stock, delivery costs,
          and warranty directly with the retailer. Your purchase is with that retailer, whose terms
          and policies apply. To the extent permitted by law, Flagship is not responsible for
          incorrect data or prices, retailer transactions, or losses from relying on the site. This
          does not limit rights or responsibilities that cannot be excluded by law.
        </p>
      </section>

      <section aria-labelledby="legal-privacy">
        <h2 id="legal-privacy">Privacy policy</h2>
        <p>
          You can browse without an account. If you create one, we store your email, profile
          details, sign-in information, and the wishlists, alerts, and feature votes you choose to
          save. Passwords are stored as hashes. We use this data to run your account and send
          account emails and alerts you request. We also process technical details, such as your IP
          address and browser information, to keep the service secure.
        </p>
        <p>
          Cookies support sign-in and remember feature votes; local storage remembers your theme. We
          do not sell your personal data. Services used for hosting, email delivery, and optional
          Google sign-in process data needed for those functions. External fonts and retailer images
          may also share technical details with their providers. Retailer sites have their own
          privacy policies.
        </p>
        <p>
          We keep account data while your account is active and technical data as needed for
          security and operation. You can update your profile, remove saved items and alerts, or
          delete your account in Settings. For questions or requests about accessing, correcting, or
          deleting your data, use the contact details in the footer.
        </p>
      </section>

      <p className="legal-changes">
        We may update this page as Flagship evolves. Changes will appear here with a new update date.
      </p>
    </main>
  );
}
