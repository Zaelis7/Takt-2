import type { components } from "./generated/openapi";

type Health = components["schemas"]["Health"];

const contractExample = { status: "ok" } satisfies Health;

export function App() {
  return (
    <main className="shell" data-contract-status={contractExample.status}>
      <section aria-labelledby="title" className="card">
        <p className="eyebrow">Takt</p>
        <h1 id="title">Technical foundation</h1>
        <p>
          The reproducible server, web build, and public contracts are ready for
          the first product slice.
        </p>
      </section>
    </main>
  );
}
