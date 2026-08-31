# Worked examples

The engine ships empty. There are no built-in procedures and no built-in skills — every opinion about how work flows is a file its owner authored, and this directory is where the examples live so you can copy one in and fork it.

A **procedure** is the named recipe for one shape of work, as data: a name, optional match rules, and the flights it stamps out, each with the same fields any flight carries, pre-filled. Filing under one mints the whole family in a single append, and the definition is read once — editing it afterwards never disturbs a flight already in the air.

A **skill** is the prose an agent-assigned flight is flown with: instructions a harness executes, never a process tower spawns. A procedure's flight names a skill by string, and that seam is what keeps structure in data and judgment in markdown.

## The two layers

Both kinds layer the same way, keyed by name, the more specific replacing the less wholesale:

| layer | procedures | skills |
|---|---|---|
| user | `~/.config/tower/procedures/<name>.toml` | `~/.config/tower/skills/<name>.md` |
| repository | `<main worktree>/.tower/procedures/<name>.toml` | `<main worktree>/.tower/skills/<name>.md` |

The user layer roams with your config; the repository layer is the team's, and it is anchored to the main worktree so every bay sees the same set. `$XDG_CONFIG_HOME` replaces `~/.config` when it is set. A missing directory is an empty layer; a file that does not parse is a refusal naming the path.

`ff tower procedures` and `ff tower skills` list what is installed and name the layer each came from.

## Copy one in

For the team, from the root of a clone of this repository:

```sh
mkdir -p .tower/procedures .tower/skills
cp docs/procedures/ticket.toml .tower/procedures/     # one flight, yours
cp docs/procedures/review.toml .tower/procedures/     # pass and smoke, then your verdict
cp docs/skills/plan.md         .tower/skills/         # decompose a goal into linked flights
cp docs/skills/review.md       .tower/skills/         # first-pass a branch
cp docs/skills/work.md         .tower/skills/         # claim, do, hold or commit, repeat
```

For yourself, `~/.config/tower/procedures/` and `~/.config/tower/skills/` take the same files.

The name a procedure is filed under is the `name =` line inside the file, not the file name — rename `ticket.toml` freely, and rename what is inside it to change the word `ff tower file` takes.

## The harness redirect

tower never writes another program's config, so the bridge to a harness is your own redirect: `ff tower skills <name>` prints the installed file raw, byte for byte.

```sh
ff tower skills work > .claude/skills/tower-work/SKILL.md
```

## A procedure should end with you

The boundary where work becomes visible to the team is a human gesture. `ff tower procedures` and `ff tower doctor` warn — by name and by flight — when every terminal flight of a definition is agent-assigned. It is a warning and not a refusal, because the file is yours and the boundary that actually holds is `never auto-outward`: whatever an agent finishes, nothing leaves the machine without a person's verb.
