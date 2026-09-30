# semlint

A command-line linter for SemVer strings. It reads a list of versions (one
per line -- a versions file, a changelog index, whatever) and reports every
line that doesn't conform to the [SemVer 2.0.0](https://semver.org) grammar,
with a line and column so you can find it.

## Why

Most "semver" in the wild isn't. Git tags get a `v` prefix, changelogs drop
the patch number, release scripts zero-pad components. None of that is
valid per the spec, and tools that silently accept it end up comparing
versions incorrectly somewhere downstream. semlint is strict by default so
those problems get caught instead of quietly tolerated, but real projects
often can't fix years of existing tags overnight -- hence `--lenient`.

## Build

```
cargo build --release
```

The binary is `target/release/semlint`. No third-party crates, so this only
needs a standard Rust toolchain.

## Usage

Given `versions.txt`:

```
1.2.3
v1.2.3
1.02.3
1.2.3-alpha.01
1.2
```

Strict mode (the default) flags the prefix, the zero-padded minor, the
zero-padded pre-release identifier, and the missing patch component:

```
$ semlint versions.txt
versions.txt:2:1: error: 'v1.2.3' has a 'v' prefix; strict SemVer core starts with a digit (pass --lenient to allow it)
versions.txt:3:1: error: minor component '02' has a leading zero, which SemVer forbids
versions.txt:4:1: error: numeric pre-release identifier '01' in '1.2.3-alpha.01' has a leading zero
versions.txt:5:1: error: version core must be exactly major.minor.patch (three dot-separated numbers), found '1.2'
$ echo $?
1
```

`--lenient` accepts the `v` prefix and short-form versions like `1.2` (they
become warnings, not errors) but still flags things that are wrong in any
tag scheme, like leading zeros:

```
$ semlint --lenient versions.txt
versions.txt:3:1: error: minor component '02' has a leading zero, which SemVer forbids
versions.txt:4:1: error: numeric pre-release identifier '01' in '1.2.3-alpha.01' has a leading zero
versions.txt:5:1: warning: '1.2' has 2 of 3 numeric components; missing components are treated as zero (--lenient)
$ echo $?
1
```

Exit status is 1 if any error-level finding was reported, 0 otherwise
(warnings alone don't fail the run). With no file argument, or `-` as the
file argument, semlint reads from stdin. Blank lines and lines starting
with `#` are skipped, so a versions file can carry its own comments.

## What counts as a finding

- a `v`/`V` prefix on the version core (strict: error, lenient: stripped)
- a version core that isn't exactly `major.minor.patch` (strict: error,
  lenient: warning if the parts present are numeric)
- a non-numeric or leading-zero major/minor/patch component
- an empty, malformed, or leading-zero pre-release identifier
- an empty or malformed build-metadata identifier

## Manifests

A file named `Cargo.toml` or `package.json` is read as a manifest instead of
a list: semlint finds the package's own version and lints just that.

```
$ semlint Cargo.toml web/package.json
Cargo.toml:3:12: error: minor component '02' has a leading zero, which SemVer forbids
```

For `Cargo.toml` the field is `version` under `[package]` or
`[workspace.package]`; dependency versions are not checked, and
`version.workspace = true` has nothing to check. For `package.json` it is
the top-level `"version"` string. If a manifest has no version field, a
note goes to stderr and the exit status is unaffected. Detection is by file
name only, and the lookup is a small scanner rather than a full TOML or
JSON parser, so exotic formatting (a JSON value on a different line from
its key, for example) is not picked up.

## Status

Early. Plain version lists and the two manifests above are supported;
there's no reading of Git tags directly yet.
