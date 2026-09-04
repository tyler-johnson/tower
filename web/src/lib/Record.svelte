<script lang="ts">
  import { render } from "./markdown";
  import { panel } from "./panel.svelte";
  import { query } from "./query.svelte";
  import { stream } from "./stream";
  import { age, statusDot, type Brief } from "./tower";
  import { feed } from "./feed.svelte";

  let { brief, refs }: { brief: Brief; refs: Map<string, string> } = $props();

  let now = $derived(feed.now);
  let entries = $derived(stream(brief));

  /// Which of the two texts is open, and the draft standing in for it.
  /// One at a time: a reader edits one thing.
  let editing = $state<"subject" | "body" | null>(null);
  let draft = $state("");
  /// Which face each of the two markdown boxes is showing. Both start on
  /// the writing.
  let bodyTab = $state<"write" | "preview">("write");
  let noteTab = $state<"write" | "preview">("write");
  /// The comment box, which is always open — a note on a closed record
  /// is fine, and comment.rs is the one verb with no active guard.
  let note = $state("");

  function open(which: "subject" | "body") {
    editing = which;
    draft = which === "subject" ? brief.subject : brief.body;
    if (which === "body") bodyTab = "write";
  }

  /// The subject's Enter and Escape both end through the blur: Escape
  /// puts the record's own words back first, so the commit that follows
  /// finds nothing changed and writes nothing. The body's Escape does
  /// the same by hand, since its form has no blur to commit on.
  function keydown(event: KeyboardEvent, which: "subject" | "body") {
    if (event.key === "Escape") {
      draft = which === "subject" ? brief.subject : brief.body;
      if (which === "subject") (event.currentTarget as HTMLElement).blur();
      else editing = null;
    } else if (event.key === "Enter" && which === "subject") {
      event.preventDefault();
      (event.currentTarget as HTMLElement).blur();
    }
  }

  async function commit(which: "subject" | "body") {
    editing = null;
    const value = draft.trim();
    const was = which === "subject" ? brief.subject : brief.body;
    // An empty box commits nothing: neither text has a clearing on the
    // wire, and the same words are no edit at all.
    if (value === "" || value === was) return;
    await panel.write(
      "edit",
      which === "subject"
        ? { target: brief.id, subject: value }
        : { target: brief.id, message: value },
    );
  }

  async function saveBody(event: SubmitEvent) {
    event.preventDefault();
    await commit("body");
  }

  async function send(event: SubmitEvent) {
    event.preventDefault();
    if (note.trim() === "") return;
    await panel.run("comment", { message: note });
    if (panel.error === null) {
      note = "";
      noteTab = "write";
    }
  }
</script>

