// One attachment for every `<details class="dropdown">` in the app, so a
// menu closes the way a person expects. daisyUI's details dropdown is
// native open/close and nothing more: a click outside it and Escape are
// both this file's.

import type { Attachment } from 'svelte/attachments';

/// Close a details on a pointerdown outside it and on Escape. Setting
/// `open` fires the native toggle, so a `bind:open` on the same element
/// follows.
///
/// pointerdown and the event's composed path, not click and `contains`:
/// a click's Svelte handler runs at the root and re-renders in a
/// microtask, which the browser flushes before the window's listener,
/// so by then a menu item that was clicked is no longer in the details
/// and `contains` says outside. The path is fixed when dispatch begins.
export function dismiss(): Attachment<HTMLDetailsElement> {
	return (node) => {
		const pointerdown = (event: PointerEvent) => {
			if (node.open && !event.composedPath().includes(node)) node.open = false;
		};
		const keydown = (event: KeyboardEvent) => {
			if (node.open && event.key === 'Escape') node.open = false;
		};
		window.addEventListener('pointerdown', pointerdown);
		window.addEventListener('keydown', keydown);
		return () => {
			window.removeEventListener('pointerdown', pointerdown);
			window.removeEventListener('keydown', keydown);
		};
	};
}
