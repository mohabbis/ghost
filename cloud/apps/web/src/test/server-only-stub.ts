/**
 * Stands in for Next's `server-only` package under vitest.
 *
 * That package exists to make a build fail when a client component imports a
 * server module; it has no runtime behaviour to reproduce, and no resolvable
 * module for vitest to load. Aliased in `vitest.config.ts`.
 *
 * Deliberately empty — importing it must do nothing.
 */
export {};
