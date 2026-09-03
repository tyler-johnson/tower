<script lang="ts">
  import { cell, noteStart } from "./columns";
  import { fieldLabel, type Field } from "./query";
  import { query } from "./query.svelte";
  import { notePhrases, statusDot, type FlightView } from "./tower";

  let {
    view,
    refs,
    now,
    open,
    show,
  }: {
    view: FlightView;
    refs: Map<string, string>;
    now: number;
    open: boolean;
    show: Field[];
  } = $props();

  let phrases = $derived(notePhrases(view, refs));
</script>

<!--
	One row of the list. The columns come from `show`, one cell per field
	drawn by its kind — the default seven are the recognizable anatomy:
	priority glyph, flight ref, status dot, subject with its progress
	mark, label chips, assignee, and the age right-aligned. The row is a
	subgrid line of the section's grid, so its columns align with the
	rows above and below. The phrases only this tracker can print — the
	audits, the collisions — go underneath from the subject's column,
	warn ones in the warn tone.
-->
<a
  href={query.href(`/f/${view.id}`)}
  class="rounded-field hover:bg-base-200 col-span-full grid grid-cols-subgrid items-baseline gap-x-2 px-1 {open
    ? 'bg-base-200'
    : ''}"
>
  {#each show as field (field)}
    {@const c = cell(field, view, refs, now)}
    {#if c.kind === "glyph"}
      <span class="text-base-content/60" title={c.title}>{c.text}</span>
    {:else if c.kind === "ref"}
      <span class="text-primary font-mono">{c.text}</span>
    {:else if c.kind === "dot"}
      <span class="status {statusDot(c.status)}" title={c.status}></span>
    {:else if c.kind === "subject"}
      <span class="truncate">{c.text}</span>
    {:else if c.kind === "chips"}
      <span class="flex items-baseline gap-2">
        {#each c.words as word (word)}
          <span class="badge badge-ghost badge-sm">{word}</span>
        {/each}
      </span>
    {:else if c.kind === "dim"}
      <span
        class="text-base-content/40 text-sm {field === 'age' ? 'text-right' : ''}"
        title={fieldLabel(field)}
      >
        {c.text}
      </span>
    {:else if c.kind === "flag"}
      <span class="text-sm {c.on ? 'text-warning' : ''}" title={fieldLabel(field)}>
        {c.on ? c.text : ""}
      </span>
    {/if}
  {/each}

  {#if phrases.length > 0}
    <span class="text-base-content/40 text-sm" style:grid-column="{noteStart(show)} / -1">
      {#each phrases as phrase, i (i)}
        {#if i > 0}<span> · </span>{/if}
        <span class={phrase.tone === "warn" ? "text-warning" : ""}>{phrase.text}</span>
      {/each}
    </span>
  {/if}
</a>
