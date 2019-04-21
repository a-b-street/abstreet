# Notes on Rust in A/B Street

This article describes parts of A/B Street's implementation that might be of
interest to the Rust community.

TODO: TOC

TODO: go through all notes (and delete duplicate stuff)

## ezgui

Not to complicate the GUI debate even more, but...

### WrappedWizard

## Timer plumbing

doubles as Warn, kind of a logger

that recent mailing list thread

## Test runner

sane output, and being able to embed really useful hotlinks

## Determinism

no binary heap :(

forking rng

## Grievances

Compile times. Tiny tweak in geom, everything that depends on it also gets
recompiled. Might just be relinking, but feels slow enough to be redoing stuff.
Very painful.

## Appendix: Code organization

by crate
