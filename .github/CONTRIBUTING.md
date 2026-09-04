# Contributing to Entropix

Thanks for taking an interest in the project.

## Workflow

- `main` is the stable branch — always buildable, always deployable.
- New work happens on a feature branch cut from `main`:
  `git checkout main && git pull && git checkout -b feature/your-feature-name`
- Open a Pull Request into `main` when ready. PRs require a review before merging.
- We don't rebase or force-push over branches other people are working from.

## Commits

- Keep commits atomic — one logical change per commit.
- Use conventional commit prefixes: `feat:`, `fix:`, `docs:`, `chore:`, `refactor:`.
- If a commit closes an issue, mention it in the body: `Fixes #12`.
- If someone genuinely contributed to a commit without being its author
  (paired on the logic, reviewed and shaped it before it was written),
  credit them with a trailer:
  `Co-authored-by: Name <email>`

## Before opening a PR

```bash
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings
```

If you changed a `sqlx::query!`/`query_as!` call or the database schema,
regenerate the offline query cache before committing:

```bash
cargo sqlx prepare
git add .sqlx/
```

## Database changes

New schema changes go in a new numbered migration file under
`migrations/` (e.g. `0003_your_change.sql`) — never edit an already-applied
migration file.

## Code style

Comments explain *why*, not *what* — assume the reader can read Rust.
Avoid referencing external planning documents in comments; the code and
its comments should be understandable on their own.
