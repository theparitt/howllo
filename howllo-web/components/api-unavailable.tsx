type ApiUnavailableProps = {
  message?: string;
};

export function ApiUnavailable({ message }: ApiUnavailableProps) {
  return (
    <section className="panel empty-state">
      <h1 className="empty-state__title">Howllo server is unavailable.</h1>
      <p className="empty-state__copy">
        Start the backend on <code className="code-hint">127.0.0.1:5110</code> or set
        <code className="code-hint"> NEXT_PUBLIC_API_BASE_URL</code> to a reachable server.
      </p>
      {message ? <p className="section-subtitle">{message}</p> : null}
    </section>
  );
}
