# Contributing

## Setup

```sh
git clone https://github.com/gremlinltd/space-investigator.git
cd space-investigator
cargo build
```

Install the commit hooks (requires [prek](https://github.com/j178/prek)):

```sh
prek install
prek install --hook-type commit-msg
```

## Commits

We use [Conventional Commits](https://www.conventionalcommits.org/). Commit messages look like this:

```
type(optional scope): description
```

Types and what they do on merge to main:

| Type | Version bump | Example |
|------|-------------|---------|
| `fix:` | patch | `fix: handle symlinks correctly` |
| `feat:` | minor | `feat: add csv output` |
| `feat!:` | major | `feat!: change json schema` |
| `chore:` | patch | `chore: update dependencies` |
| `docs:` | patch | `docs: fix install instructions` |
| `refactor:` | patch | `refactor: simplify size collection` |
| `test:` | patch | `test: add json output tests` |
| `ci:` | patch | `ci: pin action versions` |

The commit hook and CI both enforce this, so you'll know straight away if a message doesn't match.

## Branching

We use Gitflow:

- `main` - releases, never commit directly
- `develop` - integration branch
- `feature/<name>` - new features
- `bugfix/<name>` - bug fixes
- `hotfix/<name>` - urgent fixes off main
- `release/<name>` - release prep

## Testing

```sh
cargo test --locked
cargo clippy --locked -- -D warnings
cargo fmt --check
```

All three need to pass before we merge.

## Pull requests

- Fill out the PR template
- Keep changes focused, one thing per PR
- Commit messages drive the changelog and version bumps, so keep them conventional