<article class="flex flex-col gap-6">
  <!--
		The subject and the body are the record. The subject is one line
		and edits in place — the text a reader reads is the text they type
		into. The body is markdown, and rendered markdown cannot live
		inside a button, so it opens from its own `edit` control instead.
	-->
  <header class="flex flex-col gap-2">
    {#if editing === "subject"}
      <!-- svelte-ignore a11y_autofocus -->
      <input
        type="text"
        class="input input-lg w-full font-medium"
        aria-label="the subject"
        autofocus
        bind:value={draft}
        onkeydown={(event) => keydown(event, "subject")}
        onblur={() => commit("subject")}
      />
    {:else}
      <h1 class="flex items-baseline gap-2">
        <button
          type="button"
          class="cursor-text text-left text-xl font-medium"
          onclick={() => open("subject")}
        >
          {brief.subject}
        </button>
        {#if brief.progress !== null}
          <span class="text-base-content/40 font-mono text-sm">
            ({brief.progress[0]}/{brief.progress[1]})
          </span>
        {/if}
      </h1>
    {/if}
    <p class="text-base-content/40 font-mono text-xs">
      filed by {brief.filed_by} · {age(now, brief.filed_at)}
    </p>
  </header>

  <!--
		The body, written and read as markdown. Blur cannot commit it: the
		tabs unmount the textarea, and an unmounted textarea never blurs.
		So it is a form, saved and abandoned by its own two buttons, the
		shape the rail's gestures already take.
	-->
  {#if editing === "body"}
    <form class="flex flex-col gap-2" onsubmit={saveBody}>
      <div class="tabs tabs-box tabs-xs w-fit">
        <button
          type="button"
          class="tab {bodyTab === 'write' ? 'tab-active' : ''}"
          onclick={() => (bodyTab = "write")}
        >
          write
        </button>
        <button
          type="button"
          class="tab {bodyTab === 'preview' ? 'tab-active' : ''}"
          onclick={() => (bodyTab = "preview")}
        >
          preview
        </button>
      </div>
      {#if bodyTab === "write"}
        <!-- svelte-ignore a11y_autofocus -->
        <textarea
          class="textarea w-full text-sm"
          rows="12"
          aria-label="the body"
          autofocus
          bind:value={draft}
          onkeydown={(event) => keydown(event, "body")}></textarea>
      {:else}
        <div class="prose border-base-300 rounded-box max-w-none border p-3">
          {@html render(draft)}
        </div>
      {/if}
      <div class="flex gap-2">
        <button type="submit" class="btn btn-sm btn-primary" disabled={panel.busy}>save</button>
        <button type="button" class="btn btn-sm btn-ghost" onclick={() => (editing = null)}>
          never mind
        </button>
      </div>
    </form>
  {:else if brief.body}
    <section class="flex flex-col gap-1">
      <div class="flex items-baseline justify-between gap-2">
        <h2 class="text-base-content/60 font-mono text-xs font-medium tracking-[0.2em] uppercase">
          body
        </h2>
        <button
          type="button"
          class="text-base-content/40 hover:text-base-content font-mono text-xs"
          onclick={() => open("body")}
        >
          edit
        </button>
      </div>
      <div class="prose max-w-none">{@html render(brief.body)}</div>
    </section>
  {:else}
    <!-- Nothing to click through to, so the placeholder stays the control. -->
    <button
      type="button"
      class="text-base-content/40 cursor-text text-left text-sm"
      onclick={() => open("body")}
    >
      no body
    </button>
  {/if}

  <!--
		The family, parents up and children down: every depends-on edge is
		a parent edge, so `blocks` is this flight's parents and
		`depends_on` is its children.
	-->
  {#if brief.blocks.length > 0}
    <section class="flex flex-col gap-1">
      <h2 class="text-base-content/60 font-mono text-xs font-medium tracking-[0.2em] uppercase">
        parents
      </h2>
      {#each brief.blocks as link (link.flight)}
        <a
          href={query.href(`/f/${link.flight}`)}
          class="rounded-field hover:bg-base-200 flex items-baseline gap-2 px-1"
        >
          <span class="text-primary font-mono">{refs.get(link.flight) ?? link.flight}</span>
          <span class="status {statusDot(link.status)}" title={link.status}></span>
          <span class="flex-1 truncate">{link.subject}</span>
        </a>
      {/each}
    </section>
  {/if}

  <!--
		The children as a folder: one level, because a brief carries one
		level of links and no more. The whole tree, parents up and children
		down over the same edges, is the projects view's job, and it stops
		here.
	-->
  {#if brief.depends_on.length > 0}
    <section class="flex flex-col gap-1">
      <h2 class="text-base-content/60 font-mono text-xs font-medium tracking-[0.2em] uppercase">
        children
      </h2>
      <p class="text-base-content/60 flex items-baseline gap-2 font-mono text-sm">
        <span>📁</span>
        <span class="truncate">{brief.subject}</span>
      </p>
      {#each brief.depends_on as link (link.flight)}
        <a
          href={query.href(`/f/${link.flight}`)}
          class="rounded-field border-base-300 hover:bg-base-200 ml-4 flex items-baseline gap-2 border-l px-1 pl-3"
        >
          <span class="text-primary font-mono">{refs.get(link.flight) ?? link.flight}</span>
          <span class="status {statusDot(link.status)}" title={link.status}></span>
          <span class="flex-1 truncate">{link.subject}</span>
          {#if link.closed}<span class="text-success">✓</span>{/if}
        </a>
      {/each}
    </section>
  {/if}

  <!--
		What happened and what was said, one column and one order: the two
		lists the wire splits are the same events, and a reader following a
		record follows time.
	-->
  {#if entries.length > 0}
    <section class="flex flex-col gap-3">
      <h2 class="text-base-content/60 font-mono text-xs font-medium tracking-[0.2em] uppercase">
        stream
      </h2>
      {#each entries as entry (entry.id)}
        {#if entry.kind === "comment"}
          <div class="flex flex-col gap-1">
            <!--
							The wire id leads the header: it is a comment's only
							name, and what `edit` takes.
						-->
            <p class="text-base-content/40 font-mono text-xs">
              {entry.id} · {entry.by} · {age(now, entry.at)}
            </p>
            <div class="prose max-w-none">{@html render(entry.text)}</div>
          </div>
        {:else}
          <div class="flex flex-col gap-1">
            <p class="text-base-content/40 font-mono text-xs">
              {entry.id} · {entry.what}{entry.line} · {entry.by} · {age(now, entry.at)}
            </p>
            {#if entry.note}
              <div class="prose prose-dim max-w-none pl-4">{@html render(entry.note)}</div>
            {/if}
          </div>
        {/if}
      {/each}
    </section>
  {/if}

  <!--
		The comment box, markdown like the body. The draft is the state
		either way, so a submit from the preview sends what is shown.
	-->
  <form class="flex flex-col gap-2" onsubmit={send}>
    <label class="flex w-full flex-col gap-2">
      <span class="text-sm font-medium">comment</span>
      <div class="tabs tabs-box tabs-xs w-fit">
        <button
          type="button"
          class="tab {noteTab === 'write' ? 'tab-active' : ''}"
          onclick={() => (noteTab = "write")}
        >
          write
        </button>
        <button
          type="button"
          class="tab {noteTab === 'preview' ? 'tab-active' : ''}"
          onclick={() => (noteTab = "preview")}
        >
          preview
        </button>
      </div>
      {#if noteTab === "write"}
        <textarea class="textarea w-full" rows="4" bind:value={note}></textarea>
      {:else}
        <div class="prose border-base-300 rounded-box max-w-none border p-3">
          {@html render(note)}
        </div>
      {/if}
    </label>
    <div>
      <button type="submit" class="btn btn-sm" disabled={panel.busy || note.trim() === ""}>
        comment
      </button>
    </div>
  </form>
</article>
