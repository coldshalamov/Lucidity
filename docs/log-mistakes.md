# Mistake Log

## 2026-01-16

- Accidentally created a `nul` file in repo root while running Windows `where` commands; removed it.

## 2026-01-17

- Tried to install Strawberry Perl via Chocolatey from a non-elevated shell; install failed due to permissions/lockfile issues.
- Initially attempted to patch Git/MSYS perl by stubbing `Locale::Maketext::Simple`, but OpenSSL vendored builds also needed additional missing perl modules (eg. `ExtUtils::MakeMaker`). Fixed by using Strawberry Perl portable for builds.

