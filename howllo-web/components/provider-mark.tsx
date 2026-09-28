type ProviderMarkProps = { provider: string };

/** Decorative mark paired with a visible provider name. */
export function ProviderMark({ provider }: ProviderMarkProps) {
  const key = provider.toLowerCase();
  const is = (name: string) => key === name || key.endsWith(`-${name}`);

  if (is("google")) return <svg className="provider-mark" viewBox="0 0 24 24" aria-hidden="true">
    <path fill="#4285F4" d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z" />
    <path fill="#34A853" d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z" />
    <path fill="#FBBC05" d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z" />
    <path fill="#EA4335" d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z" />
  </svg>;

  if (is("microsoft")) return <svg className="provider-mark" viewBox="0 0 24 24" aria-hidden="true">
    <path fill="#F25022" d="M1 1h10v10H1z" /><path fill="#7FBA00" d="M13 1h10v10H13z" />
    <path fill="#00A4EF" d="M1 13h10v10H1z" /><path fill="#FFB900" d="M13 13h10v10H13z" />
  </svg>;

  if (is("local")) return <svg className="provider-mark" viewBox="0 0 24 24" fill="none" aria-hidden="true">
    <rect x="4" y="10" width="16" height="11" rx="2.5" fill="#e1f5ee" stroke="#208c69" strokeWidth="1.8" />
    <path d="M7.5 10V7a4.5 4.5 0 0 1 9 0v3" stroke="#208c69" strokeWidth="1.8" strokeLinecap="round" />
    <circle cx="12" cy="15.5" r="1.2" fill="#208c69" />
  </svg>;

  // OpenID Connect is a protocol, so this is a generic connection icon.
  return <svg className="provider-mark" viewBox="0 0 24 24" fill="none" aria-hidden="true">
    <path d="M7 7.5 17 12 7 16.5" stroke="#7659ba" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
    <circle cx="5" cy="6.5" r="3" fill="#f7ad58" /><circle cx="19" cy="12" r="3" fill="#7659ba" /><circle cx="5" cy="17.5" r="3" fill="#45b6a4" />
  </svg>;
}
