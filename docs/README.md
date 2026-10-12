# Documentation

The [documentation website](docs-site/) contains learner guides for getting
started, navigation, Chat, Practice, coaching, Progress, Skills, settings and
troubleshooting. Its downloads page resolves installers from published releases.
Guides describe current source behavior; installed releases may differ.

The [UI](../ui/), [native](../native/) and [server](../server/) READMEs own developer
instructions. The [design system](design-system/) documents generated UI tokens,
components and icons. [Working notes](notes/README.md) hold investigations,
proposals and verification reports.

The [AI graph foundations](notes/ai-graph-foundations.md) records the agreed
architecture-first constraints and formal specification for AI execution and
its visualization. Future graph work must follow those constraints; they are not
a claim about current runtime conformance. The linked audit separates existing
behavior, migration requirements and verification from the proposed architecture.
The [initial core profile](notes/ai-graph-core-semantics.md) records the isolated
implementation and the remaining foundation gates before workflow migration.

Former website architecture, plans and audits are retained in
[docs-site-archive](notes/docs-site-archive/README.md), with their original planning
status and supersession notices. Old website addresses point readers to the new
guides and retained references. Moving a reference does not validate its claims.

## Build and review the website

Use Node 24. From the repository root:

```sh
npm ci
npm ci --prefix docs/docs-site
npm run docs:test
npm run docs:links
npm run typecheck --prefix docs/docs-site
npm run build --prefix docs/docs-site
npm run preview:local --prefix docs/docs-site
```

Open `http://127.0.0.1:3000/skellyspeak/`. For documentation editing, run
`npm run start --prefix docs/docs-site` instead. Both start and build regenerate
the app demonstrations first. Rebuild/restart after changing demo or app source;
Docusaurus does not watch the separate Vite build.

## App demonstrations and drift

`ui/tools/docs-demo/` builds a small standalone React application using production
UI components, icons, generated catalogs and app CSS. The website embeds it in an
iframe so site styles cannot change the app controls. Fixture state is in memory;
the demos do not sign in, record audio, call AI or save workspace data. Simulated
actions and example results are labeled. These are instructional subsets, not a
browser version of the native app.

`npm run docs:demos` type-checks and builds the fixtures into ignored
`docs/docs-site/static/demos/`. The docs build and CI rebuild them from source,
so changed production component contracts fail the build instead of leaving old
screenshots in place. Root dependencies are required as well as site dependencies.

When changing a user flow, review its guide and demo together. Compilation catches
component contract changes; it cannot prove that prose, sample data or a simplified
navigation description still matches the app. The
[rebuild report](notes/docs-site-rebuild.md) records source owners, verification
and remaining review boundaries.

A local build does not publish the website. Deployment requires explicit authorization.
